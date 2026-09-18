use std::process::Command;

fn git(args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .output()
        .expect("git is available");
    assert!(output.status.success(), "git metadata is available");
    String::from_utf8(output.stdout)
        .expect("git metadata is UTF-8")
        .trim()
        .to_owned()
}

fn main() {
    println!("cargo:rerun-if-env-changed=ZIRAL_BUILD_TAG");
    println!(
        "cargo:rerun-if-changed={}",
        git(&["rev-parse", "--git-path", "HEAD"])
    );
    let head = git(&["rev-parse", "--symbolic-full-name", "HEAD"]);
    if head != "HEAD" {
        println!(
            "cargo:rerun-if-changed={}",
            git(&["rev-parse", "--git-path", &head])
        );
    }
    let build = std::env::var("ZIRAL_BUILD_TAG").unwrap_or_else(|_| git(&["rev-parse", "HEAD"]));
    println!("cargo:rustc-env=ZIRAL_BUILD_TAG={build}");
}
