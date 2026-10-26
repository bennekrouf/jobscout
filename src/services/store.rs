//! On-disk state: one JSON file per concern under the platform data directory
//! (~/Library/Application Support/JobScout on macOS). Writes are best-effort
//! and atomic; a missing or unreadable file reads as the default.
//!
//! Settings are shared. Everything else belongs to one country and lives in
//! that country's folder (`ch/`, `sg/`): jobs, watches, profile, last search.
//!
//! Two processes can write these files: the window, and `jobscout mcp` when
//! Claude drives it. Each file's modification time is noted whenever this
//! process reads or writes it, so the window can tell a change it did not
//! make (`changed_elsewhere`) and load it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use serde::{de::DeserializeOwned, Serialize};

use crate::model::{Country, Job, SearchParams, Settings, Watch};

pub const JOBS: &str = "jobs.json";
pub const WATCHES: &str = "watches.json";
pub const PROFILE: &str = "profile.md";

/// Files kept per country.
const COUNTRY_FILES: [&str; 4] = [JOBS, WATCHES, PROFILE, "search.json"];

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
    let text = std::fs::read_to_string(path);
    note(path);
    text.ok().and_then(|s| serde_json::from_str(&s).ok())
}

fn save<T: Serialize + ?Sized>(path: &Path, value: &T) {
    let tmp = path.with_extension("tmp");
    if let Ok(json) = serde_json::to_string_pretty(value) {
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }
    note(path);
}

/// Each file's modification time as this process last read or wrote it.
fn seen() -> &'static Mutex<HashMap<PathBuf, Option<SystemTime>>> {
    static SEEN: OnceLock<Mutex<HashMap<PathBuf, Option<SystemTime>>>> = OnceLock::new();
    SEEN.get_or_init(Default::default)
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn note(path: &Path) {
    let mut seen = seen().lock().unwrap_or_else(|e| e.into_inner());
    seen.insert(path.to_path_buf(), mtime(path));
}

/// Whether `path` changed on disk since this process last read or wrote it,
/// that is, whether another process wrote it. Notes the current state, so
/// each change is reported once. A file never read or written here is noted
/// and reported unchanged.
fn changed_since_seen(path: &Path) -> bool {
    let now = mtime(path);
    let mut seen = seen().lock().unwrap_or_else(|e| e.into_inner());
    seen.insert(path.to_path_buf(), now)
        .is_some_and(|before| before != now)
}

/// Whether `file` in `country`'s folder (`JOBS`, `WATCHES`, `PROFILE`) was
/// changed by another process since this one last read or wrote it.
pub fn changed_elsewhere(country: Country, file: &str) -> bool {
    changed_since_seen(&country_dir(country).join(file))
}

/// Whether settings.json was changed by another process since this one last
/// read or wrote it: Claude can set the scoring model.
pub fn settings_changed_elsewhere() -> bool {
    changed_since_seen(&data_dir().join("settings.json"))
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
    load(&country_dir(country).join(JOBS)).unwrap_or_default()
}

pub fn save_jobs(country: Country, jobs: &[Job]) {
    save(&country_dir(country).join(JOBS), jobs)
}

pub fn load_settings() -> Settings {
    let mut settings: Settings = load(&data_dir().join("settings.json")).unwrap_or_default();
    if settings.apply_api0_default() {
        save_settings(&settings);
    }
    settings
}

pub fn save_settings(settings: &Settings) {
    save(&data_dir().join("settings.json"), settings)
}

pub fn load_watches(country: Country) -> Vec<Watch> {
    load(&country_dir(country).join(WATCHES)).unwrap_or_default()
}

pub fn save_watches(country: Country, watches: &[Watch]) {
    save(&country_dir(country).join(WATCHES), watches)
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
    let path = country_dir(country).join(PROFILE);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    note(&path);
    text
}

pub fn save_profile(country: Country, text: &str) {
    let path = country_dir(country).join(PROFILE);
    let _ = std::fs::write(&path, text);
    note(&path);
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
        assert_eq!(p.keywords, "rust");

        // A stray old file never replaces what is already in ch/.
        std::fs::write(root.join("profile.md"), "stale").unwrap();
        migrate_in(&root);
        assert_eq!(
            std::fs::read_to_string(ch.join("profile.md")).unwrap(),
            "me"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_change_is_reported_only_when_another_process_made_it() {
        let dir = std::env::temp_dir().join(format!("jobscout-seen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("jobs.json");

        save(&path, &Vec::<Job>::new());
        assert!(!changed_since_seen(&path), "our own write is not a change");

        // Another process writes it: a later modification time than ours.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&path, "[]").unwrap();
        assert!(changed_since_seen(&path));
        assert!(!changed_since_seen(&path), "reported once");

        let _ = load::<Vec<Job>>(&path);
        assert!(!changed_since_seen(&path));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
