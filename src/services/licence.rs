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
//! started offline read the last one: Pro lasts a week
//! (`desktop_kit::licence::GRACE`) after the last check that got through, then
//! waits for the next.
//!
//! Every country is open to every plan. What a plan sets is how many searches
//! JobScout runs a month, counted on this computer across every country:
//! [`FREE_SEARCHES`] without Pro, then by plan (see [`plan_searches`]). A
//! search from the window and a search from Claude each count as one; a
//! watch's scheduled run counts only when it finds new jobs. Scoring and
//! export are never limited.
//!
//! What every app agrees on — the number's format, api0's answer, the grace
//! period, the shared file — is `desktop_kit::licence`. Here: the requests,
//! where JobScout keeps its own answer, and its search allowance.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use desktop_kit::licence::{
    check_due, free_reply, load_shared, normalise, plausible_email, reconcile, Shared,
};
pub use desktop_kit::licence::{Answer, FreeError, Saved, Status, GATEWAY};
use serde::{Deserialize, Serialize};

/// The product id api0 knows JobScout by: a plan lists it to make JobScout Pro.
pub const PRODUCT: &str = "jobscout";

/// Where "Buy JobScout Pro" leads.
pub const BUY_URL: &str = "https://mayorana.ch/en/apps/jobscout";

/// api0's OpenAI-compatible endpoint. Its "default" model is the one the
/// licence's plan pays for.
pub const LLM_BASE_URL: &str = "https://gateway.api0.ai/v1";
pub const LLM_MODEL: &str = "default";
/// Read before the saved licence, for a number kept out of JobScout.
pub const LLM_KEY_ENV: &str = "API0_API_KEY";

const LICENCE_FILE: &str = "licence.json";

const SEARCHES_FILE: &str = "searches.json";

/// What `saved` amounts to for JobScout at `now` (seconds since 1970).
pub fn status_at(saved: Option<&Saved>, now: u64) -> Status {
    desktop_kit::licence::status_at(saved, now, PRODUCT)
}

/// What JobScout makes of a licence status.
pub trait StatusExt {
    /// What the window title adds after "JobScout <version>".
    fn title_badge(&self) -> &'static str;
    /// How many searches a month this licence gives.
    fn monthly_searches(&self) -> u32;
    /// One line about it, for the MCP server.
    fn describe(&self) -> String;
}

impl StatusExt for Status {
    fn title_badge(&self) -> &'static str {
        self.title_badge_for(PRODUCT)
    }

    fn monthly_searches(&self) -> u32 {
        match self {
            Status::Pro(a) => plan_searches(&a.plan.id),
            _ => FREE_SEARCHES,
        }
    }

    fn describe(&self) -> String {
        match self {
            Status::Free => format!("free: {FREE_SEARCHES} searches a month, in any country"),
            Status::Pro(a) => format!(
                "JobScout Pro, {} plan for {}: {} searches a month, in any country ({} AI credits left; {})",
                a.plan.name,
                a.email,
                self.monthly_searches(),
                a.credits.left,
                a.renewal().to_lowercase()
            ),
            Status::NotPro(a) if a.is_free() => format!(
                "free version with free AI credits ({} left this month): {FREE_SEARCHES} searches a month, in any country",
                a.credits.left
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

/// Writes the shared licence file.
fn save_shared(text: &str) {
    desktop_kit::licence::save_shared(text, PRODUCT);
}

/// Searches a month without Pro.
pub const FREE_SEARCHES: u32 = 10;

/// Searches a month on a plan that makes JobScout Pro, by plan id.
pub fn plan_searches(plan_id: &str) -> u32 {
    match plan_id {
        "basic" => 50,
        "max" => 1000,
        // Pro, and any other plan that includes JobScout.
        _ => 500,
    }
}

/// The searches run in one calendar month (`YYYY-MM`, this computer's clock).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub month: String,
    pub used: u32,
}

impl Usage {
    /// The searches run in `month`: none once it is a new month.
    pub fn in_month(self, month: &str) -> Usage {
        if self.month == month {
            self
        } else {
            Usage {
                month: month.into(),
                used: 0,
            }
        }
    }

    /// One more search, if `limit` leaves room for it in `month`.
    pub fn take(self, month: &str, limit: u32) -> Option<Usage> {
        let mut u = self.in_month(month);
        (u.used < limit).then(|| {
            u.used += 1;
            u
        })
    }
}

fn this_month() -> String {
    chrono::Local::now().format("%Y-%m").to_string()
}

fn searches_path() -> PathBuf {
    dir().join(SEARCHES_FILE)
}

fn load_usage() -> Usage {
    std::fs::read_to_string(searches_path())
        .ok()
        .and_then(|t| serde_json::from_str::<Usage>(&t).ok())
        .unwrap_or_default()
        .in_month(&this_month())
}

/// The searches run this month, on this computer.
pub fn searches_used() -> u32 {
    load_usage().used
}

/// Why there is no search left this month, with where to get more.
fn none_left(status: &Status) -> String {
    let more = if status.is_pro() {
        "A bigger plan has more"
    } else {
        "JobScout Pro has more"
    };
    format!(
        "This month's {} searches are used. They come back on the 1st. {more}: {BUY_URL}",
        status.monthly_searches()
    )
}

/// Whether a search is left this month, or why not, without counting one.
pub fn search_left(status: &Status) -> Result<(), String> {
    if searches_used() < status.monthly_searches() {
        Ok(())
    } else {
        Err(none_left(status))
    }
}

/// Counts one search against the plan's month, or says why there is none
/// left. The window and `jobscout mcp` share the count.
pub fn take_search(status: &Status) -> Result<(), String> {
    let Some(usage) = load_usage().take(&this_month(), status.monthly_searches()) else {
        return Err(none_left(status));
    };
    let json = serde_json::to_vec(&usage).map_err(|e| e.to_string())?;
    let path = searches_path();
    let write = || -> std::io::Result<()> {
        std::fs::create_dir_all(dir())?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, &path)
    };
    write().map_err(|e| format!("The search count couldn't be saved: {e}"))
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

fn load_own() -> Option<Saved> {
    let text = std::fs::read_to_string(licence_path()).ok()?;
    serde_json::from_str(&text).ok()
}

fn load() -> Option<Saved> {
    let shared = load_shared();
    if shared == Shared::Removed {
        let _ = std::fs::remove_file(licence_path());
    }
    let (saved, share) = reconcile(shared, load_own());
    if share {
        if let Some(saved) = &saved {
            save_shared(&saved.licence);
        }
    }
    saved
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

/// The licence number saved on this computer, which api0's model also takes
/// as its key: its AI credits pay for the scoring.
pub fn saved_key() -> Option<String> {
    load().map(|saved| saved.licence)
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
    save_shared(&saved.licence);
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

/// Asks api0 to email `email` a licence for the free allowance of AI credits.
/// The licence number itself only goes by email.
pub async fn request_free_credits(email: &str) -> Result<String, FreeError> {
    if !plausible_email(email) {
        return Err(FreeError::Refused(
            "Enter the email the licence should go to.".into(),
        ));
    }
    let resp = client()
        .post(format!("{GATEWAY}/api/plans/free"))
        .header("X-Client-App", PRODUCT)
        .json(&serde_json::json!({ "email": email.trim() }))
        .send()
        .await
        .map_err(|e| {
            FreeError::Refused(format!("The free AI credits couldn't be requested ({e})."))
        })?;
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    free_reply(status, &body)
}

/// Removes the licence from this computer (to move it to another one), for
/// every Mayorana app on it.
pub fn deactivate() {
    let _ = std::fs::remove_file(licence_path());
    save_shared("");
}

#[cfg(test)]
mod tests {
    use super::*;
    use desktop_kit::licence::Plan;

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
    fn each_plan_gives_its_searches_a_month() {
        let now = 1_000 * DAY;
        let on = |plan: &str| {
            let mut a = answer(&["jobscout"], true);
            a.plan.id = plan.into();
            status_at(Some(&saved(a, now)), now).monthly_searches()
        };
        assert_eq!(Status::Free.monthly_searches(), 10);
        assert_eq!(on("basic"), 50);
        assert_eq!(on("pro"), 500);
        assert_eq!(on("max"), 1000);
        // A plan without JobScout, or the free credits, is the free version.
        let gitagent = status_at(Some(&saved(answer(&["gitagent"], true), now)), now);
        assert_eq!(gitagent.monthly_searches(), FREE_SEARCHES);
        let free = status_at(Some(&saved(answer(&[], true), now)), now);
        assert_eq!(free.monthly_searches(), FREE_SEARCHES);
    }

    #[test]
    fn searches_are_counted_by_month() {
        let full = Usage {
            month: "2026-10".into(),
            used: 10,
        };
        assert_eq!(full.clone().take("2026-10", 10), None);
        assert_eq!(full.clone().take("2026-10", 50).map(|u| u.used), Some(11));
        // A new month starts again from nothing.
        assert_eq!(
            full.take("2026-11", 10),
            Some(Usage {
                month: "2026-11".into(),
                used: 1
            })
        );
        assert_eq!(
            Usage::default().take("2026-10", 10).map(|u| u.used),
            Some(1)
        );
    }
}
