use crate::idle::IdlePhase;
use tauri::{AppHandle, Emitter, Manager};

use super::notifications::send_idle_notification;

fn is_idle_alert_phase(phase: IdlePhase) -> bool {
    matches!(
        phase,
        IdlePhase::Warning
            | IdlePhase::Countdown
            | IdlePhase::PausedIdle
            | IdlePhase::ResumePrompt
            | IdlePhase::ManualWorkCheck
    )
}

pub(crate) fn handle_idle_phase_transition(app: &AppHandle, before: IdlePhase, after: IdlePhase) {
    let was_alert = is_idle_alert_phase(before);
    let is_alert = is_idle_alert_phase(after);

    let _ = app.emit("idle-changed", ());

    if is_alert && !was_alert {
        present_idle_alert_window(app);
    }

    if is_alert && after != before {
        send_idle_notification(app, after);
    }

    if after == IdlePhase::Active && was_alert {
        release_idle_alert_window(app);
    }
}

fn present_idle_alert_window(app: &AppHandle) {
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        let Some(window) = app.get_webview_window("main") else {
            return;
        };

        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_always_on_top(true);
    });
}

fn release_idle_alert_window(app: &AppHandle) {
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        let Some(window) = app.get_webview_window("main") else {
            return;
        };

        let _ = window.set_always_on_top(false);
    });
}
