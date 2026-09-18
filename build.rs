use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    println!("cargo:rerun-if-env-changed=ZIRAL_BUILD_TAG");
    if let Ok(build) = std::env::var("ZIRAL_BUILD_TAG") {
        println!("cargo:rustc-env=ZIRAL_BUILD_TAG={build}");
        return;
    }
    if let Some(path) = git(&["rev-parse", "--git-path", "HEAD"]) {
        println!("cargo:rerun-if-changed={path}");
    }
    if let Some(head) = git(&["rev-parse", "--symbolic-full-name", "HEAD"])
        && head != "HEAD"
        && let Some(path) = git(&["rev-parse", "--git-path", &head])
    {
        println!("cargo:rerun-if-changed={path}");
    }
    if let Some(build) = git(&["rev-parse", "HEAD"]) {
        println!("cargo:rustc-env=ZIRAL_BUILD_TAG={build}");
    }
}
