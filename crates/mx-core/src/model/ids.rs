use std::time::{SystemTime, UNIX_EPOCH};

use crate::text::base36;

pub fn micros_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_micros() as u64)
}

pub fn run_stamp(micros: u64) -> String {
    base36(micros)
}

pub fn tab_id(run: &str, seq: u64) -> String {
    format!("tab{run}_{seq}")
}

pub fn sidebar_tab_id(plugin_id: &str) -> String {
    format!("sidebar:{plugin_id}")
}

pub fn feature_id(micros: u64) -> String {
    format!("pj{}", base36(micros))
}

pub fn group_id(micros: u64) -> String {
    format!("gp{}", base36(micros))
}

pub fn launcher_id(micros: u64) -> String {
    format!("lch{micros}")
}

pub fn suggestion_id(bytes: [u8; 4]) -> String {
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("task_{hex}")
}
