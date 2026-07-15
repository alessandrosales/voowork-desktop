use crate::tracking_inactivity::TrackingInactivityPhase;
use crate::windows::{show_main_window, show_mini_timer};
use tauri::{AppHandle, Emitter, Manager};

use super::notifications::send_inactivity_notification;

fn is_inactivity_alert_phase(phase: TrackingInactivityPhase) -> bool {
    matches!(
        phase,
        TrackingInactivityPhase::Warning
            | TrackingInactivityPhase::Countdown
            | TrackingInactivityPhase::PausedInactivity
            | TrackingInactivityPhase::ResumePrompt
            | TrackingInactivityPhase::ManualWorkCheck
    )
}

pub(crate) fn handle_inactivity_phase_transition(app: &AppHandle, before: TrackingInactivityPhase, after: TrackingInactivityPhase) {
    let was_alert = is_inactivity_alert_phase(before);
    let is_alert = is_inactivity_alert_phase(after);

    let _ = app.emit("tracking-inactivity-changed", ());

    if is_alert && !was_alert {
        present_inactivity_alert_window(app);
    }

    if is_alert && after != before {
        send_inactivity_notification(app, after);
    }

    if after == TrackingInactivityPhase::Active && was_alert {
        release_inactivity_alert_window(app);
    }
}

fn present_inactivity_alert_window(app: &AppHandle) {
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        show_main_window(&app);
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.set_always_on_top(true);
        }
    });
}

fn release_inactivity_alert_window(app: &AppHandle) {
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        let main_visible = app
            .get_webview_window("main")
            .and_then(|window| window.is_visible().ok())
            .unwrap_or(false);

        if let Some(window) = app.get_webview_window("main") {
            let _ = window.set_always_on_top(false);
        }

        if !main_visible {
            let _ = show_mini_timer(&app);
        }
    });
}
