use std::process::Command;

/// Bakes the short git hash into the binary (`GIT_HASH`) for the log's start line.
/// No git, no hash: the build still works.
fn main() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");
    let hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|hash| !hash.is_empty());
    if let Some(hash) = hash {
        println!("cargo:rustc-env=GIT_HASH={hash}");
    }
}
