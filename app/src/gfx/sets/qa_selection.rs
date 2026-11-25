//! Test/export selection accepts a D: JSON key list to avoid Windows environment limits.
use std::collections::HashSet;
pub(super) fn selected() -> Option<HashSet<String>> {
    if let Some(path)=std::env::var_os("CWX_QA_SET_KEYS_FILE") {
        let raw=std::fs::read_to_string(path).expect("Read QA JSON key list");
        let keys:Vec<String>=serde_json::from_str(raw.trim_start_matches('\u{feff}')).expect("QA key list must be a JSON string array");
        return Some(keys.into_iter().collect());
    }
    std::env::var("CWX_QA_SET_KEYS").ok().map(|keys|keys.split(',').map(str::trim).filter(|k|!k.is_empty()).map(str::to_string).collect())
}
