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
//! The licence itself (the number, api0's answer, the daily check, the shared
//! file) is `desktop_kit::licence`, the same in every mayorana app; JobScout
//! names its product id and where it keeps api0's last answer in `kit`. Here:
//! its search allowance.

use std::path::PathBuf;

pub use desktop_kit::licence::{buy_url, current, title_badge, Status, GATEWAY};
use serde::{Deserialize, Serialize};

/// The product id api0 knows JobScout by: a plan lists it to make JobScout Pro.
pub const PRODUCT: &str = "jobscout";

/// api0's OpenAI-compatible endpoint. Its "default" model is the one the
/// licence's plan pays for.
pub const LLM_BASE_URL: &str = "https://gateway.api0.ai/v1";
pub const LLM_MODEL: &str = "default";
/// Read before the saved licence, for a number kept out of JobScout.
pub const LLM_KEY_ENV: &str = "API0_API_KEY";

const SEARCHES_FILE: &str = "searches.json";

/// What JobScout makes of a licence status.
pub trait StatusExt {
    /// How many searches a month this licence gives.
    fn monthly_searches(&self) -> u32;
    /// One line about it, for the MCP server.
    fn describe(&self) -> String;
}

impl StatusExt for Status {
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
        "This month's {} searches are used. They come back on the 1st. {more}: {}",
        status.monthly_searches(),
        buy_url()
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

/// The folder the licence (and the free version's choices) are kept in.
pub fn dir() -> PathBuf {
    crate::services::store::data_dir()
}

/// The licence number saved on this computer, which api0's model also takes
/// as its key: its AI credits pay for the scoring.
pub fn saved_key() -> Option<String> {
    current().licence().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use desktop_kit::licence::{Answer, Plan, Saved};

    fn status_at(saved: Option<&Saved>, now: u64) -> Status {
        desktop_kit::licence::status_at(saved, now, PRODUCT)
    }

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
