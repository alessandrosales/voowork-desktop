use crate::idle::IdlePhase;
use tauri::{AppHandle, Manager};
#[cfg(not(target_os = "linux"))]
use tauri_plugin_notification::NotificationExt;

pub(crate) fn idle_notification_copy(locale: &str, phase: IdlePhase) -> Option<(&'static str, &'static str)> {
    let lang = if locale.to_ascii_lowercase().starts_with("en") {
        "en"
    } else if locale.to_ascii_lowercase().starts_with("es") {
        "es"
    } else {
        "pt-BR"
    };

    match (lang, phase) {
        ("en", IdlePhase::Warning) => Some((
            "Inactivity alert",
            "Confirm that you are still working to keep tracking time.",
        )),
        ("en", IdlePhase::Countdown) => Some((
            "Inactivity alert",
            "Confirm within 60 seconds that you're still working.",
        )),
        ("en", IdlePhase::PausedIdle) => Some((
            "Session paused",
            "Your session was paused due to inactivity. Time is not being tracked.",
        )),
        ("en", IdlePhase::ResumePrompt) => Some((
            "You're back",
            "The away period was not counted. Open the app to continue.",
        )),
        ("es", IdlePhase::Warning) => Some((
            "Alerta de inactividad",
            "Confirma que sigues trabajando para mantener el tiempo registrado.",
        )),
        ("es", IdlePhase::Countdown) => Some((
            "Alerta de inactividad",
            "Confirma en 60 segundos que sigues trabajando.",
        )),
        ("es", IdlePhase::PausedIdle) => Some((
            "Sesión pausada",
            "Tu sesión se pausó por inactividad. El tiempo no se está contabilizando.",
        )),
        ("es", IdlePhase::ResumePrompt) => Some((
            "Has vuelto",
            "El período de ausencia no se contabilizó. Abre la app para continuar.",
        )),
        (_, IdlePhase::Warning) => Some((
            "Alerta de inatividade",
            "Confirme que você ainda está trabalhando para manter o tempo registrado.",
        )),
        (_, IdlePhase::Countdown) => Some((
            "Alerta de inatividade",
            "Confirme em até 60 segundos que você ainda está trabalhando.",
        )),
        (_, IdlePhase::PausedIdle) => Some((
            "Sessão pausada",
            "Sua sessão foi pausada por inatividade. O tempo não está sendo contabilizado.",
        )),
        (_, IdlePhase::ResumePrompt) => Some((
            "Você voltou",
            "O período de ausência não foi contabilizado. Abra o app para continuar.",
        )),
        ("en", IdlePhase::ManualWorkCheck) => Some((
            "Are you working?",
            "Activity detected while the timer is paused. Resume tracking?",
        )),
        ("es", IdlePhase::ManualWorkCheck) => Some((
            "¿Estás trabajando?",
            "Actividad detectada con el temporizador pausado. ¿Reanudar el registro?",
        )),
        (_, IdlePhase::ManualWorkCheck) => Some((
            "Você está trabalhando?",
            "Atividade detectada com o timer pausado. Deseja retomar o registro?",
        )),
        _ => None,
    }
}

pub(crate) fn send_idle_notification(app: &AppHandle, phase: IdlePhase) {
    let locale = app
        .try_state::<crate::app_state::AppState>()
        .and_then(|state| state.db.lock().get_setting("locale").ok().flatten())
        .unwrap_or_else(|| "pt-BR".to_string());

    let Some((title, body)) = idle_notification_copy(&locale, phase) else {
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
        return;
    }

    #[cfg(not(target_os = "linux"))]
    {
        let app = app.clone();
        let _ = app.run_on_main_thread(move || {
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
