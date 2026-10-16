//! Desktop notifications for watch matches.
//!
//! macOS attributes every notification to an installed app bundle. The
//! packaged app (`dx bundle`) claims its own id and notifies natively. An
//! unbundled binary (`cargo run`) has no bundle, and asking notify-rust to
//! notify anyway makes macOS show a "choose an application" dialog every
//! time, so there the notification goes through `osascript` instead and
//! appears as coming from Script Editor.

use std::sync::OnceLock;

use crate::model::Job;

/// Must match `[bundle] identifier` in Dioxus.toml.
#[cfg(target_os = "macos")]
const BUNDLE_ID: &str = "ch.mayorana.jobscout";

static NATIVE: OnceLock<bool> = OnceLock::new();

/// Call once at startup, before anything can raise a notification.
pub fn init() {
    let _ = NATIVE.set(claim_application());
}

#[cfg(target_os = "macos")]
fn claim_application() -> bool {
    notify_rust::set_application(BUNDLE_ID).is_ok()
}

#[cfg(not(target_os = "macos"))]
fn claim_application() -> bool {
    true
}

/// Raises a notification off the async runtime: a slow notification daemon
/// must never stall the executor. Best-effort; a failure is only logged.
pub fn send(title: String, body: String) {
    let native = *NATIVE.get().unwrap_or(&false);
    std::thread::spawn(move || {
        let result = if native {
            notify_rust::Notification::new()
                .summary(&title)
                .body(&body)
                .show()
                .map(|_| ())
                .map_err(|e| e.to_string())
        } else {
            fallback(&title, &body)
        };
        if let Err(e) = result {
            eprintln!("notification failed: {e}");
        }
    });
}

#[cfg(target_os = "macos")]
fn fallback(title: &str, body: &str) -> Result<(), String> {
    // Title and body go in as arguments, never spliced into the script, so
    // a job title cannot break out of the AppleScript string.
    std::process::Command::new("osascript")
        .args([
            "-e",
            "on run argv",
            "-e",
            "display notification (item 2 of argv) with title (item 1 of argv)",
            "-e",
            "end run",
            title,
            body,
        ])
        .output()
        .map_err(|e| e.to_string())
        .and_then(|o| {
            o.status
                .success()
                .then_some(())
                .ok_or_else(|| String::from_utf8_lossy(&o.stderr).into_owned())
        })
}

#[cfg(not(target_os = "macos"))]
fn fallback(_title: &str, _body: &str) -> Result<(), String> {
    Err("no notification backend".into())
}

/// Title and body for the matches of one watch run, best first. One match
/// gets its own title; several are listed, three at most.
pub fn matches_message(watch: &str, jobs: &[Job]) -> (String, String) {
    let score = |j: &Job| j.score.as_ref().map(|s| s.score).unwrap_or(0);
    match jobs {
        [] => (String::new(), String::new()),
        [j] => (
            format!("{} · {}", score(j), j.title),
            format!(
                "{} · {}{}",
                j.company,
                j.location,
                j.score
                    .as_ref()
                    .map(|s| format!(" — {}", s.verdict))
                    .unwrap_or_default()
            ),
        ),
        many => {
            let mut lines: Vec<String> = many
                .iter()
                .take(3)
                .map(|j| format!("{} {} @ {}", score(j), j.title, j.company))
                .collect();
            if many.len() > 3 {
                lines.push(format!("+{} more", many.len() - 3));
            }
            (
                format!("{} new matches for “{watch}”", many.len()),
                lines.join("\n"),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Score, Status};
    use chrono::Utc;

    fn job(title: &str, score: u8) -> Job {
        Job {
            id: title.into(),
            source: crate::model::Source::JobsCh,
            title: title.into(),
            company: "Acme".into(),
            location: "Zurich".into(),
            url: String::new(),
            posted: None,
            salary: None,
            found_at: Utc::now(),
            query: String::new(),
            status: Status::New,
            detail: None,
            detail_failed_at: None,
            score: Some(Score {
                score,
                verdict: "Strong fit".into(),
                strengths: vec![],
                gaps: vec![],
                scored_at: Utc::now(),
            }),
            notified: false,
        }
    }

    #[test]
    fn one_match_names_the_job() {
        let (t, b) = matches_message("rust · Switzerland", &[job("Rust Engineer", 88)]);
        assert_eq!(t, "88 · Rust Engineer");
        assert_eq!(b, "Acme · Zurich — Strong fit");
    }

    #[test]
    fn several_matches_are_listed_and_capped() {
        let jobs: Vec<Job> = (0..5).map(|i| job(&format!("Job {i}"), 90 - i)).collect();
        let (t, b) = matches_message("rust", &jobs);
        assert_eq!(t, "5 new matches for “rust”");
        assert_eq!(b.lines().count(), 4);
        assert!(b.ends_with("+2 more"));
    }
}
