//! On-disk state: one JSON file per concern under the platform data directory
//! (~/Library/Application Support/JobScout on macOS). Writes are best-effort
//! and atomic; a missing or unreadable file reads as the default.
//!
//! Settings are shared. Everything else belongs to one country and lives in
//! that country's folder (`ch/`, `sg/`): jobs, watches, profile, last search.

use std::path::{Path, PathBuf};

use serde::{de::DeserializeOwned, Serialize};

use crate::model::{Country, Job, SearchParams, Settings, Watch};

/// Files kept per country.
const COUNTRY_FILES: [&str; 4] = ["jobs.json", "watches.json", "profile.md", "search.json"];

pub fn data_dir() -> PathBuf {
    let dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("JobScout");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn country_dir(country: Country) -> PathBuf {
    let dir = data_dir().join(country.code());
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn load<T: DeserializeOwned>(path: &Path) -> Option<T> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn save<T: Serialize + ?Sized>(path: &Path, value: &T) {
    let tmp = path.with_extension("tmp");
    if let Ok(json) = serde_json::to_string_pretty(value) {
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }
}

/// Before countries, everything sat in the data directory itself and was
/// all Swiss. Moves those files into `ch/`, once: a file already in `ch/`
/// is never overwritten. The last search came out of settings.json.
pub fn migrate() {
    migrate_in(&data_dir());
}

fn migrate_in(root: &Path) {
    let ch = root.join(Country::Switzerland.code());
    let _ = std::fs::create_dir_all(&ch);
    for name in COUNTRY_FILES {
        let (old, new) = (root.join(name), ch.join(name));
        if old.exists() && !new.exists() {
            let _ = std::fs::rename(&old, &new);
        }
    }
    let search = ch.join("search.json");
    if !search.exists() {
        let old: Option<serde_json::Value> = load(&root.join("settings.json"));
        if let Some(p) =
            old.and_then(|v| serde_json::from_value::<SearchParams>(v["search"].clone()).ok())
        {
            save(&search, &p);
        }
    }
}

pub fn load_jobs(country: Country) -> Vec<Job> {
    load(&country_dir(country).join("jobs.json")).unwrap_or_default()
}

pub fn save_jobs(country: Country, jobs: &[Job]) {
    save(&country_dir(country).join("jobs.json"), jobs)
}

pub fn load_settings() -> Settings {
    load(&data_dir().join("settings.json")).unwrap_or_default()
}

pub fn save_settings(settings: &Settings) {
    save(&data_dir().join("settings.json"), settings)
}

pub fn load_watches(country: Country) -> Vec<Watch> {
    load(&country_dir(country).join("watches.json")).unwrap_or_default()
}

pub fn save_watches(country: Country, watches: &[Watch]) {
    save(&country_dir(country).join("watches.json"), watches)
}

pub fn load_search(country: Country) -> SearchParams {
    let mut p: SearchParams = load(&country_dir(country).join("search.json"))
        .unwrap_or_else(|| SearchParams::for_country(country));
    // A source the country does not have (or no longer has) is dropped.
    p.sources.retain(|s| country.sources().contains(s));
    p
}

pub fn save_search(country: Country, p: &SearchParams) {
    save(&country_dir(country).join("search.json"), p)
}

pub fn load_profile(country: Country) -> String {
    std::fs::read_to_string(country_dir(country).join("profile.md")).unwrap_or_default()
}

pub fn save_profile(country: Country, text: &str) {
    let _ = std::fs::write(country_dir(country).join("profile.md"), text);
}

/// CSV of the given jobs, for a spreadsheet.
pub fn to_csv(jobs: &[&Job]) -> String {
    fn cell(s: &str) -> String {
        format!("\"{}\"", s.replace('"', "\"\""))
    }
    let mut out = String::from("score,status,source,title,company,location,posted,url,verdict\n");
    for j in jobs {
        let score = j
            .score
            .as_ref()
            .map(|s| s.score.to_string())
            .unwrap_or_default();
        let verdict = j
            .score
            .as_ref()
            .map(|s| s.verdict.as_str())
            .unwrap_or_default();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_moves_old_files_into_ch_once() {
        let root = std::env::temp_dir().join(format!("jobscout-migrate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("jobs.json"), "[]").unwrap();
        std::fs::write(root.join("profile.md"), "me").unwrap();
        std::fs::write(
            root.join("settings.json"),
            r#"{"search":{"keywords":"rust","location":"Zug"}}"#,
        )
        .unwrap();

        migrate_in(&root);
        let ch = root.join("ch");
        assert!(!root.join("jobs.json").exists());
        assert_eq!(
            std::fs::read_to_string(ch.join("profile.md")).unwrap(),
            "me"
        );
        let p: SearchParams = load(&ch.join("search.json")).unwrap();
        assert_eq!((p.keywords.as_str(), p.location.as_str()), ("rust", "Zug"));

        // A stray old file never replaces what is already in ch/.
        std::fs::write(root.join("profile.md"), "stale").unwrap();
        migrate_in(&root);
        assert_eq!(
            std::fs::read_to_string(ch.join("profile.md")).unwrap(),
            "me"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
