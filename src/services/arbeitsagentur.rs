//! The Bundesagentur für Arbeit's Jobbörse (arbeitsagentur.de), Germany's
//! public employment service.
//!
//! Read from the JSON API its own apps use. The key is the public one every
//! client sends (`X-API-Key: jobboerse-jobsuche`); without it the API answers
//! 403.
//!
//!   * search: `GET /pc/v6/jobs?was=…&angebotsart=1&veroeffentlichtseit=N&page=N&size=25`,
//!     all of Germany. `angebotsart=1` keeps jobs, without apprenticeships or
//!     self-employment.
//!   * detail: `GET /pc/v4/jobdetails/{base64 of the reference number}`, the
//!     full posting. Postings first published on a partner board still have
//!     their description here.
//!
//! The reference number (`referenznummer`) is the id, and
//! `/jobsuche/jobdetail/{id}` on the site is the posting's page.

use base64::Engine;
use chrono::{DateTime, Utc};
use serde_json::Value;

use super::text::plain_to_text;
use crate::model::{Job, JobDetail, SearchParams, Source, Status};

const API: &str = "https://rest.arbeitsagentur.de/jobboerse/jobsuche-service";
pub const HEADERS: [(&str, &str); 1] = [("X-API-Key", "jobboerse-jobsuche")];
pub const PAGE_SIZE: u32 = 25;

/// `page` is 1-based, like the API's.
pub fn search_url(p: &SearchParams, page: u32) -> String {
    let mut url = reqwest::Url::parse(&format!("{API}/pc/v6/jobs")).expect("constant URL parses");
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("was", p.keywords.trim());
        q.append_pair("angebotsart", "1");
        if let Some(days) = p.posted.days() {
            q.append_pair("veroeffentlichtseit", &days.to_string());
        }
        q.append_pair("page", &page.max(1).to_string());
        q.append_pair("size", &PAGE_SIZE.to_string());
    }
    url.to_string()
}

pub fn detail_url(id: &str) -> String {
    let key = base64::engine::general_purpose::STANDARD.encode(id);
    format!("{API}/pc/v4/jobdetails/{key}")
}

/// The posting's page on the site, for the browser.
fn page_url(id: &str) -> String {
    format!("https://www.arbeitsagentur.de/jobsuche/jobdetail/{id}")
}

pub fn parse_search(body: &str, query: &str) -> Vec<Job> {
    let now = Utc::now();
    let v: Value = serde_json::from_str(body).unwrap_or_default();
    v["ergebnisliste"]
        .as_array()
        .map(|rs| rs.iter().filter_map(|r| to_job(r, query, now)).collect())
        .unwrap_or_default()
}

/// `None` when the answer has no description: a removed posting.
pub fn parse_detail(body: &str) -> Option<JobDetail> {
    let v: Value = serde_json::from_str(body).ok()?;
    let description = plain_to_text(v["stellenangebotsBeschreibung"].as_str()?);
    if description.is_empty() {
        return None;
    }
    let mut criteria = vec![];
    let mut push = |name: &str, value: Option<String>| {
        if let Some(v) = value.filter(|v| !v.trim().is_empty()) {
            criteria.push((name.to_string(), v));
        }
    };
    push("Work hours", work_hours(&v));
    push("Contract", contract(&v));
    push(
        "Remote",
        (v["homeofficemoeglich"].as_bool() == Some(true)).then(|| "Home office possible".into()),
    );
    push("Start", str_of(&v["eintrittszeitraum"]["von"]));
    push("Occupation", str_of(&v["hauptberuf"]));
    push(
        "Employment",
        (v["istArbeitnehmerUeberlassung"].as_bool() == Some(true))
            .then(|| "Through a staffing agency".into()),
    );
    push("Salary", salary(&v));
    // Partner-board postings are applied for on that board.
    push(
        "Apply at",
        str_of(&v["externeURL"]).and(str_of(&v["allianzpartnerName"])),
    );

    Some(JobDetail {
        description,
        criteria,
        applicants: None,
        posted_ago: None,
        fetched_at: Utc::now(),
    })
}

fn to_job(r: &Value, query: &str, now: DateTime<Utc>) -> Option<Job> {
    let id = str_of(&r["referenznummer"])?;
    Some(Job {
        url: page_url(&id),
        title: str_of(&r["stellenangebotsTitel"])?,
        company: str_of(&r["firma"]).unwrap_or_default(),
        location: places(&r["stellenlokationen"]),
        // The current publication: the date the API's own filter goes by.
        posted: str_of(&r["veroeffentlichungszeitraum"]["von"])
            .or_else(|| str_of(&r["datumErsteVeroeffentlichung"]))
            .map(|d| d.chars().take(10).collect()),
        salary: salary(r),
        id,
        source: Source::Arbeitsagentur,
        found_at: now,
        query: query.to_string(),
        status: Status::New,
        detail: None,
        detail_failed_at: None,
        score: None,
        notified: false,
    })
}

fn str_of(v: &Value) -> Option<String> {
    v.as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Towns, each once; a posting for many sites names the first three.
fn places(v: &Value) -> String {
    let mut towns: Vec<String> = vec![];
    for p in v.as_array().into_iter().flatten() {
        if let Some(t) = str_of(&p["adresse"]["ort"]) {
            if !towns.contains(&t) {
                towns.push(t);
            }
        }
    }
    match towns.len() {
        0 => "Germany".into(),
        1..=3 => towns.join(", "),
        n => format!("{} +{}", towns[..3].join(", "), n - 3),
    }
}

fn work_hours(v: &Value) -> Option<String> {
    let full = v["arbeitszeitVollzeit"].as_bool() == Some(true);
    let part = [
        "arbeitszeitTeilzeitVormittag",
        "arbeitszeitTeilzeitNachmittag",
        "arbeitszeitTeilzeitAbend",
        "arbeitszeitTeilzeitFlexibel",
    ]
    .iter()
    .any(|k| v[*k].as_bool() == Some(true));
    match (full, part) {
        (true, true) => Some("Full-time or part-time".into()),
        (true, false) => Some("Full-time".into()),
        (false, true) => Some("Part-time".into()),
        (false, false) => None,
    }
}

fn contract(v: &Value) -> Option<String> {
    match v["vertragsdauer"].as_str()? {
        "UNBEFRISTET" => Some("Permanent".into()),
        "BEFRISTET" => Some(match v["befristungInMonaten"].as_u64() {
            Some(n) => format!("Fixed-term, {n} months"),
            None => "Fixed-term".into(),
        }),
        _ => None,
    }
}

/// `EUR 68k–75k / year`, `EUR 38–43.71 / hour`.
fn salary(v: &Value) -> Option<String> {
    let amount = |n: f64| {
        if n >= 1000.0 {
            let k = n / 1000.0;
            if (k - k.round()).abs() < 0.05 {
                format!("{}k", k.round())
            } else {
                format!("{k:.1}k")
            }
        } else if n.fract() == 0.0 {
            format!("{n}")
        } else {
            format!("{n:.2}")
        }
    };
    let positive = |key: &str| v[key].as_f64().filter(|n| *n > 0.0);
    let range = match (positive("gehaltsspanneVon"), positive("gehaltsspanneBis")) {
        (Some(lo), Some(hi)) if hi > lo => format!("{}–{}", amount(lo), amount(hi)),
        (Some(n), _) | (_, Some(n)) => amount(n),
        _ => amount(positive("festgehalt")?),
    };
    let unit = match v["verguetungsangabe"].as_str() {
        Some("JAHRESGEHALT") => " / year",
        Some("MONATSGEHALT") => " / month",
        Some("STUNDENLOHN") => " / hour",
        _ => "",
    };
    Some(format!("EUR {range}{unit}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Posted;

    #[test]
    fn urls_cover_germany_and_the_date_window() {
        let mut p = SearchParams {
            keywords: " rust dev ".into(),
            ..SearchParams::default()
        };
        assert_eq!(
            search_url(&p, 1),
            "https://rest.arbeitsagentur.de/jobboerse/jobsuche-service/pc/v6/jobs\
             ?was=rust+dev&angebotsart=1&veroeffentlichtseit=7&page=1&size=25"
        );
        p.posted = Posted::Any;
        assert!(search_url(&p, 3).ends_with("?was=rust+dev&angebotsart=1&page=3&size=25"));
        assert_eq!(
            detail_url("14225-5ccc656c5430b764-S"),
            "https://rest.arbeitsagentur.de/jobboerse/jobsuche-service/pc/v4/jobdetails/\
             MTQyMjUtNWNjYzY1NmM1NDMwYjc2NC1T"
        );
    }

    /// Answers saved from the live API.
    #[test]
    fn parses_saved_answers() {
        let jobs = parse_search(
            include_str!("../../tests/fixtures/arbeitsagentur_search.json"),
            "q",
        );
        assert_eq!(jobs.len(), 25);
        let j = &jobs[0];
        assert_eq!(j.id, "12160-13898397-S");
        assert_eq!(j.title, "Software Engineer (m/w/d)");
        assert_eq!(j.company, "akut... Kompetente Lösungen GmbH");
        assert_eq!(j.location, "Wedel");
        assert_eq!(j.posted.as_deref(), Some("2026-10-16"));
        assert_eq!(j.salary.as_deref(), Some("EUR 38–43.71 / hour"));
        assert_eq!(
            j.url,
            "https://www.arbeitsagentur.de/jobsuche/jobdetail/12160-13898397-S"
        );
        assert_eq!(jobs[1].salary.as_deref(), Some("EUR 68k–75k / year"));
        assert!(jobs.iter().any(|j| j.location.contains(" +")));
        assert!(jobs.iter().all(|j| j.source == Source::Arbeitsagentur));

        let d = parse_detail(include_str!(
            "../../tests/fixtures/arbeitsagentur_detail.json"
        ))
        .unwrap();
        assert!(d.description.starts_with("For our location in Hamburg"));
        assert!(!d.description.contains("\n\n\n"));
        assert!(d
            .criteria
            .contains(&("Salary".into(), "EUR 71.3k / year".into())));
        assert!(d
            .criteria
            .contains(&("Apply at".into(), "jobvector.de".into())));
        assert!(d
            .criteria
            .contains(&("Work hours".into(), "Full-time".into())));
        assert!(parse_detail("{}").is_none());
        assert!(parse_search("not json", "q").is_empty());
    }

    #[test]
    fn list_markers_become_bullets() {
        let v = serde_json::json!({
            "stellenangebotsBeschreibung": "Aufgaben:\n\n\n  - Rust\n  - SQL\n"
        });
        let d = parse_detail(&v.to_string()).unwrap();
        assert_eq!(d.description, "Aufgaben:\n\n• Rust\n• SQL");
    }

    /// Against the live API: `cargo test live_ -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn live_arbeitsagentur_search_and_detail() {
        use crate::services::fetch;
        let settings = crate::model::Settings {
            delay_ms: 1000,
            ..Default::default()
        };
        let p = SearchParams {
            keywords: "software engineer".into(),
            posted: Posted::Month,
            ..SearchParams::default()
        };
        let ttl = std::time::Duration::ZERO;
        let (url1, url2) = (search_url(&p, 1), search_url(&p, 2));
        let page = |url| fetch::get_json_cached(url, &HEADERS, ttl, &settings);
        let page1 = parse_search(&page(&url1).await.unwrap().0, "q");
        let page2 = parse_search(&page(&url2).await.unwrap().0, "q");
        assert_eq!(page1.len(), PAGE_SIZE as usize);
        // Postings published the same day come in no fixed order, so a page
        // can repeat one or two from the page before; the merge drops those.
        let repeated = page2
            .iter()
            .filter(|j| page1.iter().any(|k| k.id == j.id))
            .count();
        assert!(repeated < 5, "page 2 is mostly new: {repeated} repeated");
        let d = parse_detail(
            &fetch::get_json(&detail_url(&page1[0].id), &HEADERS, &settings)
                .await
                .unwrap(),
        );
        assert!(d.is_some_and(|d| !d.description.is_empty()));
    }
}
