//! Build script — generates version info from git and source content hash.
//!
//! Emits `cargo:rustc-env=BUILD_*` vars for use at compile time.
//! Re-runs whenever any `.rs` source file changes, so BUILD_SRC_HASH always
//! reflects the actual compiled code, not just the last git commit.

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let git_hash = git_short_hash();
    let git_timestamp = git_commit_timestamp();
    let build_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());

    // Hash all .rs source files — changes whenever code changes, even without a commit.
    let src_hash = compute_source_hash("src");

    println!("cargo:rustc-env=BUILD_GIT_HASH={}", git_hash);
    println!("cargo:rustc-env=BUILD_GIT_TIMESTAMP={}", git_timestamp);
    println!("cargo:rustc-env=BUILD_NUMBER={}", build_timestamp);
    println!("cargo:rustc-env=BUILD_PROFILE={}", profile);
    println!("cargo:rustc-env=BUILD_SRC_HASH={}", src_hash);

    // Re-run when git HEAD changes (new commit).
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads/");

    // Re-run when any source file changes.
    register_source_files("src");
}

// ── Git helpers ───────────────────────────────────────────────────────────────

fn git_short_hash() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            o.status
                .success()
                .then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn git_commit_timestamp() -> String {
    Command::new("git")
        .args(["log", "-1", "--format=%ci"])
        .output()
        .ok()
        .and_then(|o| {
            o.status
                .success()
                .then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

// ── Source hashing ────────────────────────────────────────────────────────────

/// Walk `dir`, hash the sorted contents of every `.rs` file with FNV-1a,
/// and return an 8-hex-char string like `"a3f0c1b2"`.
fn compute_source_hash(dir: &str) -> String {
    let mut paths: Vec<std::path::PathBuf> = collect_rs_files(dir);
    // Sort so the hash is deterministic regardless of filesystem order.
    paths.sort();

    let mut hash: u64 = FNV_OFFSET;
    for path in &paths {
        if let Ok(contents) = std::fs::read(path) {
            // Mix the file path into the hash so renames are detected.
            for b in path.to_string_lossy().as_bytes() {
                hash = fnv1a_step(hash, *b);
            }
            hash = fnv1a_step(hash, b':'); // separator
            for b in &contents {
                hash = fnv1a_step(hash, *b);
            }
        }
    }

    format!("{:08x}", hash & 0xffff_ffff)
}

/// Register every `.rs` file under `dir` with Cargo so the build re-runs
/// when any of them change.
fn register_source_files(dir: &str) {
    for path in collect_rs_files(dir) {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

fn collect_rs_files(dir: &str) -> Vec<std::path::PathBuf> {
    let mut result = Vec::new();
    collect_rs_recursive(std::path::Path::new(dir), &mut result);
    result
}

fn collect_rs_recursive(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_recursive(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

// ── FNV-1a (64-bit) ───────────────────────────────────────────────────────────

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[inline(always)]
fn fnv1a_step(hash: u64, byte: u8) -> u64 {
    (hash ^ byte as u64).wrapping_mul(FNV_PRIME)
}
