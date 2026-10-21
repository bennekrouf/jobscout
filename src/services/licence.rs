//! JobScout Pro: the licence on this computer.
//!
//! Pro comes with a monthly plan bought on mayorana.ch. The plan's licence
//! number (`ABCDE-FGHJK-LMNPQ-RSTUV`, emailed when it is bought) is saved here
//! and checked with api0 when JobScout starts and then once a day:
//!
//! ```text
//! GET https://gateway.api0.ai/v1/licence
//! Authorization: Bearer <licence>
//! X-Client-App: jobscout
//! ```
//!
//! The answer names the plan, the apps it makes Pro, whether it is paid up and
//! the AI credits left. It is kept on disk, so the MCP server and a JobScout
//! started offline read the last one: Pro lasts [`GRACE`] after the last check
//! that got through, then waits for the next.
//!
//! Without Pro, JobScout keeps [`FREE_WATCHES`] watch, counted across every
//! country. Searching, scoring and export are never limited, and watches
//! already saved keep running.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// The product id api0 knows JobScout by: a plan lists it to make JobScout Pro.
pub const PRODUCT: &str = "jobscout";

/// Where "Buy JobScout Pro" leads.
pub const BUY_URL: &str = "https://mayorana.ch/en/apps/jobscout";

const GATEWAY: &str = "https://gateway.api0.ai";

/// How often the licence is checked while JobScout is open.
pub const CHECK_EVERY: Duration = Duration::from_secs(24 * 3600);

/// How long Pro lasts without a check getting through (offline, api0 down).
pub const GRACE: Duration = Duration::from_secs(7 * 24 * 3600);

const LICENCE_FILE: &str = "licence.json";

/// How many watches the free version keeps, across every country.
pub const FREE_WATCHES: usize = 1;

const ALPHABET: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// A licence number as typed — any case, spaces or dashes — in the form api0
/// stores it, or `None` if it can't be one.
pub fn normalise(typed: &str) -> Option<String> {
    let chars: String = typed
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if chars.len() != 20 || !chars.chars().all(|c| ALPHABET.contains(c)) {
        return None;
    }
    Some(
        chars
            .as_bytes()
            .chunks(5)
            .map(|c| std::str::from_utf8(c).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("-"),
    )
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Credits {
    #[serde(default)]
    pub granted: i64,
    #[serde(default)]
    pub used: i64,
    #[serde(default)]
    pub left: i64,
}

/// api0's answer about a licence (`GET /v1/licence`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Answer {
    #[serde(default)]
    pub licence: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub plan: Plan,
    /// The apps the plan makes Pro, by product id; "*" is every app.
    #[serde(default)]
    pub products: Vec<String>,
    /// Paid up. False once the subscription lapses or is cancelled.
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub cancel_at_period_end: bool,
    /// When the current month ends (RFC 3339).
    #[serde(default)]
    pub period_end: String,
    #[serde(default)]
    pub credits: Credits,
    #[serde(default)]
    pub checked_at: String,
}

impl Answer {
    /// The plan is paid up and includes JobScout.
    pub fn unlocks(&self) -> bool {
        self.active && self.products.iter().any(|p| p == PRODUCT || p == "*")
    }

    /// The day the current month ends, `YYYY-MM-DD`.
    pub fn period_end_day(&self) -> &str {
        self.period_end.get(..10).unwrap_or(&self.period_end)
    }

    /// "Renews on …", or "Ends on …" once cancelled.
    pub fn renewal(&self) -> String {
        let verb = if self.cancel_at_period_end {
            "Ends"
        } else {
            "Renews"
        };
        format!("{verb} on {}", self.period_end_day())
    }
}

/// What is kept on disk: the licence number, and api0's last answer with when
/// it came (seconds since 1970, this computer's clock).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Saved {
    pub licence: String,
    #[serde(default)]
    pub answer: Option<Answer>,
    #[serde(default)]
    pub checked: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// No licence on this computer.
    Free,
    /// The plan includes JobScout and is paid up, as of a check within [`GRACE`].
    Pro(Answer),
    /// Checked: the plan has lapsed, or doesn't include JobScout.
    NotPro(Answer),
    /// Pro at the last check, but no check has got through for [`GRACE`].
    Offline(Answer),
    /// A licence number api0 doesn't know, or one not checked yet.
    Unknown(String),
}

impl Status {
    /// Whether the free version's limit is lifted.
    pub fn unlimited(&self) -> bool {
        matches!(self, Status::Pro(_))
    }

    /// The saved licence number, if any.
    pub fn licence(&self) -> Option<&str> {
        match self {
            Status::Free => None,
            Status::Pro(a) | Status::NotPro(a) | Status::Offline(a) => Some(&a.licence),
            Status::Unknown(l) => Some(l),
        }
    }

    /// One line about it, for the MCP server.
    pub fn describe(&self) -> String {
        match self {
            Status::Free => format!("free: {FREE_WATCHES} watch across all countries"),
            Status::Pro(a) => format!(
                "JobScout Pro, {} plan for {} ({} AI credits left; {})",
                a.plan.name,
                a.email,
                a.credits.left,
                a.renewal().to_lowercase()
            ),
            Status::NotPro(a) if !a.active => {
                "subscription not active: free limits apply until it is renewed".into()
            }
            Status::NotPro(a) => format!(
                "the {} plan doesn't include JobScout: free limits apply",
                a.plan.name
            ),
            Status::Offline(_) => {
                "Pro licence not checked for a week (offline?): free limits apply until it is"
                    .into()
            }
            Status::Unknown(_) => "licence number not recognised: free limits apply".into(),
        }
    }
}

/// What `saved` amounts to at `now` (seconds since 1970).
pub fn status_at(saved: Option<&Saved>, now: u64) -> Status {
    let Some(saved) = saved else {
        return Status::Free;
    };
    let Some(answer) = saved.answer.clone() else {
        return Status::Unknown(saved.licence.clone());
    };
    if !answer.unlocks() {
        Status::NotPro(answer)
    } else if now.saturating_sub(saved.checked) > GRACE.as_secs() {
        Status::Offline(answer)
    } else {
        Status::Pro(answer)
    }
}

/// Whether the last check is old enough for another, at `now`.
pub fn check_due(saved: &Saved, now: u64) -> bool {
    saved.answer.is_none() || now.saturating_sub(saved.checked) >= CHECK_EVERY.as_secs()
}

/// Whether another watch may be saved when `existing` are already saved.
pub fn may_add_watch(status: &Status, existing: usize) -> bool {
    status.unlimited() || existing < FREE_WATCHES
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The folder the licence (and the free version's choices) are kept in.
pub fn dir() -> PathBuf {
    crate::services::store::data_dir()
}

fn licence_path() -> PathBuf {
    dir().join(LICENCE_FILE)
}

fn load() -> Option<Saved> {
    let text = std::fs::read_to_string(licence_path()).ok()?;
    serde_json::from_str(&text).ok()
}

fn save(saved: &Saved) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(saved).map_err(|e| e.to_string())?;
    let path = licence_path();
    let write = || -> std::io::Result<()> {
        std::fs::create_dir_all(dir())?;
        // Written beside the target and renamed over it, so a crash can't leave
        // half a file behind.
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, &path)
    };
    write().map_err(|e| format!("The licence couldn't be saved: {e}"))
}

/// The licence as of the last check, without asking api0.
pub fn current() -> Status {
    status_at(load().as_ref(), now())
}

/// Why a check didn't give an answer.
#[derive(Debug)]
enum CheckError {
    /// api0 doesn't know this licence number.
    Unknown,
    /// Offline, api0 down, or an answer that made no sense.
    Unreachable(String),
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap_or_default()
}

async fn ask(licence: &str) -> Result<Answer, CheckError> {
    let resp = client()
        .get(format!("{GATEWAY}/v1/licence"))
        .bearer_auth(licence)
        .header("X-Client-App", PRODUCT)
        .send()
        .await
        .map_err(|e| CheckError::Unreachable(e.to_string()))?;
    match resp.status().as_u16() {
        200 => resp
            .json::<Answer>()
            .await
            .map_err(|e| CheckError::Unreachable(e.to_string())),
        401 => Err(CheckError::Unknown),
        code => Err(CheckError::Unreachable(format!("api0 answered {code}"))),
    }
}

/// Checks `typed` with api0 and, if api0 knows it, saves it.
pub async fn activate(typed: &str) -> Result<Status, String> {
    let licence = normalise(typed).ok_or(
        "This isn't a licence number. It has 20 letters and digits, like ABCDE-FGHJK-LMNPQ-RSTUV.",
    )?;
    let answer = match ask(&licence).await {
        Ok(a) => a,
        Err(CheckError::Unknown) => {
            return Err("This licence number isn't known. Check it against the email.".into())
        }
        Err(CheckError::Unreachable(e)) => {
            return Err(format!(
                "The licence couldn't be checked ({e}). Check the connection and try again."
            ))
        }
    };
    let saved = Saved {
        licence,
        answer: Some(answer),
        checked: now(),
    };
    save(&saved)?;
    Ok(status_at(Some(&saved), now()))
}

/// Checks the saved licence with api0 again, when `force`d or a day after the
/// last check, and keeps the answer. Offline, the last answer stands.
pub async fn refresh(force: bool) -> Status {
    let Some(mut saved) = load() else {
        return Status::Free;
    };
    if !force && !check_due(&saved, now()) {
        return status_at(Some(&saved), now());
    }
    match ask(&saved.licence).await {
        Ok(answer) => {
            saved.answer = Some(answer);
            saved.checked = now();
            let _ = save(&saved);
        }
        Err(CheckError::Unknown) => {
            saved.answer = None;
            let _ = save(&saved);
        }
        Err(CheckError::Unreachable(_)) => {}
    }
    status_at(Some(&saved), now())
}

/// The address of the page where the subscription is changed or cancelled.
pub async fn portal_url(licence: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Portal {
        url: String,
    }
    let resp = client()
        .post(format!("{GATEWAY}/v1/licence/portal"))
        .bearer_auth(licence)
        .header("X-Client-App", PRODUCT)
        .send()
        .await
        .map_err(|e| format!("The subscription page couldn't be opened ({e})."))?;
    if !resp.status().is_success() {
        return Err(format!(
            "The subscription page couldn't be opened (api0 answered {}).",
            resp.status().as_u16()
        ));
    }
    let portal: Portal = resp
        .json()
        .await
        .map_err(|e| format!("The subscription page couldn't be opened ({e})."))?;
    Ok(portal.url)
}

/// Removes the licence from this computer (to move it to another one).
pub fn deactivate() {
    let _ = std::fs::remove_file(licence_path());
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: u64 = 24 * 3600;

    fn answer(products: &[&str], active: bool) -> Answer {
        Answer {
            licence: "ABCDE-FGHJK-LMNPQ-RSTUV".into(),
            email: "dev@example.ch".into(),
            plan: Plan {
                id: "pro".into(),
                name: "Pro".into(),
            },
            products: products.iter().map(|p| p.to_string()).collect(),
            active,
            status: "active".into(),
            period_end: "2026-11-21T10:00:00Z".into(),
            ..Answer::default()
        }
    }

    fn saved(answer: Answer, checked: u64) -> Saved {
        Saved {
            licence: answer.licence.clone(),
            answer: Some(answer),
            checked,
        }
    }

    #[test]
    fn licence_numbers_are_read_in_any_case_with_spaces_or_dashes() {
        let want = Some("ABCDE-FGHJK-LMNPQ-RSTUV".to_string());
        assert_eq!(normalise("ABCDE-FGHJK-LMNPQ-RSTUV"), want);
        assert_eq!(normalise(" abcde fghjk\nlmnpq-rstuv "), want);
        assert_eq!(normalise("ABCDEFGHJKLMNPQRSTUV"), want);
        // Too short, and letters that look like digits aren't in the alphabet.
        assert_eq!(normalise("ABCDE-FGHJK-LMNPQ"), None);
        assert_eq!(normalise("ABCDE-FGHJK-LMNPQ-RSTU0"), None);
        assert_eq!(normalise("ABCDE-FGHJK-LMNPQ-RSTUI"), None);
    }

    #[test]
    fn pro_needs_an_active_plan_that_lists_jobscout_or_every_app() {
        assert!(answer(&["jobscout"], true).unlocks());
        assert!(answer(&["gitagent", "*"], true).unlocks());
        assert!(!answer(&["gitagent", "spreadwatch"], true).unlocks());
        assert!(!answer(&["jobscout"], false).unlocks());
        assert!(!answer(&[], true).unlocks());

        let now = 1_000 * DAY;
        assert!(status_at(Some(&saved(answer(&["*"], true), now)), now).unlimited());
        assert!(matches!(
            status_at(Some(&saved(answer(&["jobscout"], false), now)), now),
            Status::NotPro(_)
        ));
        assert!(matches!(
            status_at(Some(&saved(answer(&["gitagent"], true), now)), now),
            Status::NotPro(_)
        ));
        assert_eq!(status_at(None, now), Status::Free);
        let unchecked = Saved {
            licence: "ABCDE-FGHJK-LMNPQ-RSTUV".into(),
            ..Saved::default()
        };
        assert!(!status_at(Some(&unchecked), now).unlimited());
    }

    #[test]
    fn pro_lasts_seven_days_after_the_last_check_that_got_through() {
        let checked = 1_000 * DAY;
        let s = saved(answer(&["jobscout"], true), checked);
        assert!(status_at(Some(&s), checked + 6 * DAY).unlimited());
        assert!(status_at(Some(&s), checked + 7 * DAY).unlimited());
        let late = status_at(Some(&s), checked + 7 * DAY + 1);
        assert!(matches!(late, Status::Offline(_)));
        assert!(!late.unlimited());
        // A clock set back doesn't take Pro away.
        assert!(status_at(Some(&s), checked - DAY).unlimited());
    }

    #[test]
    fn the_licence_is_checked_again_after_a_day() {
        let checked = 1_000 * DAY;
        let s = saved(answer(&["jobscout"], true), checked);
        assert!(!check_due(&s, checked + DAY - 1));
        assert!(check_due(&s, checked + DAY));
        assert!(check_due(
            &Saved {
                licence: s.licence.clone(),
                ..Saved::default()
            },
            checked
        ));
    }

    #[test]
    fn api0s_answer_is_read() {
        let json = r#"{"object":"licence","licence":"ABCDE-FGHJK-LMNPQ-RSTUV","email":"dev@example.ch",
            "plan":{"id":"studio","name":"Studio"},"products":["jobscout","splitter"],"active":true,
            "status":"active","cancel_at_period_end":true,"period_end":"2026-11-21T10:00:00Z",
            "credits":{"granted":500,"used":120,"left":380},"checked_at":"2026-10-21T10:00:00Z"}"#;
        let a: Answer = serde_json::from_str(json).unwrap();
        assert!(a.unlocks());
        assert_eq!(a.plan.name, "Studio");
        assert_eq!(a.credits.left, 380);
        assert_eq!(a.renewal(), "Ends on 2026-11-21");
    }

    #[test]
    fn the_free_version_keeps_one_watch() {
        assert!(may_add_watch(&Status::Free, 0));
        assert!(!may_add_watch(&Status::Free, 1));
        let now = 1_000 * DAY;
        let pro = status_at(Some(&saved(answer(&["jobscout"], true), now)), now);
        assert!(may_add_watch(&pro, 4));
    }
}
