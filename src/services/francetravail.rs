//! France Travail (francetravail.fr, formerly Pôle emploi), France's public
//! employment service, through its official "Offres d'emploi v2" API.
//!
//! The site's own search is off limits (its robots.txt disallows every
//! `motsCles=` URL), so this source needs API credentials: an application
//! registered at francetravail.io and subscribed to "Offres d'emploi v2".
//! The client id is in Settings; the secret is read from the environment
//! variable Settings names, like the scoring model's key, and never saved.
//!
//!   * token: `POST entreprise.francetravail.fr/connexion/oauth2/access_token?realm=/partenaire`,
//!     client credentials, kept until shortly before it expires.
//!   * search: `GET /v2/offres/search?motsCles=…&minCreationDate=…&maxCreationDate=…&range=a-b`,
//!     all of France. Answers 206 for a partial range and 204 past the end.
//!   * detail: `GET /v2/offres/{id}`. A search result already holds the full
//!     posting, so this only refreshes one.
//!
//! The posting's page on the site, `/offres/recherche/detail/{id}`, is the
//! link opened in the browser.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::fetch::{self, FetchError};
use super::llm;
use super::text::plain_to_text;
use crate::model::{Contract, Job, JobDetail, SearchParams, Settings, Source, Status};

const TOKEN_URL: &str =
    "https://entreprise.francetravail.fr/connexion/oauth2/access_token?realm=%2Fpartenaire";
const SCOPE: &str = "api_offresdemploiv2 o2dsoffre";
const API: &str = "https://api.francetravail.io/partenaire/offresdemploi/v2";
pub const PAGE_SIZE: u32 = 50;
/// The API refuses a range starting past this index, however many results
/// there are.
const MAX_START: u32 = 3000;

/// `page` is 1-based; each page is one `range` of `PAGE_SIZE` postings.
pub fn search_url(p: &SearchParams, page: u32, now: DateTime<Utc>) -> Option<String> {
    let first = (page.max(1) - 1) * PAGE_SIZE;
    if first > MAX_START {
        return None;
    }
    let last = first + PAGE_SIZE - 1;
    let mut url =
        reqwest::Url::parse(&format!("{API}/offres/search")).expect("constant URL parses");
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("motsCles", p.keywords.trim());
        // Both bounds or neither: the API refuses one without the other.
        if let Some(days) = p.posted.days() {
            let iso = |t: DateTime<Utc>| t.format("%Y-%m-%dT%H:%M:%SZ").to_string();
            q.append_pair("minCreationDate", &iso(now - chrono::Duration::days(days)));
            q.append_pair("maxCreationDate", &iso(now));
        }
        // LIB is "profession libérale", how freelance missions are listed.
        if !p.contract.is_empty() {
            let types: Vec<&str> = p
                .contract
                .iter()
                .map(|c| match c {
                    Contract::Permanent => "CDI",
                    Contract::Freelance => "LIB",
                })
                .collect();
            q.append_pair("typeContrat", &types.join(","));
        }
        q.append_pair("range", &format!("{first}-{last}"));
    }
    Some(url.to_string())
}

pub fn detail_url(id: &str) -> String {
    format!("{API}/offres/{id}")
}

fn page_url(id: &str) -> String {
    format!("https://candidat.francetravail.fr/offres/recherche/detail/{id}")
}

/// One search page. Past the last result there is nothing to ask for, which
/// reads as an empty page.
pub async fn fetch_search(
    p: &SearchParams,
    page: u32,
    ttl: Duration,
    settings: &Settings,
) -> Result<(String, bool), FetchError> {
    let Some(url) = search_url(p, page, Utc::now()) else {
        return Ok((String::new(), false));
    };
    with_token(settings, |auth| {
        let url = url.clone();
        async move {
            fetch::get_json_cached(&url, &[("Authorization", auth.as_str())], ttl, settings).await
        }
    })
    .await
}

pub async fn fetch_detail(id: &str, settings: &Settings) -> Result<String, FetchError> {
    let url = detail_url(id);
    with_token(settings, |auth| {
        let url = url.clone();
        async move { fetch::get_json(&url, &[("Authorization", auth.as_str())], settings).await }
    })
    .await
}

/// Runs `call` with a bearer token. A 401 means the token was revoked or
/// expired early: it is dropped and the call made once more with a new one.
async fn with_token<T, F, Fut>(settings: &Settings, call: F) -> Result<T, FetchError>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = Result<T, FetchError>>,
{
    let first = token(settings).await?;
    match call(format!("Bearer {first}")).await {
        Err(FetchError::Status(401)) => {
            forget_token();
            let fresh = token(settings).await?;
            call(format!("Bearer {fresh}")).await
        }
        other => other,
    }
}

struct Token {
    client_id: String,
    value: String,
    until: Instant,
}

static TOKEN: Mutex<Option<Token>> = Mutex::new(None);

fn forget_token() {
    *TOKEN.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

async fn token(settings: &Settings) -> Result<String, FetchError> {
    let client_id = settings.france_travail_client_id.trim();
    if client_id.is_empty() {
        return Err(FetchError::Setup(
            "France Travail needs API credentials: register an application at \
             francetravail.io, then enter its client id in Settings"
                .into(),
        ));
    }
    {
        let held = TOKEN.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = held.as_ref() {
            if t.client_id == client_id && Instant::now() < t.until {
                return Ok(t.value.clone());
            }
        }
    }
    let secret = llm::api_key(&settings.france_travail_secret_env)
        .map_err(FetchError::Setup)?
        .ok_or_else(|| {
            FetchError::Setup(
                "France Travail needs the client secret: name its environment variable in Settings"
                    .into(),
            )
        })?;
    let form = [
        ("grant_type", "client_credentials"),
        ("client_id", client_id),
        ("client_secret", secret.as_str()),
        ("scope", SCOPE),
    ];
    let body = match fetch::post_form(TOKEN_URL, &form, settings).await {
        // A wrong id or secret.
        Err(FetchError::Status(400 | 401)) => {
            return Err(FetchError::Setup(
                "France Travail refused the client id or secret in Settings".into(),
            ))
        }
        other => other?,
    };
    let (value, lifetime) = parse_token(&body)
        .ok_or_else(|| FetchError::Network("France Travail sent no access token".into()))?;
    *TOKEN.lock().unwrap_or_else(|e| e.into_inner()) = Some(Token {
        client_id: client_id.to_string(),
        value: value.clone(),
        // A minute early, so a token never runs out mid-search.
        until: Instant::now() + lifetime.saturating_sub(Duration::from_secs(60)),
    });
    Ok(value)
}

fn parse_token(body: &str) -> Option<(String, Duration)> {
    let v: Value = serde_json::from_str(body).ok()?;
    let token = str_of(&v["access_token"])?;
    let secs = v["expires_in"].as_u64().unwrap_or(1499);
    Some((token, Duration::from_secs(secs)))
}

/// A search result is the whole posting, so its description comes with it.
pub fn parse_search(body: &str, query: &str) -> Vec<Job> {
    let now = Utc::now();
    let v: Value = serde_json::from_str(body).unwrap_or_default();
    v["resultats"]
        .as_array()
        .map(|rs| rs.iter().filter_map(|r| to_job(r, query, now)).collect())
        .unwrap_or_default()
}

/// `None` when the answer has no description: a removed posting.
pub fn parse_detail(body: &str) -> Option<JobDetail> {
    let v: Value = serde_json::from_str(body).ok()?;
    detail(&v, Utc::now())
}

fn detail(v: &Value, now: DateTime<Utc>) -> Option<JobDetail> {
    let description = plain_to_text(v["description"].as_str()?);
    if description.is_empty() {
        return None;
    }
    let mut criteria = vec![];
    let mut push = |name: &str, value: Option<String>| {
        if let Some(v) = value.filter(|v| !v.trim().is_empty()) {
            criteria.push((name.to_string(), v));
        }
    };
    push("Contract", str_of(&v["typeContratLibelle"]));
    push("Work hours", str_of(&v["dureeTravailLibelleConverti"]));
    push("Experience", str_of(&v["experienceLibelle"]));
    push("Qualification", str_of(&v["qualificationLibelle"]));
    push("Occupation", str_of(&v["appellationlibelle"]));
    push("Industry", str_of(&v["secteurActiviteLibelle"]));
    push("Salary", salary(&v["salaire"]));
    push("Skills", names(&v["competences"]));
    // Partner-board postings are applied for on that board.
    push(
        "Apply at",
        v["origineOffre"]["partenaires"]
            .as_array()
            .and_then(|ps| ps.iter().find_map(|p| str_of(&p["nom"]))),
    );

    Some(JobDetail {
        description,
        criteria,
        applicants: None,
        posted_ago: None,
        fetched_at: now,
    })
}

fn to_job(r: &Value, query: &str, now: DateTime<Utc>) -> Option<Job> {
    let id = str_of(&r["id"])?;
    Some(Job {
        url: page_url(&id),
        title: str_of(&r["intitule"])?,
        company: str_of(&r["entreprise"]["nom"]).unwrap_or_default(),
        location: place(&r["lieuTravail"]),
        posted: str_of(&r["dateCreation"]).map(|d| d.chars().take(10).collect()),
        salary: salary(&r["salaire"]),
        detail: detail(r, now),
        id,
        source: Source::FranceTravail,
        found_at: now,
        query: query.to_string(),
        status: Status::New,
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

/// `75 - Paris 15e Arrondissement` → `Paris 15e Arrondissement`: the
/// department number in front is dropped.
fn place(v: &Value) -> String {
    let Some(label) = str_of(&v["libelle"]) else {
        return "France".into();
    };
    match label.split_once(" - ") {
        Some((dept, town)) if dept.chars().all(|c| c.is_ascii_alphanumeric()) => {
            town.trim().to_string()
        }
        _ => label,
    }
}

/// `[{"libelle": "Rust"}, {"libelle": "SQL"}]` → `Rust, SQL`.
fn names(list: &Value) -> Option<String> {
    let names: Vec<String> = list
        .as_array()?
        .iter()
        .filter_map(|x| str_of(&x["libelle"]))
        .collect();
    (!names.is_empty()).then(|| names.join(", "))
}

/// `Annuel de 45000.00 Euros à 55000.00 Euros sur 12 mois` → `EUR 45k–55k /
/// year`. Anything not in that shape is shown as the employer wrote it.
fn salary(v: &Value) -> Option<String> {
    let label = str_of(&v["libelle"]).or_else(|| str_of(&v["commentaire"]))?;
    let unit = match label.split_whitespace().next()? {
        "Annuel" => " / year",
        "Mensuel" => " / month",
        "Horaire" => " / hour",
        _ => return Some(label),
    };
    let amounts: Vec<f64> = label
        .split_whitespace()
        .filter_map(|w| w.replace(',', ".").parse::<f64>().ok())
        .filter(|n| *n > 0.0)
        .collect();
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
    // "sur 12 mois" is a number too; only the first one or two are amounts.
    let range = match amounts.as_slice() {
        [lo, hi, ..] if label.contains(" à ") && hi > lo => {
            format!("{}–{}", amount(*lo), amount(*hi))
        }
        [n, ..] => amount(*n),
        [] => return Some(label),
    };
    Some(format!("EUR {range}{unit}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Posted;

    #[test]
    fn urls_page_by_range_and_bound_the_dates() {
        let now = DateTime::parse_from_rfc3339("2026-10-18T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut p = SearchParams {
            keywords: " développeur rust ".into(),
            ..SearchParams::default()
        };
        assert_eq!(
            search_url(&p, 1, now).unwrap(),
            "https://api.francetravail.io/partenaire/offresdemploi/v2/offres/search\
             ?motsCles=d%C3%A9veloppeur+rust\
             &minCreationDate=2026-10-11T12%3A00%3A00Z&maxCreationDate=2026-10-18T12%3A00%3A00Z\
             &range=0-49"
        );
        p.posted = Posted::Any;
        assert!(search_url(&p, 3, now)
            .unwrap()
            .ends_with("?motsCles=d%C3%A9veloppeur+rust&range=100-149"));
        assert!(search_url(&p, 61, now)
            .unwrap()
            .ends_with("range=3000-3049"));
        assert_eq!(search_url(&p, 62, now), None);
        p.contract = vec![Contract::Permanent, Contract::Freelance];
        assert!(search_url(&p, 1, now)
            .unwrap()
            .ends_with("&typeContrat=CDI%2CLIB&range=0-49"));
        assert_eq!(
            detail_url("201ABCD"),
            "https://api.francetravail.io/partenaire/offresdemploi/v2/offres/201ABCD"
        );
    }

    #[test]
    fn reads_a_token() {
        let (t, life) =
            parse_token(r#"{"access_token":"abc","expires_in":1499,"token_type":"Bearer"}"#)
                .unwrap();
        assert_eq!((t.as_str(), life.as_secs()), ("abc", 1499));
        assert!(parse_token(r#"{"error":"invalid_client"}"#).is_none());
    }

    /// A search answer in the API's documented shape.
    #[test]
    fn parses_a_search_answer() {
        let jobs = parse_search(
            include_str!("../../tests/fixtures/francetravail_search.json"),
            "q",
        );
        assert_eq!(jobs.len(), 2);
        let j = &jobs[0];
        assert_eq!(j.id, "201QXKL");
        assert_eq!(j.title, "Développeur Rust H/F");
        assert_eq!(j.company, "ACME SYSTEMES");
        assert_eq!(j.location, "Lyon 3e Arrondissement");
        assert_eq!(j.posted.as_deref(), Some("2026-10-15"));
        assert_eq!(j.salary.as_deref(), Some("EUR 45k–55k / year"));
        assert_eq!(
            j.url,
            "https://candidat.francetravail.fr/offres/recherche/detail/201QXKL"
        );
        let d = j.detail.as_ref().expect("the search holds the description");
        assert!(d.description.contains("• Concevoir"));
        assert!(d
            .criteria
            .contains(&("Contract".into(), "Contrat à durée indéterminée".into())));
        assert!(d
            .criteria
            .contains(&("Skills".into(), "Rust, Linux".into())));

        let other = &jobs[1];
        assert_eq!(other.company, "");
        assert_eq!(other.location, "France");
        assert_eq!(other.salary.as_deref(), Some("Selon profil"));
        assert!(other
            .detail
            .as_ref()
            .unwrap()
            .criteria
            .contains(&("Apply at".into(), "HELLOWORK".into())));

        assert!(parse_search("", "q").is_empty(), "204 past the end");
        assert!(parse_detail("{}").is_none());
    }

    #[test]
    fn salary_shapes() {
        let s = |l: &str| salary(&serde_json::json!({ "libelle": l }));
        assert_eq!(
            s("Mensuel de 2500.00 Euros sur 12 mois").as_deref(),
            Some("EUR 2.5k / month")
        );
        assert_eq!(
            s("Horaire de 12.50 Euros à 14.00 Euros").as_deref(),
            Some("EUR 12.50–14 / hour")
        );
        assert_eq!(s("Selon expérience").as_deref(), Some("Selon expérience"));
    }

    /// Against the live API, with credentials:
    /// `FRANCE_TRAVAIL_CLIENT_ID=… FRANCE_TRAVAIL_CLIENT_SECRET=… cargo test live_francetravail -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn live_francetravail_search_and_detail() {
        let settings = Settings {
            delay_ms: 1000,
            france_travail_client_id: std::env::var("FRANCE_TRAVAIL_CLIENT_ID").unwrap(),
            ..Default::default()
        };
        let p = SearchParams {
            keywords: "développeur".into(),
            ..SearchParams::default()
        };
        let ttl = Duration::ZERO;
        let page1 = parse_search(&fetch_search(&p, 1, ttl, &settings).await.unwrap().0, "q");
        let page2 = parse_search(&fetch_search(&p, 2, ttl, &settings).await.unwrap().0, "q");
        assert_eq!(page1.len(), PAGE_SIZE as usize);
        assert!(
            page2.iter().any(|j| !page1.iter().any(|k| k.id == j.id)),
            "page 2 is new"
        );
        assert!(page1.iter().all(|j| j.detail.is_some()));
        let d = parse_detail(&fetch_detail(&page1[0].id, &settings).await.unwrap());
        assert!(d.is_some_and(|d| !d.description.is_empty()));
    }
}
