//! On-disk state: one JSON file per concern under the platform data directory
//! (~/Library/Application Support/JobScout on macOS). Writes are best-effort
//! and atomic; a missing or unreadable file reads as the default.

use std::path::PathBuf;

use serde::{de::DeserializeOwned, Serialize};

use crate::model::{Job, Settings, Watch};

pub fn data_dir() -> PathBuf {
    let dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("JobScout");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn load<T: DeserializeOwned + Default>(name: &str) -> T {
    std::fs::read_to_string(data_dir().join(name))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save<T: Serialize + ?Sized>(name: &str, value: &T) {
    let path = data_dir().join(name);
    let tmp = path.with_extension("tmp");
    if let Ok(json) = serde_json::to_string_pretty(value) {
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

pub fn load_jobs() -> Vec<Job> {
    load("jobs.json")
}

pub fn save_jobs(jobs: &[Job]) {
    save("jobs.json", jobs)
}

pub fn load_settings() -> Settings {
    load("settings.json")
}

pub fn save_settings(settings: &Settings) {
    save("settings.json", settings)
}

pub fn load_watches() -> Vec<Watch> {
    load("watches.json")
}

pub fn save_watches(watches: &[Watch]) {
    save("watches.json", watches)
}

pub fn load_profile() -> String {
    std::fs::read_to_string(data_dir().join("profile.md")).unwrap_or_default()
}

pub fn save_profile(text: &str) {
    let _ = std::fs::write(data_dir().join("profile.md"), text);
}

/// CSV of the given jobs, for a spreadsheet.
pub fn to_csv(jobs: &[&Job]) -> String {
    fn cell(s: &str) -> String {
        format!("\"{}\"", s.replace('"', "\"\""))
    }
    let mut out = String::from("score,status,source,title,company,location,posted,url,verdict\n");
    for j in jobs {
        let score = j.score.as_ref().map(|s| s.score.to_string()).unwrap_or_default();
        let verdict = j.score.as_ref().map(|s| s.verdict.as_str()).unwrap_or_default();
        let row = [
            score.as_str(),
            j.status.label(),
            j.source.label(),
            &j.title,
            &j.company,
            &j.location,
            j.posted.as_deref().unwrap_or_default(),
            &j.url,
            verdict,
        ];
        out.push_str(&row.map(cell).join(","));
        out.push('\n');
    }
    out
}
