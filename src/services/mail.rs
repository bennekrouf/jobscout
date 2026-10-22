//! A watch's results by email, to the licence's own email.
//!
//! api0 sends it: the app never sees an address or a mail server. The
//! licence is the key, and api0 only ever sends to the email it belongs to,
//! a few a day:
//!
//! ```text
//! POST https://gateway.api0.ai/v1/licence/email
//! Authorization: Bearer <licence>
//! { "app", "title", "intro", "items": [{ "title", "detail", "url" }] }
//! → 202 { "emailed": true, "to": "<the licence's email>" }
//! ```

use serde::Serialize;
use std::time::Duration;

use crate::model::{Country, Job};
use crate::services::licence;

/// The most jobs one email lists; api0 takes up to 50 lines.
pub const MAX_JOBS: usize = 30;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Item {
    pub title: String,
    pub detail: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Email {
    pub app: String,
    pub title: String,
    pub intro: String,
    pub items: Vec<Item>,
}

/// One line of text, within `max` characters.
fn line(s: &str, max: usize) -> String {
    let flat: String = s
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if flat.chars().count() <= max {
        flat
    } else {
        let cut: String = flat.chars().take(max - 1).collect();
        format!("{}…", cut.trim_end())
    }
}

/// What a watch run found: its matches when the jobs were scored, else every
/// new job. `jobs` comes in the order to list them.
pub fn watch_email(watch: &str, country: Country, jobs: &[Job], scored: bool) -> Email {
    let n = jobs.len();
    let what = match (scored, n) {
        (true, 1) => "1 good match".to_string(),
        (true, n) => format!("{n} good matches"),
        (false, 1) => "1 new job".to_string(),
        (false, n) => format!("{n} new jobs"),
    };
    let mut intro = format!(
        "Your watch “{}” in {} found {what}.",
        line(watch, 80),
        country.label()
    );
    if n > MAX_JOBS {
        intro.push_str(&format!(" The first {MAX_JOBS} are below; all of them are in JobScout."));
    }
    let items = jobs
        .iter()
        .take(MAX_JOBS)
        .map(|j| {
            let mut detail: Vec<String> = vec![];
            for part in [&j.company, &j.location] {
                if !part.trim().is_empty() {
                    detail.push(part.trim().to_string());
                }
            }
            if let Some(s) = &j.score {
                detail.push(format!("{}/100 — {}", s.score, s.verdict.trim()));
            }
            Item {
                title: line(&j.title, 200),
                detail: line(&detail.join(" · "), 500),
                url: if j.url.starts_with("https://") {
                    j.url.clone()
                } else {
                    String::new()
                },
            }
        })
        .collect();
    Email {
        app: "JobScout".into(),
        title: line(&format!("{what} for “{watch}”"), 150),
        intro,
        items,
    }
}

/// Sends `email` to the licence's email; returns the address it went to.
pub async fn send(email: &Email) -> Result<String, String> {
    let key = licence::saved_key()
        .ok_or("no licence on this computer: paste one in the Pro window to get emails")?;
    let resp = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap_or_default()
        .post(format!("{}/v1/licence/email", licence::GATEWAY))
        .bearer_auth(key)
        .header("X-Client-App", licence::PRODUCT)
        .json(email)
        .send()
        .await
        .map_err(|e| format!("email not sent ({e})"))?;
    let status = resp.status().as_u16();
    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    match status {
        200..=299 => Ok(body["to"].as_str().unwrap_or("your email").to_string()),
        401 => Err("email not sent: the licence isn't known".into()),
        402 => Err("email not sent: the licence's subscription isn't active".into()),
        429 => Err("email not sent: today's emails are used".into()),
        _ => Err(format!(
            "email not sent ({})",
            body["error"]["message"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("api0 answered {status}"))
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Score, Source, Status};
    use chrono::Utc;

    fn job(title: &str, url: &str, score: Option<u8>) -> Job {
        Job {
            id: title.into(),
            source: Source::JobsCh,
            title: title.into(),
            company: "Acme".into(),
            location: "Zurich".into(),
            url: url.into(),
            posted: None,
            salary: None,
            found_at: Utc::now(),
            query: String::new(),
            status: Status::New,
            detail: None,
            detail_failed_at: None,
            score: score.map(|score| Score {
                score,
                verdict: "Strong Rust fit".into(),
                strengths: vec![],
                gaps: vec![],
                scored_at: Utc::now(),
            }),
            notified: false,
        }
    }

    #[test]
    fn a_watch_email_lists_matches_with_score_and_link() {
        let jobs = [job("Rust developer", "https://www.jobs.ch/en/vacancies/detail/1/", Some(86))];
        let e = watch_email("rust developer", Country::Switzerland, &jobs, true);
        assert_eq!(e.title, "1 good match for “rust developer”");
        assert_eq!(e.intro, "Your watch “rust developer” in Switzerland found 1 good match.");
        assert_eq!(e.items[0].detail, "Acme · Zurich · 86/100 — Strong Rust fit");
        assert_eq!(e.items[0].url, "https://www.jobs.ch/en/vacancies/detail/1/");
    }

    #[test]
    fn unscored_runs_list_new_jobs_and_long_ones_are_cut() {
        let jobs: Vec<Job> = (0..MAX_JOBS + 5)
            .map(|i| job(&format!("Job {i}\n  title"), "http://insecure.example", None))
            .collect();
        let e = watch_email("data", Country::Germany, &jobs, false);
        assert_eq!(e.items.len(), MAX_JOBS);
        assert!(e.title.starts_with("35 new jobs"));
        assert!(e.intro.contains("first 30"));
        assert_eq!(e.items[0].title, "Job 0 title");
        // Only https links go out; api0 refuses anything else.
        assert_eq!(e.items[0].url, "");
        assert_eq!(e.items[0].detail, "Acme · Zurich");
    }
}
