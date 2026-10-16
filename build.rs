//! Build script — the release date a Pro licence is checked against.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // What a Pro licence's `updates_until` is compared with: JOBSCOUT_RELEASE_DATE
    // if set, else the date of the commit being built, else empty (unknown:
    // every licence covers it). The public key is read with `option_env!`;
    // listed here so changing it rebuilds.
    println!("cargo:rerun-if-env-changed=JOBSCOUT_RELEASE_DATE");
    println!("cargo:rerun-if-env-changed=JOBSCOUT_LICENSE_PUBLIC_KEY");
    let date = std::env::var("JOBSCOUT_RELEASE_DATE")
        .ok()
        .filter(|d| !d.is_empty())
        .or_else(commit_date);
    println!(
        "cargo:rustc-env=JOBSCOUT_RELEASE_DATE={}",
        date.unwrap_or_default()
    );
}

fn commit_date() -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["log", "-1", "--format=%cs"])
        .output()
        .ok()?;
    let date = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && date.len() == 10).then_some(date)
}
