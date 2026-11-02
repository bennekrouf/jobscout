//! Reading a tool call's arguments, and the settings and sources it runs with.

use super::*;

pub(super) fn text(args: &Value, key: &str) -> Result<String, String> {
    args[key]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .ok_or(format!("{key} is required"))
}

pub(super) fn country(args: &Value) -> Result<Country, String> {
    let Some(v) = args["country"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Ok(store::load_settings().country);
    };
    Country::ALL
        .iter()
        .copied()
        .find(|c| {
            c.code().eq_ignore_ascii_case(v)
                || c.label().eq_ignore_ascii_case(v)
                || c.short_label().eq_ignore_ascii_case(v)
        })
        .ok_or_else(|| {
            let known: Vec<String> = Country::ALL
                .iter()
                .map(|c| format!("{} ({})", c.code(), c.label()))
                .collect();
            format!("no country {v}; JobScout covers {}", known.join(", "))
        })
}

/// Letters and digits only, lowercased: "jobs.ch", "JobsCh" and "jobsch" are
/// the same board.
pub(super) fn squash(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

pub(super) fn source(v: &str, country: Country) -> Result<Source, String> {
    country
        .sources()
        .iter()
        .copied()
        // "jobup" names jobup.ch as well as the full name does.
        .find(|s| {
            let label = s.label();
            let short = label.split('.').next().unwrap_or(label);
            [squash(label), squash(short)].contains(&squash(v))
        })
        .ok_or_else(|| {
            let names: Vec<&str> = country.sources().iter().map(|s| s.label()).collect();
            format!(
                "{v} is not a board in {}; it has {}",
                country.label(),
                names.join(", ")
            )
        })
}

pub(super) fn status(v: &str) -> Result<Status, String> {
    Status::ALL
        .iter()
        .copied()
        .find(|s| s.label().eq_ignore_ascii_case(v.trim()))
        .ok_or_else(|| format!("no status {v}; use New, Saved, Applied or Hidden"))
}

pub(super) fn posted(v: &str) -> Result<Posted, String> {
    match v.trim().to_lowercase().as_str() {
        "any" | "any time" => Ok(Posted::Any),
        "day" | "24h" | "past 24 hours" => Ok(Posted::Day),
        "week" | "past week" => Ok(Posted::Week),
        "month" | "past month" => Ok(Posted::Month),
        _ => Err(format!("posted is any, day, week or month, not {v}")),
    }
}

/// A search from the arguments, starting from what a first search in the
/// window has: every board of the country, the past week, 3 pages.
pub(super) fn search_params(args: &Value, country: Country) -> Result<SearchParams, String> {
    let mut p = SearchParams::for_country(country);
    p.keywords = text(args, "keywords")?;
    if let Some(v) = args["posted"].as_str() {
        p.posted = posted(v)?;
    }
    if let Some(list) = args["sources"].as_array() {
        let mut chosen = vec![];
        for v in list {
            let s = source(v.as_str().unwrap_or(""), country)?;
            if !chosen.contains(&s) {
                chosen.push(s);
            }
        }
        if chosen.is_empty() {
            return Err("sources is empty; leave it out to search every board".into());
        }
        p.sources = chosen;
    }
    if let Some(n) = args["pages"].as_u64() {
        p.pages = n.clamp(1, 20) as u32;
    }
    p.onsite = args["onsite"].as_bool().unwrap_or(false);
    p.remote = args["remote"].as_bool().unwrap_or(false);
    p.hybrid = args["hybrid"].as_bool().unwrap_or(false);
    if let Some(list) = args["experience"].as_array() {
        p.experience = list
            .iter()
            .filter_map(Value::as_u64)
            .filter(|n| EXPERIENCE.iter().any(|(code, _)| u64::from(*code) == *n))
            .map(|n| n as u8)
            .collect();
    }
    if let Some(list) = args["contract"].as_array() {
        for v in list {
            let c = match v.as_str().unwrap_or("") {
                "permanent" => Contract::Permanent,
                "freelance" => Contract::Freelance,
                other => {
                    return Err(format!(
                        "unknown contract {other:?}: permanent or freelance"
                    ))
                }
            };
            if !p.contract.contains(&c) {
                p.contract.push(c);
            }
        }
    }
    Ok(p)
}

/// The window's settings, minus what only works with a window: the built-in
/// WebKit browser runs on the window's main thread, so here LinkedIn is read
/// over plain HTTP instead.
pub(super) fn settings() -> Settings {
    let mut s = store::load_settings();
    if s.backend == Backend::WebKit {
        s.backend = Backend::Http;
    }
    s
}

/// JobStreet, Bayt, IrishJobs and stepstone.at are always read in a browser,
/// and stepstone.de's, stepstone.be's and Totaljobs' job
/// pages are (`detail`); without the window, only Camoufox is one.
pub(super) fn reachable(source: Source, detail: bool, settings: &Settings) -> Result<(), String> {
    let always = matches!(
        source,
        Source::JobStreet | Source::Bayt | Source::IrishJobs | Source::StepStoneAustria
    );
    let pages = matches!(
        source,
        Source::StepStone | Source::Totaljobs | Source::StepStoneBelgium
    );
    let browser_only = always || (pages && detail);
    if browser_only && settings.backend != Backend::Camoufox {
        let what = if detail {
            format!("{} job pages need", source.label())
        } else {
            format!("{} needs", source.label())
        };
        return Err(format!(
            "{what} a browser: through Claude, only Camoufox (Settings → \
             Backend) works; the built-in one runs in the JobScout window"
        ));
    }
    Ok(())
}
