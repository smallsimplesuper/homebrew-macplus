use plist::Value;
use std::path::Path;

use crate::utils::AppResult;

pub fn read_info_plist(app_path: &Path) -> AppResult<plist::Dictionary> {
    let plist_path = app_path.join("Contents/Info.plist");
    let val = Value::from_file(&plist_path)?;
    val.into_dictionary()
        .ok_or_else(|| crate::utils::AppError::Custom("Info.plist is not a dictionary".into()))
}

pub fn get_string(dict: &plist::Dictionary, key: &str) -> Option<String> {
    dict.get(key)?.as_string().map(String::from)
}

pub fn get_bool(dict: &plist::Dictionary, key: &str) -> Option<bool> {
    dict.get(key)?.as_boolean()
}
