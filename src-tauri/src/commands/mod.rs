mod dashboard;
mod navigation;
mod platform;
mod projects;
mod settings;
mod tracking;

pub use dashboard::{
    get_activity_chart, get_dashboard_summary, get_tracking_screenshot_image, list_tracking_inactivity_periods,
    list_sync_queue, list_tracking_apps, list_tracking_peripheral_events,
    list_tracking_screenshots, list_tracking_sites, list_trackings,
};
pub use navigation::{
    open_external_url, open_system_settings_input_monitoring,
    open_system_settings_screen_recording, open_web_panel,
};
pub use platform::get_platform_info;
pub use projects::{list_projects, sync_projects};
pub use tracking::{
    check_active_window_permission, check_input_monitoring_permission,
    classify_paused_inactivity_period, classify_tracking_inactivity_period, confirm_manual_work,
    confirm_still_working, dismiss_activity_buffer, dismiss_inactivity_period, dismiss_manual_work_check, get_app_status,
    get_tracking_inactivity_config, get_task_elapsed_seconds, get_tracking_status, pause_tracking,
    restart_tracking, resume_tracking, skip_tracking_inactivity_classification, start_tracking,
    stop_tracking,
};
pub use settings::{
    get_app_version, get_setting, get_tracking_capabilities, get_tracking_config, open_data_directory,
    set_setting,
};
