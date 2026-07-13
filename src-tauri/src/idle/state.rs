use crate::crypto::DeviceKeys;
use crate::error::AgentResult;
use parking_lot::Mutex;
use rusqlite::Connection;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::constants::{
    COUNTDOWN_SECS, MANUAL_INPUT_GAP_SECS, MANUAL_PAUSE_ACTIVITY_SECS,
    MANUAL_PAUSE_AUTO_RESUME_SECS,
};
use super::persistence::{
    classify_idle_period_record, discard_idle_period_record, finalize_idle_period_on_resume,
    insert_paused_idle_period,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdlePhase {
    Active,
    Warning,
    Countdown,
    PausedIdle,
    ResumePrompt,
    ManualPaused,
    ManualWorkCheck,
}

impl IdlePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Warning => "warning",
            Self::Countdown => "countdown",
            Self::PausedIdle => "paused_idle",
            Self::ResumePrompt => "resume_prompt",
            Self::ManualPaused => "manual_paused",
            Self::ManualWorkCheck => "manual_work_check",
        }
    }
}

fn is_billable_phase(phase: IdlePhase) -> bool {
    matches!(
        phase,
        IdlePhase::Active | IdlePhase::Warning | IdlePhase::Countdown
    )
}

#[derive(Debug, Clone)]
pub struct IdleSnapshot {
    pub phase: IdlePhase,
    pub threshold_secs: u64,
    pub countdown_secs: u64,
    pub countdown_remaining_secs: Option<u64>,
    pub countdown_ends_at: Option<String>,
    pub idle_started_at: Option<String>,
    pub paused_at: Option<String>,
    pub away_seconds: Option<u64>,
    pub pending_period_id: Option<String>,
    pub meeting_exempt: bool,
    pub active_seconds: u64,
    pub idle_discarded_seconds: u64,
    pub idle_reclassified_seconds: u64,
}

pub struct IdleController {
    phase: Mutex<IdlePhase>,
    threshold_secs: u64,
    last_input_at: Arc<Mutex<Instant>>,
    last_input_wall_at: Arc<Mutex<String>>,
    last_polled_input: Mutex<Instant>,
    countdown_started_at: Mutex<Option<Instant>>,
    idle_started_at: Mutex<Option<String>>,
    paused_at: Mutex<Option<String>>,
    pending_period_id: Mutex<Option<String>>,
    active_seconds: Mutex<u64>,
    segment_start: Mutex<Option<Instant>>,
    idle_discarded_seconds: Mutex<u64>,
    idle_reclassified_seconds: Mutex<u64>,
    meeting_exempt: Mutex<bool>,
    manual_input_streak_start: Mutex<Option<Instant>>,
    last_manual_input_at: Mutex<Instant>,
}

impl IdleController {
    pub fn new(
        threshold_secs: u64,
        last_input_at: Arc<Mutex<Instant>>,
        last_input_wall_at: Arc<Mutex<String>>,
    ) -> Self {
        let now = Instant::now();
        *last_input_at.lock() = now;
        *last_input_wall_at.lock() = chrono::Utc::now().to_rfc3339();
        Self {
            phase: Mutex::new(IdlePhase::Active),
            threshold_secs,
            last_input_at,
            last_input_wall_at,
            last_polled_input: Mutex::new(now),
            countdown_started_at: Mutex::new(None),
            idle_started_at: Mutex::new(None),
            paused_at: Mutex::new(None),
            pending_period_id: Mutex::new(None),
            active_seconds: Mutex::new(0),
            segment_start: Mutex::new(Some(now)),
            idle_discarded_seconds: Mutex::new(0),
            idle_reclassified_seconds: Mutex::new(0),
            meeting_exempt: Mutex::new(false),
            manual_input_streak_start: Mutex::new(None),
            last_manual_input_at: Mutex::new(now),
        }
    }

    pub fn set_meeting_exempt(&self, exempt: bool) {
        *self.meeting_exempt.lock() = exempt;
        if exempt {
            let now = Instant::now();
            *self.last_input_at.lock() = now;
            *self.last_input_wall_at.lock() = chrono::Utc::now().to_rfc3339();
            self.cancel_idle_flow();
            if self.segment_start.lock().is_none() {
                *self.segment_start.lock() = Some(now);
            }
        }
    }

    pub fn tick(
        &self,
        conn: &Connection,
        session_id: &str,
        device_keys: &DeviceKeys,
    ) -> AgentResult<()> {
        let phase = *self.phase.lock();

        if phase == IdlePhase::ManualPaused || phase == IdlePhase::ManualWorkCheck {
            self.detect_input(conn, device_keys)?;
            self.tick_manual_pause()?;
            return Ok(());
        }

        self.detect_input(conn, device_keys)?;

        if *self.meeting_exempt.lock() {
            return Ok(());
        }

        let now = Instant::now();
        let last_input = *self.last_input_at.lock();
        let idle_for = now.duration_since(last_input);
        let threshold = Duration::from_secs(self.threshold_secs);

        let phase = *self.phase.lock();
        match phase {
            IdlePhase::Active => {
                if idle_for >= threshold {
                    self.enter_warning();
                    self.start_idle_countdown();
                }
            }
            IdlePhase::Warning => {
                self.start_idle_countdown();
            }
            IdlePhase::Countdown => {
                let countdown_start = self.countdown_started_at.lock().unwrap_or(now);
                if now.duration_since(countdown_start) >= Duration::from_secs(COUNTDOWN_SECS) {
                    self.enter_paused(conn, session_id, device_keys, last_input, now)?;
                }
            }
            IdlePhase::PausedIdle | IdlePhase::ResumePrompt => {}
            IdlePhase::ManualPaused | IdlePhase::ManualWorkCheck => {}
        }

        Ok(())
    }

    pub fn pause_manually(&self) {
        let phase = *self.phase.lock();
        if matches!(
            phase,
            IdlePhase::ManualPaused
                | IdlePhase::ManualWorkCheck
                | IdlePhase::PausedIdle
                | IdlePhase::ResumePrompt
        ) {
            return;
        }

        if matches!(phase, IdlePhase::Warning | IdlePhase::Countdown) {
            self.cancel_idle_flow();
        }
        self.freeze_billable_at();
        *self.phase.lock() = IdlePhase::ManualPaused;
        *self.paused_at.lock() = Some(chrono::Utc::now().to_rfc3339());
        *self.manual_input_streak_start.lock() = None;
        let now = Instant::now();
        *self.last_manual_input_at.lock() = now;
        log::info!("session manually paused");
    }

    pub fn resume_manually(&self) {
        if !matches!(
            *self.phase.lock(),
            IdlePhase::ManualPaused | IdlePhase::ManualWorkCheck
        ) {
            return;
        }

        let now = Instant::now();
        *self.phase.lock() = IdlePhase::Active;
        *self.paused_at.lock() = None;
        *self.manual_input_streak_start.lock() = None;
        *self.segment_start.lock() = Some(now);
        *self.last_input_at.lock() = now;
        *self.last_input_wall_at.lock() = chrono::Utc::now().to_rfc3339();
        *self.last_polled_input.lock() = now;
        *self.last_manual_input_at.lock() = now;
        log::info!("session manually resumed");
    }

    pub fn confirm_manual_work(&self) -> AgentResult<()> {
        if *self.phase.lock() != IdlePhase::ManualWorkCheck {
            return Ok(());
        }
        let now = Instant::now();
        self.resume_with_work_credit(now);
        Ok(())
    }

    pub fn dismiss_manual_work_check(&self) -> AgentResult<()> {
        if *self.phase.lock() != IdlePhase::ManualWorkCheck {
            return Ok(());
        }
        *self.phase.lock() = IdlePhase::ManualPaused;
        *self.manual_input_streak_start.lock() = None;
        let now = Instant::now();
        *self.last_manual_input_at.lock() = now;
        log::info!("manual work check dismissed");
        Ok(())
    }

    pub fn confirm_still_working(&self) -> AgentResult<()> {
        let phase = *self.phase.lock();
        if !matches!(phase, IdlePhase::Warning | IdlePhase::Countdown) {
            return Ok(());
        }
        let now = Instant::now();
        *self.last_input_at.lock() = now;
        *self.last_input_wall_at.lock() = chrono::Utc::now().to_rfc3339();
        *self.last_polled_input.lock() = now;
        self.cancel_idle_flow();
        if self.segment_start.lock().is_none() {
            *self.segment_start.lock() = Some(now);
        }
        Ok(())
    }

    pub fn classify_idle_period(
        &self,
        conn: &Connection,
        period_id: &str,
        category: &str,
        device_keys: &DeviceKeys,
    ) -> AgentResult<()> {
        if *self.phase.lock() != IdlePhase::ResumePrompt {
            return Ok(());
        }

        let pending = self.pending_period_id.lock().clone();
        if pending.as_deref() != Some(period_id) {
            return Ok(());
        }

        let (reclassify, duration) =
            classify_idle_period_record(conn, period_id, category, device_keys)?;

        if reclassify {
            *self.idle_reclassified_seconds.lock() += duration;
            *self.idle_discarded_seconds.lock() =
                self.idle_discarded_seconds.lock().saturating_sub(duration);
            *self.active_seconds.lock() += duration;
        }

        self.finish_resume_prompt();
        Ok(())
    }

    pub fn skip_idle_classification(
        &self,
        conn: &Connection,
        device_keys: &DeviceKeys,
    ) -> AgentResult<()> {
        if *self.phase.lock() != IdlePhase::ResumePrompt {
            return Ok(());
        }

        if let Some(period_id) = self.pending_period_id.lock().clone() {
            discard_idle_period_record(conn, &period_id, device_keys)?;
        }

        self.finish_resume_prompt();
        Ok(())
    }

    pub fn snapshot(&self) -> IdleSnapshot {
        let phase = *self.phase.lock();
        let now = Instant::now();
        let countdown_remaining = self.countdown_started_at.lock().map(|started| {
            COUNTDOWN_SECS.saturating_sub(now.duration_since(started).as_secs())
        });

        let away_seconds = if matches!(phase, IdlePhase::PausedIdle | IdlePhase::ResumePrompt) {
            self.idle_started_at.lock().as_ref().map(|start| {
                chrono::DateTime::parse_from_rfc3339(start)
                    .ok()
                    .map(|start_at| {
                        chrono::Utc::now()
                            .signed_duration_since(start_at.with_timezone(&chrono::Utc))
                            .num_seconds()
                            .max(0) as u64
                    })
                    .unwrap_or(0)
            })
        } else {
            None
        };

        IdleSnapshot {
            phase,
            threshold_secs: self.threshold_secs,
            countdown_secs: COUNTDOWN_SECS,
            countdown_remaining_secs: if phase == IdlePhase::Countdown {
                countdown_remaining
            } else {
                None
            },
            countdown_ends_at: if phase == IdlePhase::Countdown {
                self.countdown_started_at.lock().map(|started| {
                    (chrono::Utc::now()
                        + chrono::Duration::seconds(
                            COUNTDOWN_SECS.saturating_sub(now.duration_since(started).as_secs())
                                as i64,
                        ))
                    .to_rfc3339()
                })
            } else {
                None
            },
            idle_started_at: self.idle_started_at.lock().clone(),
            paused_at: self.paused_at.lock().clone(),
            away_seconds,
            pending_period_id: self.pending_period_id.lock().clone(),
            meeting_exempt: *self.meeting_exempt.lock(),
            active_seconds: self.billable_seconds(),
            idle_discarded_seconds: *self.idle_discarded_seconds.lock(),
            idle_reclassified_seconds: *self.idle_reclassified_seconds.lock(),
        }
    }

    fn tick_manual_pause(&self) -> AgentResult<()> {
        let now = Instant::now();
        let last_input = *self.last_input_at.lock();
        let gap = Duration::from_secs(MANUAL_INPUT_GAP_SECS);

        if now.duration_since(last_input) > gap {
            *self.manual_input_streak_start.lock() = None;
            return Ok(());
        }

        let streak_start = match *self.manual_input_streak_start.lock() {
            Some(start) => start,
            None => return Ok(()),
        };

        let streak_secs = now.duration_since(streak_start).as_secs();
        let phase = *self.phase.lock();

        if phase == IdlePhase::ManualPaused && streak_secs >= MANUAL_PAUSE_ACTIVITY_SECS {
            *self.phase.lock() = IdlePhase::ManualWorkCheck;
            log::info!("manual pause: activity detected, showing work check");
        }

        if streak_secs >= MANUAL_PAUSE_AUTO_RESUME_SECS {
            self.resume_with_work_credit(now);
            log::info!("manual pause: auto-resumed after sustained activity");
        }

        Ok(())
    }

    fn track_manual_pause_input(&self, input_at: Instant) {
        let last_tracked = *self.last_manual_input_at.lock();
        if input_at <= last_tracked {
            return;
        }
        *self.last_manual_input_at.lock() = input_at;

        if self.manual_input_streak_start.lock().is_none() {
            *self.manual_input_streak_start.lock() = Some(input_at);
        }
    }

    fn resume_with_work_credit(&self, now: Instant) {
        let credit_secs = self
            .manual_input_streak_start
            .lock()
            .map(|start| now.duration_since(start).as_secs())
            .unwrap_or(0);

        if credit_secs > 0 {
            *self.active_seconds.lock() += credit_secs;
            log::info!("manual pause: credited {credit_secs}s of work time");
        }

        *self.phase.lock() = IdlePhase::Active;
        *self.paused_at.lock() = None;
        *self.manual_input_streak_start.lock() = None;
        *self.segment_start.lock() = Some(now);
        *self.last_input_at.lock() = now;
        *self.last_input_wall_at.lock() = chrono::Utc::now().to_rfc3339();
        *self.last_polled_input.lock() = now;
        *self.last_manual_input_at.lock() = now;
    }

    fn detect_input(&self, conn: &Connection, device_keys: &DeviceKeys) -> AgentResult<()> {
        let current = *self.last_input_at.lock();
        let previous = *self.last_polled_input.lock();
        if current <= previous {
            return Ok(());
        }
        *self.last_polled_input.lock() = current;

        let phase = *self.phase.lock();
        match phase {
            IdlePhase::Active => {}
            IdlePhase::Warning | IdlePhase::Countdown => {
                // Durante o alerta, input global (mouse/teclado) não cancela o countdown.
                // O usuário precisa confirmar explicitamente na UI.
            }
            IdlePhase::ManualPaused | IdlePhase::ManualWorkCheck => {
                self.track_manual_pause_input(current);
            }
            IdlePhase::PausedIdle => {
                if let Some(period_id) = self.pending_period_id.lock().clone() {
                    let (total, previous) =
                        finalize_idle_period_on_resume(conn, &period_id, device_keys)?;
                    let additional = total.saturating_sub(previous);
                    if additional > 0 {
                        *self.idle_discarded_seconds.lock() += additional;
                    }
                }
                *self.phase.lock() = IdlePhase::ResumePrompt;
            }
            IdlePhase::ResumePrompt => {}
        }
        Ok(())
    }

    fn billable_seconds(&self) -> u64 {
        let base = *self.active_seconds.lock();
        let phase = *self.phase.lock();

        if !is_billable_phase(phase) {
            return base;
        }

        let Some(seg_start) = *self.segment_start.lock() else {
            return base;
        };

        let now = Instant::now();
        if now > seg_start {
            base + now.duration_since(seg_start).as_secs()
        } else {
            base
        }
    }

    fn enter_warning(&self) {
        if *self.phase.lock() != IdlePhase::Active {
            return;
        }

        *self.phase.lock() = IdlePhase::Warning;
        *self.idle_started_at.lock() = Some(self.last_input_wall_at.lock().clone());
        log::info!(
            "idle: warning — no input for {}s",
            self.threshold_secs
        );
    }

    fn start_idle_countdown(&self) {
        let phase = *self.phase.lock();
        if !matches!(phase, IdlePhase::Active | IdlePhase::Warning) {
            return;
        }
        if self.countdown_started_at.lock().is_some() {
            return;
        }

        *self.phase.lock() = IdlePhase::Countdown;
        *self.countdown_started_at.lock() = Some(Instant::now());
        log::info!(
            "idle: alert — {}s countdown after {}s without input",
            COUNTDOWN_SECS,
            self.threshold_secs
        );
    }

    fn enter_paused(
        &self,
        conn: &Connection,
        session_id: &str,
        device_keys: &DeviceKeys,
        last_input: Instant,
        now: Instant,
    ) -> AgentResult<()> {
        let now_billable = self.billable_seconds();
        let idle_after_input = now.duration_since(last_input).as_secs();
        let frozen = now_billable.saturating_sub(idle_after_input);
        *self.active_seconds.lock() = frozen;
        *self.segment_start.lock() = None;

        let idle_started = self.last_input_wall_at.lock().clone();
        let paused_at = chrono::Utc::now().to_rfc3339();
        let discarded = idle_after_input;

        *self.idle_discarded_seconds.lock() += discarded;
        *self.phase.lock() = IdlePhase::PausedIdle;
        *self.paused_at.lock() = Some(paused_at.clone());

        let period_id = insert_paused_idle_period(
            conn,
            session_id,
            &idle_started,
            &paused_at,
            discarded,
            device_keys,
        )?;
        *self.pending_period_id.lock() = Some(period_id);

        log::info!("idle: paused — discarded {discarded}s");
        Ok(())
    }

    fn freeze_billable_at(&self) {
        let frozen_total = self.billable_seconds();
        *self.active_seconds.lock() = frozen_total;
        *self.segment_start.lock() = None;
    }

    fn cancel_idle_flow(&self) {
        *self.phase.lock() = IdlePhase::Active;
        *self.countdown_started_at.lock() = None;
        *self.idle_started_at.lock() = None;
    }

    fn finish_resume_prompt(&self) {
        let now = Instant::now();
        *self.phase.lock() = IdlePhase::Active;
        *self.paused_at.lock() = None;
        *self.pending_period_id.lock() = None;
        *self.idle_started_at.lock() = None;
        *self.countdown_started_at.lock() = None;
        *self.last_input_at.lock() = now;
        *self.last_input_wall_at.lock() = chrono::Utc::now().to_rfc3339();
        *self.last_polled_input.lock() = now;
        *self.segment_start.lock() = Some(now);
    }
}
