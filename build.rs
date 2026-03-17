//! Build script for litellm-rs Gateway
//!
//! Sets up build-time environment variables and metadata.

use std::process::Command;

fn main() {
    // Set build timestamp using system time
    let build_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    println!("cargo:rustc-env=BUILD_TIME={}", build_time);

    // Get Git hash
    let git_hash = get_git_hash().unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);

    // Get Rust version
    let rust_version = get_rust_version().unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=RUST_VERSION={}", rust_version);

    // Set rerun conditions
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=Cargo.toml");
}

/// Get the current Git commit hash
fn get_git_hash() -> Option<String> {
    // Check if we're in a CI/CD environment without git
    if std::env::var("DOCS_RS").is_ok() {
        return Some("docs-rs-build".to_string());
    }

    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;

    if output.status.success() {
        let hash = String::from_utf8(output.stdout).ok()?;
        Some(hash.trim().to_string())
    } else {
        None
    }
}

/// Get the Rust version used for compilation
fn get_rust_version() -> Option<String> {
    // Check if we're in a CI/CD environment
    if std::env::var("DOCS_RS").is_ok() {
        return Some("stable".to_string());
    }

    let output = Command::new("rustc").args(["--version"]).output().ok()?;

    if output.status.success() {
        let version = String::from_utf8(output.stdout).ok()?;
        Some(version.trim().to_string())
    } else {
        None
    }
}
