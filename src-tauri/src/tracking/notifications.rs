use crate::tracking_inactivity::TrackingInactivityPhase;
use tauri::{AppHandle, Manager};
#[cfg(not(target_os = "linux"))]
use tauri_plugin_notification::NotificationExt;

pub(crate) fn inactivity_notification_copy(locale: &str, phase: TrackingInactivityPhase) -> Option<(&'static str, &'static str)> {
    let lang = if locale.to_ascii_lowercase().starts_with("en") {
        "en"
    } else if locale.to_ascii_lowercase().starts_with("es") {
        "es"
    } else {
        "pt-BR"
    };

    match (lang, phase) {
        ("en", TrackingInactivityPhase::Warning) => Some((
            "Inactivity alert",
            "Confirm that you are still working to keep tracking time.",
        )),
        ("en", TrackingInactivityPhase::Countdown) => Some((
            "Inactivity alert",
            "Confirm within 60 seconds that you're still working.",
        )),
        ("en", TrackingInactivityPhase::PausedInactivity) => Some((
            "Session paused",
            "Your session was paused due to inactivity. Time is not being tracked.",
        )),
        ("en", TrackingInactivityPhase::ResumePrompt) => Some((
            "You're back",
            "The away period was not counted. Open the app to continue.",
        )),
        ("es", TrackingInactivityPhase::Warning) => Some((
            "Alerta de inactividad",
            "Confirma que sigues trabajando para mantener el tiempo registrado.",
        )),
        ("es", TrackingInactivityPhase::Countdown) => Some((
            "Alerta de inactividad",
            "Confirma en 60 segundos que sigues trabajando.",
        )),
        ("es", TrackingInactivityPhase::PausedInactivity) => Some((
            "Sesión pausada",
            "Tu sesión se pausó por inactividad. El tiempo no se está contabilizando.",
        )),
        ("es", TrackingInactivityPhase::ResumePrompt) => Some((
            "Has vuelto",
            "El período de ausencia no se contabilizó. Abre la app para continuar.",
        )),
        (_, TrackingInactivityPhase::Warning) => Some((
            "Alerta de inatividade",
            "Confirme que você ainda está trabalhando para manter o tempo registrado.",
        )),
        (_, TrackingInactivityPhase::Countdown) => Some((
            "Alerta de inatividade",
            "Confirme em até 60 segundos que você ainda está trabalhando.",
        )),
        (_, TrackingInactivityPhase::PausedInactivity) => Some((
            "Sessão pausada",
            "Sua sessão foi pausada por inatividade. O tempo não está sendo contabilizado.",
        )),
        (_, TrackingInactivityPhase::ResumePrompt) => Some((
            "Você voltou",
            "O período de ausência não foi contabilizado. Abra o app para continuar.",
        )),
        ("en", TrackingInactivityPhase::ManualWorkCheck) => Some((
            "Are you working?",
            "Activity detected while the timer is paused. Resume tracking?",
        )),
        ("es", TrackingInactivityPhase::ManualWorkCheck) => Some((
            "¿Estás trabajando?",
            "Actividad detectada con el temporizador pausado. ¿Reanudar el registro?",
        )),
        (_, TrackingInactivityPhase::ManualWorkCheck) => Some((
            "Você está trabalhando?",
            "Atividade detectada com o timer pausado. Deseja retomar o registro?",
        )),
        _ => None,
    }
}

pub(crate) fn send_inactivity_notification(app: &AppHandle, phase: TrackingInactivityPhase) {
    let locale = app
        .try_state::<crate::app_state::AppState>()
        .and_then(|state| state.db.lock().get_setting("locale").ok().flatten())
        .unwrap_or_else(|| "pt-BR".to_string());

    let Some((title, body)) = inactivity_notification_copy(&locale, phase) else {
        return;
    };

    let message = format!("{title} — {body}");

    #[cfg(target_os = "linux")]
    {
        std::thread::spawn(move || {
            if let Err(err) = notify_rust::Notification::new()
                .appname("Voowork")
                .summary("Voowork")
                .body(&message)
                .show()
            {
                log::warn!("idle notification failed: {err}");
            }
        });
    }

    #[cfg(not(target_os = "linux"))]
    {
        let app = app.clone();
        let _ = app.clone().run_on_main_thread(move || {
            if let Err(err) = app
                .notification()
                .builder()
                .title("Voowork")
                .body(message)
                .show()
            {
                log::warn!("idle notification failed: {err}");
            }
        });
    }
}
