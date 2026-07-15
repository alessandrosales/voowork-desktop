use crate::locale::resolve_locale;

pub struct TrayLabels {
    pub show: &'static str,
    pub logout: &'static str,
    pub quit: &'static str,
    pub tooltip: &'static str,
    pub status_idle: &'static str,
    pub status_signed_out: &'static str,
    pub toggle_start: &'static str,
    pub toggle_pause: &'static str,
    pub toggle_resume: &'static str,
    pub toggle_open: &'static str,
    pub reset_widget_position: &'static str,
}

pub struct TrayToggleLabels {
    pub text: &'static str,
    pub enabled: bool,
}

pub fn tray_labels(locale: &str) -> TrayLabels {
    match resolve_locale(locale).unwrap_or("pt-BR") {
        "en" => TrayLabels {
            show: "Open Voowork",
            logout: "Sign out",
            quit: "Quit",
            tooltip: "Voowork",
            status_idle: "No active session",
            status_signed_out: "Sign in to start",
            toggle_start: "▶ Start",
            toggle_pause: "⏸ Pause",
            toggle_resume: "▶ Resume",
            toggle_open: "Open Voowork",
            reset_widget_position: "Reset timer position",
        },
        "es" => TrayLabels {
            show: "Abrir Voowork",
            logout: "Cerrar sesión",
            quit: "Salir",
            tooltip: "Voowork",
            status_idle: "Sin sesión activa",
            status_signed_out: "Inicia sesión para comenzar",
            toggle_start: "▶ Iniciar",
            toggle_pause: "⏸ Pausar",
            toggle_resume: "▶ Reanudar",
            toggle_open: "Abrir Voowork",
            reset_widget_position: "Restablecer posición del timer",
        },
        _ => TrayLabels {
            show: "Abrir Voowork",
            logout: "Fazer logout",
            quit: "Sair",
            tooltip: "Voowork",
            status_idle: "Nenhuma task em andamento",
            status_signed_out: "Faça login para iniciar",
            toggle_start: "▶ Iniciar",
            toggle_pause: "⏸ Pausar",
            toggle_resume: "▶ Retomar",
            toggle_open: "Abrir Voowork",
            reset_widget_position: "Reposicionar timer",
        },
    }
}
