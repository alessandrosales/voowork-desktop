use crate::app_focus::capture_active_window;
use crate::models::{PermissionCheck, TrackingCapabilities};

pub fn probe_tracking_capabilities() -> TrackingCapabilities {
    let input = probe_input_capture();
    let windows = probe_window_tracking();
    let screenshots = probe_screenshots();

    let mut notes = Vec::new();

    if !input.granted {
        notes.push(
            "Para monitorar mouse e teclado, peça ajuda ao administrador do seu computador para liberar as permissões necessárias. Depois, reinicie o app.".into(),
        );
    }

    if !windows.granted {
        notes.push(
            "Para registrar os aplicativos em uso, verifique as permissões de acesso às janelas nas configurações do sistema.".into(),
        );
    }

    if !screenshots.granted {
        notes.push(
            "Para capturas de tela, conceda permissão de gravação de tela ao Voowork nas configurações do sistema.".into(),
        );
    }

    if input.granted && windows.granted && screenshots.granted {
        notes.push("Tudo pronto — o monitoramento está funcionando neste computador.".into());
    }

    TrackingCapabilities {
        input_capture: input,
        window_tracking: windows,
        screenshots,
        notes,
    }
}

fn probe_input_capture() -> PermissionCheck {
    #[cfg(target_os = "linux")]
    {
        let in_group = linux_in_input_group();
        return PermissionCheck {
            granted: in_group,
            label: if in_group {
                "hardware".into()
            } else {
                "simulated".into()
            },
            action: None,
        };
    }

    #[cfg(not(target_os = "linux"))]
    {
        PermissionCheck {
            granted: true,
            label: "hardware".into(),
            action: None,
        }
    }
}

fn probe_window_tracking() -> PermissionCheck {
    let granted = capture_active_window().is_some();
    PermissionCheck {
        granted,
        label: if granted {
            "active".into()
        } else {
            "unavailable".into()
        },
        action: None,
    }
}

fn probe_screenshots() -> PermissionCheck {
    let granted = xcap::Monitor::all()
        .map(|m| !m.is_empty())
        .unwrap_or(false);
    PermissionCheck {
        granted,
        label: if granted {
            "ready".into()
        } else {
            "blocked".into()
        },
        action: None,
    }
}

#[cfg(target_os = "linux")]
fn linux_in_input_group() -> bool {
    let Ok(output) = std::process::Command::new("groups").output() else {
        return false;
    };
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .any(|group| group == "input")
}
