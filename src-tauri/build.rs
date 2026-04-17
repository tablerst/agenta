use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    emit_git_build_metadata();
    tauri_build::build()
}

fn emit_git_build_metadata() {
    println!("cargo:rerun-if-env-changed=AGENTA_BUILD_FORCE_RERUN");

    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string()));

    emit_git_rerun_hints(&manifest_dir);

    let git_commit = git_output(&manifest_dir, &["rev-parse", "HEAD"]).unwrap_or_default();
    let git_commit_short =
        git_output(&manifest_dir, &["rev-parse", "--short", "HEAD"]).unwrap_or_default();
    let git_describe = git_output(
        &manifest_dir,
        &["describe", "--tags", "--always", "--dirty"],
    )
    .unwrap_or_default();
    let git_dirty = git_output(&manifest_dir, &["status", "--porcelain"])
        .map(|output| !output.is_empty())
        .unwrap_or(false);

    println!("cargo:rustc-env=AGENTA_BUILD_GIT_COMMIT={git_commit}");
    println!("cargo:rustc-env=AGENTA_BUILD_GIT_COMMIT_SHORT={git_commit_short}");
    println!("cargo:rustc-env=AGENTA_BUILD_GIT_DESCRIBE={git_describe}");
    println!("cargo:rustc-env=AGENTA_BUILD_GIT_DIRTY={git_dirty}");
}

fn emit_git_rerun_hints(manifest_dir: &Path) {
    let git_dir = manifest_dir.join("..").join(".git");
    if !git_dir.exists() {
        return;
    }

    println!("cargo:rerun-if-changed={}", git_dir.join("HEAD").display());
    println!("cargo:rerun-if-changed={}", git_dir.join("index").display());
    println!(
        "cargo:rerun-if-changed={}",
        git_dir.join("packed-refs").display()
    );

    if let Ok(head) = std::fs::read_to_string(git_dir.join("HEAD")) {
        if let Some(reference) = head.strip_prefix("ref: ") {
            println!(
                "cargo:rerun-if-changed={}",
                git_dir.join(reference.trim()).display()
            );
        }
    }
}

fn git_output(working_dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(working_dir)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
