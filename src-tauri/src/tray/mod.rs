mod actions;
mod i18n;
mod refresh;
mod state;

use crate::app_state::AppState;
use crate::icons::app_icon;
use crate::locale;
use crate::windows::show_main_window;
use i18n::tray_labels;
use state::TrayState;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

pub use actions::{handle_tray_menu_event, persist_last_selection};
pub use refresh::{refresh_tray_ui, refresh_tray_ui_sync, schedule_tray_refresh, spawn_refresh_loop};
pub use state::{
    SETTING_LAST_PROJECT_ID, SETTING_LAST_TASK_ID, SETTING_SELECTED_PROJECT_ID,
    SETTING_SELECTED_TASK_ID,
};

pub const EVENT_AUTH_LOGGED_OUT: &str = "auth-logged-out";
pub const TRAY_ID: &str = "main-tray";

pub fn setup_tray(app: &tauri::App, locale: &str) -> tauri::Result<()> {
    let app_handle = app.handle();
    let labels = tray_labels(locale);

    let status = MenuItem::with_id(
        app_handle,
        "tray_status",
        labels.status_idle,
        false,
        None::<&str>,
    )?;
    let toggle = MenuItem::with_id(
        app_handle,
        "toggle_tracking",
        labels.toggle_start,
        false,
        None::<&str>,
    )?;
    let show = MenuItem::with_id(app_handle, "show", labels.show, true, None::<&str>)?;
    let reset_widget_position = MenuItem::with_id(
        app_handle,
        "reset_widget_position",
        labels.reset_widget_position,
        true,
        None::<&str>,
    )?;
    let logout = MenuItem::with_id(app_handle, "logout", labels.logout, true, None::<&str>)?;
    let quit = MenuItem::with_id(app_handle, "quit", labels.quit, true, None::<&str>)?;

    let tray_state = TrayState {
        status,
        toggle,
        show,
        reset_widget_position,
        logout,
        quit,
        locale: parking_lot::Mutex::new(locale.to_string()),
    };
    app.manage(tray_state);

    let menu = build_tray_menu(app_handle, &labels, &app.state::<TrayState>())?;
    let labels = tray_labels(locale);

    let _tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(app_icon())
        .menu(&menu)
        .tooltip(labels.tooltip)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    refresh_tray_ui_sync(app_handle)?;

    Ok(())
}

pub fn refresh_tray_menu(app: &tauri::AppHandle, locale: &str) -> tauri::Result<()> {
    let tray_state = app.state::<TrayState>();
    *tray_state.locale.lock() = locale.to_string();

    let labels = tray_labels(locale);
    tray_state.show.set_text(labels.show)?;
    tray_state
        .reset_widget_position
        .set_text(labels.reset_widget_position)?;
    tray_state.logout.set_text(labels.logout)?;
    tray_state.quit.set_text(labels.quit)?;
    tray_state.toggle.set_text(labels.toggle_start)?;

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let menu = build_tray_menu(app, &labels, &tray_state)?;
        tray.set_menu(Some(menu))?;
    }

    refresh_tray_ui_sync(app)
}

fn build_tray_menu(
    app: &tauri::AppHandle,
    labels: &i18n::TrayLabels,
    tray_state: &TrayState,
) -> tauri::Result<Menu<tauri::Wry>> {
    let _ = labels;
    let separator_top = PredefinedMenuItem::separator(app)?;
    let separator_bottom = PredefinedMenuItem::separator(app)?;
    Menu::with_items(
        app,
        &[
            &tray_state.status,
            &tray_state.toggle,
            &separator_top,
            &tray_state.show,
            &tray_state.reset_widget_position,
            &tray_state.logout,
            &separator_bottom,
            &tray_state.quit,
        ],
    )
}

pub fn setup_tray_from_state(app: &tauri::App) -> tauri::Result<()> {
    let locale = app
        .try_state::<AppState>()
        .map(|state| {
            let db = state.db.lock();
            locale::effective_locale(&db)
        })
        .unwrap_or_else(locale::detect_system_locale);
    setup_tray(app, locale)
}
