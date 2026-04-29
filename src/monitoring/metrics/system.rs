//! System metrics collection using sysinfo crate
//!
//! These functions provide real system monitoring when the metrics feature is enabled

#[cfg(feature = "metrics")]
use std::sync::LazyLock;
#[cfg(feature = "metrics")]
use sysinfo::{Disks, Networks, System};

// System metrics collection using sysinfo crate
// These functions provide real system monitoring when the metrics feature is enabled

#[cfg(feature = "metrics")]
static SYSTEM: LazyLock<parking_lot::Mutex<System>> =
    LazyLock::new(|| parking_lot::Mutex::new(System::new_all()));

#[cfg(feature = "metrics")]
static NETWORKS: LazyLock<parking_lot::Mutex<Networks>> =
    LazyLock::new(|| parking_lot::Mutex::new(Networks::new_with_refreshed_list()));

#[cfg(feature = "metrics")]
static DISKS: LazyLock<parking_lot::Mutex<Disks>> =
    LazyLock::new(|| parking_lot::Mutex::new(Disks::new_with_refreshed_list()));

#[cfg(feature = "metrics")]
pub(super) fn get_cpu_usage() -> f64 {
    let mut sys = SYSTEM.lock();
    sys.refresh_cpu_usage();
    sys.global_cpu_usage() as f64
}

#[cfg(not(feature = "metrics"))]
pub(super) fn get_cpu_usage() -> f64 {
    0.0
}

#[cfg(feature = "metrics")]
pub(super) fn get_memory_usage() -> u64 {
    let mut sys = SYSTEM.lock();
    sys.refresh_memory();
    sys.used_memory()
}

#[cfg(not(feature = "metrics"))]
pub(super) fn get_memory_usage() -> u64 {
    0
}

#[cfg(feature = "metrics")]
pub(super) fn get_disk_usage() -> u64 {
    let mut disks = DISKS.lock();
    disks.refresh_list();
    disks
        .iter()
        .map(|d| d.total_space() - d.available_space())
        .sum()
}

#[cfg(not(feature = "metrics"))]
pub(super) fn get_disk_usage() -> u64 {
    0
}

#[cfg(feature = "metrics")]
pub(super) fn get_network_bytes_in() -> u64 {
    let mut networks = NETWORKS.lock();
    networks.refresh();
    networks.values().map(|data| data.total_received()).sum()
}

#[cfg(not(feature = "metrics"))]
pub(super) fn get_network_bytes_in() -> u64 {
    0
}

#[cfg(feature = "metrics")]
pub(super) fn get_network_bytes_out() -> u64 {
    let mut networks = NETWORKS.lock();
    networks.refresh();
    networks.values().map(|data| data.total_transmitted()).sum()
}

#[cfg(not(feature = "metrics"))]
pub(super) fn get_network_bytes_out() -> u64 {
    0
}

pub(super) fn get_active_connections() -> u32 {
    // We currently don't have a cross-platform socket-connection counter wired
    // in this module. Return 0 (unknown) instead of a fabricated constant.
    0
}
