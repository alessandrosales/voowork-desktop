use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::app_state::AppState;
use crate::auth::KEY_AUTHENTICATED;
use tauri::{AppHandle, Manager};

use super::actions::selected_selection;
use super::i18n::{tray_labels, TrayToggleLabels};
use super::state::TrayState;
use super::TRAY_ID;

static TRAY_REFRESH_RUNNING: AtomicBool = AtomicBool::new(true);
static TRAY_REFRESH_SCHEDULED: AtomicBool = AtomicBool::new(false);

pub fn request_shutdown() {
    TRAY_REFRESH_RUNNING.store(false, Ordering::SeqCst);
}

pub fn schedule_tray_refresh(app: AppHandle) {
    if !TRAY_REFRESH_RUNNING.load(Ordering::SeqCst) {
        return;
    }
    if TRAY_REFRESH_SCHEDULED.swap(true, Ordering::SeqCst) {
        return;
    }

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(32));
        TRAY_REFRESH_SCHEDULED.store(false, Ordering::SeqCst);
        if !TRAY_REFRESH_RUNNING.load(Ordering::SeqCst) {
            return;
        }
        if let Err(err) = refresh_tray_ui_sync(&app) {
            log::debug!("deferred tray refresh failed: {err}");
        }
    });
}

pub fn spawn_refresh_loop(app: AppHandle) {
    std::thread::spawn(move || {
        while TRAY_REFRESH_RUNNING.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(1));
            if !TRAY_REFRESH_RUNNING.load(Ordering::SeqCst) {
                break;
            }
            let _ = refresh_tray_ui(&app);
        }
    });
}

pub fn refresh_tray_ui(app: &AppHandle) -> tauri::Result<()> {
    if !TRAY_REFRESH_RUNNING.load(Ordering::SeqCst) {
        return Ok(());
    }

    let handle = app.clone();
    let handle_for_closure = handle.clone();
    handle.run_on_main_thread(move || {
        if let Err(err) = refresh_tray_ui_sync(&handle_for_closure) {
            log::debug!("tray refresh failed: {err}");
        }
    })
}

pub fn refresh_tray_ui_sync(app: &AppHandle) -> tauri::Result<()> {
    let Some(tray_state) = app.try_state::<TrayState>() else {
        return Ok(());
    };
    let Some(app_state) = app.try_state::<AppState>() else {
        return Ok(());
    };
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };

    let locale = tray_state.locale.lock().clone();
    let labels = tray_labels(&locale);
    let authenticated = {
        let db = app_state.db.lock();
        db.get_setting(KEY_AUTHENTICATED)
            .ok()
            .flatten()
            .is_some_and(|value| value == "true")
    };

    let tracking = app_state.tracking_manager.status();
    let (status_text, tooltip, toggle) = if !authenticated {
        (
            labels.status_signed_out.to_string(),
            labels.tooltip.to_string(),
            TrayToggleLabels {
                text: labels.toggle_start,
                enabled: false,
            },
        )
    } else if tracking.active {
        let subtitle = resolve_subtitle(&app_state, &tracking.project_id, &tracking.task_id);
        let elapsed = format_elapsed(tracking.elapsed_seconds);
        let status = match &subtitle {
            Some(name) => format!("{elapsed} · {name}"),
            None => elapsed.clone(),
        };
        let tooltip = match &subtitle {
            Some(name) => format!("{} · {name} · {elapsed}", labels.tooltip),
            None => format!("{} · {elapsed}", labels.tooltip),
        };
        let toggle = toggle_labels_for_active(&labels, tracking.inactivity.phase.as_str());
        (status, tooltip, toggle)
    } else {
        let (elapsed, subtitle) = idle_display(&app_state)?;
        let status = match subtitle {
            Some(name) => format!("{elapsed} · {name}"),
            None => labels.status_idle.to_string(),
        };
        let has_last = {
            let db = app_state.db.lock();
            selected_selection(&db)
                .ok()
                .is_some_and(|(project_id, task_id)| project_id.is_some() && task_id.is_some())
        };
        (
            status,
            format!("{} · {elapsed}", labels.tooltip),
            TrayToggleLabels {
                text: labels.toggle_start,
                enabled: has_last,
            },
        )
    };

    tray_state.status.set_text(&status_text)?;
    tray_state.toggle.set_text(toggle.text)?;
    tray_state.toggle.set_enabled(toggle.enabled)?;
    tray.set_tooltip(Some(tooltip))?;

    Ok(())
}

fn idle_display(state: &AppState) -> tauri::Result<(String, Option<String>)> {
    let db = state.db.lock();
    let (project_id, task_id) = selected_selection(&db).unwrap_or((None, None));
    drop(db);

    let Some(task_id) = task_id else {
        return Ok((format_elapsed(0), None));
    };
    let seconds = state
        .tracking_manager
        .task_elapsed_seconds(&task_id)
        .unwrap_or(0);
    let task_ref = Some(task_id);
    let subtitle = resolve_subtitle(state, &project_id, &task_ref);
    Ok((format_elapsed(seconds), subtitle))
}

fn resolve_subtitle(
    state: &AppState,
    project_id: &Option<String>,
    task_id: &Option<String>,
) -> Option<String> {
    let project_id = project_id.as_deref()?;
    let task_id = task_id.as_deref()?;
    let db = state.db.lock();
    let task_name = db
        .task_name(project_id, task_id)
        .ok()
        .flatten()
        .filter(|name| !name.is_empty());
    if let Some(name) = task_name {
        return Some(name);
    }
    db.project_name(project_id).ok()
}

fn toggle_labels_for_active(labels: &super::i18n::TrayLabels, phase: &str) -> TrayToggleLabels {
    if matches!(
        phase,
        "warning" | "countdown" | "paused_inactivity" | "resume_prompt" | "manual_work_check"
    ) {
        return TrayToggleLabels {
            text: labels.toggle_open,
            enabled: true,
        };
    }

    if phase == "manual_paused" {
        TrayToggleLabels {
            text: labels.toggle_resume,
            enabled: true,
        }
    } else {
        TrayToggleLabels {
            text: labels.toggle_pause,
            enabled: true,
        }
    }
}

pub fn format_elapsed(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    format!("{hours:02}:{minutes:02}:{secs:02}")
}

#[cfg(test)]
mod tests {
    use super::format_elapsed;

    #[test]
    fn format_elapsed_pads_segments() {
        assert_eq!(format_elapsed(0), "00:00:00");
        assert_eq!(format_elapsed(3661), "01:01:01");
    }
}
