use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub check_interval_minutes: u32,
    pub launch_at_login: bool,
    pub show_menu_bar_icon: bool,
    pub notification_on_updates: bool,
    pub auto_check_on_launch: bool,
    pub theme: ThemeMode,
    pub ignored_bundle_ids: Vec<String>,
    pub scan_locations: Vec<String>,
    pub scan_depth: u32,
    pub show_badge_count: bool,
    pub notification_sound: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            check_interval_minutes: 60,
            launch_at_login: false,
            show_menu_bar_icon: true,
            notification_on_updates: true,
            auto_check_on_launch: true,
            theme: ThemeMode::System,
            ignored_bundle_ids: Vec::new(),
            scan_locations: vec![
                "/Applications".into(),
                "~/Applications".into(),
            ],
            scan_depth: 2,
            show_badge_count: true,
            notification_sound: true,
        }
    }
}
