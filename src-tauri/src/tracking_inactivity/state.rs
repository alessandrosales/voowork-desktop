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
    classify_tracking_inactivity_period_record, discard_inactivity_period_record, finalize_inactivity_period_on_resume,
    insert_paused_inactivity_period,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackingInactivityPhase {
    Active,
    Warning,
    Countdown,
    PausedInactivity,
    ResumePrompt,
    ManualPaused,
    ManualWorkCheck,
}

impl TrackingInactivityPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Warning => "warning",
            Self::Countdown => "countdown",
            Self::PausedInactivity => "paused_inactivity",
            Self::ResumePrompt => "resume_prompt",
            Self::ManualPaused => "manual_paused",
            Self::ManualWorkCheck => "manual_work_check",
        }
    }
}

fn is_billable_phase(phase: TrackingInactivityPhase) -> bool {
    matches!(
        phase,
        TrackingInactivityPhase::Active | TrackingInactivityPhase::Warning | TrackingInactivityPhase::Countdown
    )
}

#[derive(Debug, Clone)]
pub struct TrackingInactivitySnapshot {
    pub phase: TrackingInactivityPhase,
    pub threshold_secs: u64,
    pub countdown_secs: u64,
    pub countdown_remaining_secs: Option<u64>,
    pub countdown_ends_at: Option<String>,
    pub inactivity_started_at: Option<String>,
    pub paused_at: Option<String>,
    pub away_seconds: Option<u64>,
    pub pending_period_id: Option<String>,
    pub meeting_exempt: bool,
    pub active_seconds: u64,
    pub inactivity_discarded_seconds: u64,
    pub inactivity_reclassified_seconds: u64,
}

pub struct TrackingInactivityController {
    phase: Mutex<TrackingInactivityPhase>,
    threshold_secs: u64,
    last_input_at: Arc<Mutex<Instant>>,
    last_input_wall_at: Arc<Mutex<String>>,
    last_polled_input: Mutex<Instant>,
    countdown_started_at: Mutex<Option<Instant>>,
    inactivity_started_at: Mutex<Option<String>>,
    paused_at: Mutex<Option<String>>,
    pending_period_id: Mutex<Option<String>>,
    active_seconds: Mutex<u64>,
    segment_start: Mutex<Option<Instant>>,
    inactivity_discarded_seconds: Mutex<u64>,
    inactivity_reclassified_seconds: Mutex<u64>,
    meeting_exempt: Mutex<bool>,
    manual_input_streak_start: Mutex<Option<Instant>>,
    last_manual_input_at: Mutex<Instant>,
}

impl TrackingInactivityController {
    pub fn new(
        threshold_secs: u64,
        last_input_at: Arc<Mutex<Instant>>,
        last_input_wall_at: Arc<Mutex<String>>,
    ) -> Self {
        let now = Instant::now();
        *last_input_at.lock() = now;
        *last_input_wall_at.lock() = chrono::Utc::now().to_rfc3339();
        Self {
            phase: Mutex::new(TrackingInactivityPhase::Active),
            threshold_secs,
            last_input_at,
            last_input_wall_at,
            last_polled_input: Mutex::new(now),
            countdown_started_at: Mutex::new(None),
            inactivity_started_at: Mutex::new(None),
            paused_at: Mutex::new(None),
            pending_period_id: Mutex::new(None),
            active_seconds: Mutex::new(0),
            segment_start: Mutex::new(Some(now)),
            inactivity_discarded_seconds: Mutex::new(0),
            inactivity_reclassified_seconds: Mutex::new(0),
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
            self.cancel_inactivity_flow();
            if self.segment_start.lock().is_none() {
                *self.segment_start.lock() = Some(now);
            }
        }
    }

    pub fn tick(&self, conn: &Connection, tracking_id: &str) -> AgentResult<()> {
        let phase = *self.phase.lock();

        if phase == TrackingInactivityPhase::ManualPaused || phase == TrackingInactivityPhase::ManualWorkCheck {
            self.detect_input(conn)?;
            self.tick_manual_pause()?;
            return Ok(());
        }

        self.detect_input(conn)?;

        if *self.meeting_exempt.lock() {
            return Ok(());
        }

        let now = Instant::now();
        let last_input = *self.last_input_at.lock();
        let idle_for = now.duration_since(last_input);
        let threshold = Duration::from_secs(self.threshold_secs);

        let phase = *self.phase.lock();
        match phase {
            TrackingInactivityPhase::Active => {
                if idle_for >= threshold {
                    self.enter_inactivity_warning();
                }
            }
            TrackingInactivityPhase::Warning => {
                self.start_inactivity_countdown();
            }
            TrackingInactivityPhase::Countdown => {
                let countdown_start = self.countdown_started_at.lock().unwrap_or(now);
                if now.duration_since(countdown_start) >= Duration::from_secs(COUNTDOWN_SECS) {
                    self.enter_inactivity_paused(conn, tracking_id, last_input, now)?;
                }
            }
            TrackingInactivityPhase::PausedInactivity | TrackingInactivityPhase::ResumePrompt => {}
            TrackingInactivityPhase::ManualPaused | TrackingInactivityPhase::ManualWorkCheck => {}
        }

        Ok(())
    }

    pub fn pause_manually(&self) {
        let phase = *self.phase.lock();
        if matches!(
            phase,
            TrackingInactivityPhase::ManualPaused
                | TrackingInactivityPhase::ManualWorkCheck
                | TrackingInactivityPhase::PausedInactivity
                | TrackingInactivityPhase::ResumePrompt
        ) {
            return;
        }

        if matches!(phase, TrackingInactivityPhase::Warning | TrackingInactivityPhase::Countdown) {
            self.cancel_inactivity_flow();
        }
        self.freeze_billable_at();
        *self.phase.lock() = TrackingInactivityPhase::ManualPaused;
        *self.paused_at.lock() = Some(chrono::Utc::now().to_rfc3339());
        *self.manual_input_streak_start.lock() = None;
        let now = Instant::now();
        *self.last_manual_input_at.lock() = now;
        log::info!("session manually paused");
    }

    pub fn resume_manually(&self) {
        if !matches!(
            *self.phase.lock(),
            TrackingInactivityPhase::ManualPaused | TrackingInactivityPhase::ManualWorkCheck
        ) {
            return;
        }

        let now = Instant::now();
        *self.phase.lock() = TrackingInactivityPhase::Active;
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
        if *self.phase.lock() != TrackingInactivityPhase::ManualWorkCheck {
            return Ok(());
        }
        let now = Instant::now();
        self.resume_with_work_credit(now);
        Ok(())
    }

    pub fn dismiss_manual_work_check(&self) -> AgentResult<()> {
        if *self.phase.lock() != TrackingInactivityPhase::ManualWorkCheck {
            return Ok(());
        }
        *self.phase.lock() = TrackingInactivityPhase::ManualPaused;
        *self.manual_input_streak_start.lock() = None;
        let now = Instant::now();
        *self.last_manual_input_at.lock() = now;
        log::info!("manual work check dismissed");
        Ok(())
    }

    /// Dismisses a paused inactivity period and resets the session.
    ///
    /// - Discards the inactivity period record in DB
    /// - Resets active_seconds to 0 (fresh start)
    /// - Transitions back to Active
    pub fn dismiss_inactivity_period(&self, conn: &Connection) -> AgentResult<()> {
        if *self.phase.lock() != TrackingInactivityPhase::PausedInactivity {
            return Ok(());
        }

        if let Some(period_id) = self.pending_period_id.lock().clone() {
            discard_inactivity_period_record(conn, &period_id)?;
        }

        let now = Instant::now();
        *self.phase.lock() = TrackingInactivityPhase::Active;
        *self.paused_at.lock() = None;
        *self.pending_period_id.lock() = None;
        *self.inactivity_started_at.lock() = None;
        *self.countdown_started_at.lock() = None;
        *self.last_input_at.lock() = now;
        *self.last_input_wall_at.lock() = chrono::Utc::now().to_rfc3339();
        *self.last_polled_input.lock() = now;
        *self.segment_start.lock() = Some(now);

        log::info!("idle: inactivity period dismissed — timer reset, back to active");
        Ok(())
    }

    /// Classifies a paused inactivity period as billable and credits
    /// the idle time to active_seconds.
    ///
    /// Mirrors the flow of `classify_tracking_inactivity_period` but
    /// operates from the `PausedInactivity` phase instead of
    /// `ResumePrompt`. Finalizes the period (calculates idle duration),
    /// marks it as classified, adds the duration to active_seconds,
    /// and transitions back to Active.
    pub fn classify_from_paused_inactivity(&self, conn: &Connection) -> AgentResult<()> {
        if *self.phase.lock() != TrackingInactivityPhase::PausedInactivity {
            return Ok(());
        }

        // Finalize the period — same logic as detect_input's PausedInactivity arm
        let total_discarded = if let Some(period_id) = self.pending_period_id.lock().clone() {
            let idle_started = self
                .inactivity_started_at
                .lock()
                .clone()
                .unwrap_or_else(|| self.last_input_wall_at.lock().clone());
            let previous = *self.inactivity_discarded_seconds.lock();
            let (total, prev) = finalize_inactivity_period_on_resume(
                conn,
                &period_id,
                &idle_started,
                previous,
            )?;
            let additional = total.saturating_sub(prev);
            if additional > 0 {
                *self.inactivity_discarded_seconds.lock() += additional;
            }

            // Classify as billable (offline work category)
            let (reclassify, duration) = classify_tracking_inactivity_period_record(
                conn,
                &period_id,
                "offline_work",
                total,
            )?;

            if reclassify {
                *self.inactivity_reclassified_seconds.lock() += duration;
                *self.inactivity_discarded_seconds.lock() =
                    self.inactivity_discarded_seconds.lock().saturating_sub(duration);
                *self.active_seconds.lock() += duration;
            }
            total
        } else {
            0
        };

        log::info!(
            "idle: inactivity period classified from paused — credited {total_discarded}s as billable"
        );
        self.finish_inactivity_resume_prompt();
        Ok(())
    }

    pub fn confirm_still_working(&self) -> AgentResult<()> {
        let phase = *self.phase.lock();
        if !matches!(phase, TrackingInactivityPhase::Warning | TrackingInactivityPhase::Countdown) {
            return Ok(());
        }
        let now = Instant::now();
        *self.last_input_at.lock() = now;
        *self.last_input_wall_at.lock() = chrono::Utc::now().to_rfc3339();
        *self.last_polled_input.lock() = now;
        self.cancel_inactivity_flow();
        if self.segment_start.lock().is_none() {
            *self.segment_start.lock() = Some(now);
        }
        Ok(())
    }

    pub fn classify_tracking_inactivity_period(
        &self,
        conn: &Connection,
        period_id: &str,
        category: &str,
    ) -> AgentResult<()> {
        if *self.phase.lock() != TrackingInactivityPhase::ResumePrompt {
            return Ok(());
        }

        let pending = self.pending_period_id.lock().clone();
        if pending.as_deref() != Some(period_id) {
            return Ok(());
        }

        let (reclassify, duration) = classify_tracking_inactivity_period_record(
            conn,
            period_id,
            category,
            *self.inactivity_discarded_seconds.lock(),
        )?;

        if reclassify {
            *self.inactivity_reclassified_seconds.lock() += duration;
            *self.inactivity_discarded_seconds.lock() =
                self.inactivity_discarded_seconds.lock().saturating_sub(duration);
            *self.active_seconds.lock() += duration;
        }

        self.finish_inactivity_resume_prompt();
        Ok(())
    }

    pub fn skip_tracking_inactivity_classification(&self, conn: &Connection) -> AgentResult<()> {
        if *self.phase.lock() != TrackingInactivityPhase::ResumePrompt {
            return Ok(());
        }

        if let Some(period_id) = self.pending_period_id.lock().clone() {
            discard_inactivity_period_record(conn, &period_id)?;
        }

        self.finish_inactivity_resume_prompt();
        Ok(())
    }

    pub fn snapshot(&self) -> TrackingInactivitySnapshot {
        let phase = *self.phase.lock();
        let now = Instant::now();
        let countdown_remaining = self.countdown_started_at.lock().map(|started| {
            COUNTDOWN_SECS.saturating_sub(now.duration_since(started).as_secs())
        });

        let away_seconds = if matches!(phase, TrackingInactivityPhase::PausedInactivity | TrackingInactivityPhase::ResumePrompt) {
            self.inactivity_started_at.lock().as_ref().map(|start| {
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

        TrackingInactivitySnapshot {
            phase,
            threshold_secs: self.threshold_secs,
            countdown_secs: COUNTDOWN_SECS,
            countdown_remaining_secs: if phase == TrackingInactivityPhase::Countdown {
                countdown_remaining
            } else {
                None
            },
            countdown_ends_at: if phase == TrackingInactivityPhase::Countdown {
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
            inactivity_started_at: self.inactivity_started_at.lock().clone(),
            paused_at: self.paused_at.lock().clone(),
            away_seconds,
            pending_period_id: self.pending_period_id.lock().clone(),
            meeting_exempt: *self.meeting_exempt.lock(),
            active_seconds: self.billable_seconds(),
            inactivity_discarded_seconds: *self.inactivity_discarded_seconds.lock(),
            inactivity_reclassified_seconds: *self.inactivity_reclassified_seconds.lock(),
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

        if phase == TrackingInactivityPhase::ManualPaused && streak_secs >= MANUAL_PAUSE_ACTIVITY_SECS {
            *self.phase.lock() = TrackingInactivityPhase::ManualWorkCheck;
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

        *self.phase.lock() = TrackingInactivityPhase::Active;
        *self.paused_at.lock() = None;
        *self.manual_input_streak_start.lock() = None;
        *self.segment_start.lock() = Some(now);
        *self.last_input_at.lock() = now;
        *self.last_input_wall_at.lock() = chrono::Utc::now().to_rfc3339();
        *self.last_polled_input.lock() = now;
        *self.last_manual_input_at.lock() = now;
    }

    fn detect_input(&self, conn: &Connection) -> AgentResult<()> {
        let current = *self.last_input_at.lock();
        let previous = *self.last_polled_input.lock();
        if current <= previous {
            return Ok(());
        }
        *self.last_polled_input.lock() = current;

        let phase = *self.phase.lock();
        match phase {
            TrackingInactivityPhase::Active => {}
            TrackingInactivityPhase::Warning | TrackingInactivityPhase::Countdown => {}
            TrackingInactivityPhase::ManualPaused | TrackingInactivityPhase::ManualWorkCheck => {
                self.track_manual_pause_input(current);
            }
            TrackingInactivityPhase::PausedInactivity => {
                if let Some(period_id) = self.pending_period_id.lock().clone() {
                    let idle_started = self
                        .inactivity_started_at
                        .lock()
                        .clone()
                        .or_else(|| Some(self.last_input_wall_at.lock().clone()))
                        .unwrap_or_default();
                    let previous = *self.inactivity_discarded_seconds.lock();
                    let (total, previous) = finalize_inactivity_period_on_resume(
                        conn,
                        &period_id,
                        &idle_started,
                        previous,
                    )?;
                    let additional = total.saturating_sub(previous);
                    if additional > 0 {
                        *self.inactivity_discarded_seconds.lock() += additional;
                    }
                }
                *self.phase.lock() = TrackingInactivityPhase::ResumePrompt;
            }
            TrackingInactivityPhase::ResumePrompt => {}
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

    fn enter_inactivity_warning(&self) {
        if *self.phase.lock() != TrackingInactivityPhase::Active {
            return;
        }
        *self.phase.lock() = TrackingInactivityPhase::Warning;
        *self.inactivity_started_at.lock() = Some(self.last_input_wall_at.lock().clone());
        log::info!("idle: warning — no input for {}s", self.threshold_secs);
    }

    fn start_inactivity_countdown(&self) {
        let phase = *self.phase.lock();
        if !matches!(phase, TrackingInactivityPhase::Active | TrackingInactivityPhase::Warning) {
            return;
        }
        if self.countdown_started_at.lock().is_some() {
            return;
        }
        *self.phase.lock() = TrackingInactivityPhase::Countdown;
        *self.countdown_started_at.lock() = Some(Instant::now());
        log::info!(
            "idle: alert — {}s countdown after {}s without input",
            COUNTDOWN_SECS,
            self.threshold_secs
        );
    }

    fn enter_inactivity_paused(
        &self,
        conn: &Connection,
        tracking_id: &str,
        _last_input: Instant,
        _now: Instant,
    ) -> AgentResult<()> {
        let frozen = self.billable_seconds();
        *self.active_seconds.lock() = frozen;
        *self.segment_start.lock() = None;

        let idle_started = self
            .inactivity_started_at
            .lock()
            .clone()
            .unwrap_or_else(|| self.last_input_wall_at.lock().clone());
        let paused_at = chrono::Utc::now().to_rfc3339();

        *self.phase.lock() = TrackingInactivityPhase::PausedInactivity;
        *self.paused_at.lock() = Some(paused_at.clone());

        let period_id = insert_paused_inactivity_period(
            conn,
            tracking_id,
            &idle_started,
            &paused_at,
            0,
        )?;
        *self.pending_period_id.lock() = Some(period_id);

        log::info!("idle: tracking continues — time counts as inactive");
        Ok(())
    }

    fn freeze_billable_at(&self) {
        let frozen_total = self.billable_seconds();
        *self.active_seconds.lock() = frozen_total;
        *self.segment_start.lock() = None;
    }

    pub fn reset_billable_seconds(&self) {
        *self.active_seconds.lock() = 0;
        *self.segment_start.lock() = None;
    }

    /// Restarts the billable segment timer after an external caller
    /// (e.g. persist_task_time_snapshot_state) cleared it via
    /// reset_billable_seconds(). Must only be called when the
    /// controller is in an active/billable phase.
    pub fn restart_segment_timer(&self) {
        *self.segment_start.lock() = Some(Instant::now());
    }

    fn cancel_inactivity_flow(&self) {
        *self.phase.lock() = TrackingInactivityPhase::Active;
        *self.countdown_started_at.lock() = None;
        *self.inactivity_started_at.lock() = None;
    }

    fn finish_inactivity_resume_prompt(&self) {
        let now = Instant::now();
        *self.phase.lock() = TrackingInactivityPhase::Active;
        *self.paused_at.lock() = None;
        *self.pending_period_id.lock() = None;
        *self.inactivity_started_at.lock() = None;
        *self.countdown_started_at.lock() = None;
        *self.last_input_at.lock() = now;
        *self.last_input_wall_at.lock() = chrono::Utc::now().to_rfc3339();
        *self.last_polled_input.lock() = now;
        *self.segment_start.lock() = Some(now);
    }
}

#[cfg(test)]
mod manual_pause_tests {
    use super::*;
    use parking_lot::Mutex;
    use std::sync::Arc;
    use std::time::Instant;

    fn test_controller() -> TrackingInactivityController {
        let last_input = Arc::new(Mutex::new(Instant::now()));
        let last_wall = Arc::new(Mutex::new(chrono::Utc::now().to_rfc3339()));
        TrackingInactivityController::new(120, last_input, last_wall)
    }

    #[test]
    fn pause_manually_from_active() {
        let ctrl = test_controller();
        ctrl.pause_manually();
        let snapshot = ctrl.snapshot();
        assert_eq!(snapshot.phase, TrackingInactivityPhase::ManualPaused);
        assert!(snapshot.paused_at.is_some());
    }

    #[test]
    fn resume_manually_restores_active() {
        let ctrl = test_controller();
        ctrl.pause_manually();
        ctrl.resume_manually();
        let snapshot = ctrl.snapshot();
        assert_eq!(snapshot.phase, TrackingInactivityPhase::Active);
        assert!(snapshot.paused_at.is_none());
    }

    #[test]
    fn dismiss_manual_work_check_returns_to_manual_paused() {
        let ctrl = test_controller();
        ctrl.pause_manually();
        *ctrl.phase.lock() = TrackingInactivityPhase::ManualWorkCheck;
        ctrl.dismiss_manual_work_check().unwrap();
        assert_eq!(ctrl.snapshot().phase, TrackingInactivityPhase::ManualPaused);
    }

    #[test]
    fn confirm_manual_work_restores_active() {
        let ctrl = test_controller();
        ctrl.pause_manually();
        *ctrl.phase.lock() = TrackingInactivityPhase::ManualWorkCheck;
        ctrl.confirm_manual_work().unwrap();
        assert_eq!(ctrl.snapshot().phase, TrackingInactivityPhase::Active);
    }
}
