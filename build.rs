use std::process::Command;
fn main() {
    let output = Command::new("git").args(&["rev-parse", "HEAD"]).output().unwrap_or("UNKNOWN");
    let git_hash = String::from_utf8(output.stdout).expect("Failed to parse string");
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);
}
