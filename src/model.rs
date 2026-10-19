//! What JobScout keeps: job posts, the search that finds them, and settings.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    #[default]
    New,
    Saved,
    Applied,
    Hidden,
}

impl Status {
    pub const ALL: [Status; 4] = [Status::New, Status::Saved, Status::Applied, Status::Hidden];

    pub fn label(self) -> &'static str {
        match self {
            Status::New => "New",
            Status::Saved => "Saved",
            Status::Applied => "Applied",
            Status::Hidden => "Hidden",
        }
    }

    pub fn class(self) -> &'static str {
        match self {
            Status::New => "new",
            Status::Saved => "saved",
            Status::Applied => "applied",
            Status::Hidden => "hidden",
        }
    }
}

/// The market being searched. Each country is its own context: its own
/// sources, jobs, watches, profile and last search, kept in its own folder
/// (see `services::store`). Nothing found in one shows up in the other.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Country {
    #[default]
    Switzerland,
    Singapore,
    Germany,
    France,
    UnitedKingdom,
    Uae,
    Ireland,
    Belgium,
    Austria,
}

impl Country {
    pub const ALL: [Country; 9] = [
        Country::Switzerland,
        Country::Singapore,
        Country::Germany,
        Country::France,
        Country::UnitedKingdom,
        Country::Uae,
        Country::Ireland,
        Country::Belgium,
        Country::Austria,
    ];

    /// Folder name under the data directory.
    pub fn code(self) -> &'static str {
        match self {
            Country::Switzerland => "ch",
            Country::Singapore => "sg",
            Country::Germany => "de",
            Country::France => "fr",
            Country::UnitedKingdom => "uk",
            Country::Uae => "ae",
            Country::Ireland => "ie",
            Country::Belgium => "be",
            Country::Austria => "at",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Country::Switzerland => "Switzerland",
            Country::Singapore => "Singapore",
            Country::Germany => "Germany",
            Country::France => "France",
            Country::UnitedKingdom => "United Kingdom",
            Country::Uae => "United Arab Emirates",
            Country::Ireland => "Ireland",
            Country::Belgium => "Belgium",
            Country::Austria => "Austria",
        }
    }

    /// The name on the flag button, where a long one would be cut off.
    pub fn short_label(self) -> &'static str {
        match self {
            Country::Uae => "UAE",
            c => c.label(),
        }
    }

    pub fn flag(self) -> &'static str {
        match self {
            Country::Switzerland => "🇨🇭",
            Country::Singapore => "🇸🇬",
            Country::Germany => "🇩🇪",
            Country::France => "🇫🇷",
            Country::UnitedKingdom => "🇬🇧",
            Country::Uae => "🇦🇪",
            Country::Ireland => "🇮🇪",
            Country::Belgium => "🇧🇪",
            Country::Austria => "🇦🇹",
        }
    }

    pub fn sources(self) -> &'static [Source] {
        match self {
            Country::Switzerland => &[Source::LinkedIn, Source::JobsCh, Source::Jobup],
            Country::Singapore => &[Source::LinkedIn, Source::Mcf, Source::JobStreet],
            Country::Germany => &[Source::LinkedIn, Source::Arbeitsagentur, Source::StepStone],
            Country::France => &[Source::LinkedIn, Source::FranceTravail],
            Country::UnitedKingdom => &[Source::LinkedIn, Source::Totaljobs, Source::Reed],
            Country::Uae => &[Source::LinkedIn, Source::Bayt],
            Country::Ireland => &[Source::LinkedIn, Source::IrishJobs],
            Country::Belgium => &[Source::LinkedIn, Source::StepStoneBelgium],
            Country::Austria => &[Source::LinkedIn, Source::StepStoneAustria],
        }
    }
}

/// Where a job was found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Source {
    #[default]
    LinkedIn,
    /// jobs.ch, the largest Swiss board (German-speaking first).
    JobsCh,
    /// jobup.ch: same JobCloud platform as jobs.ch, French-speaking first.
    Jobup,
    /// MyCareersFuture, Singapore's government job board. Employers who
    /// sponsor an Employment Pass generally advertise here first.
    Mcf,
    /// JobStreet (SEEK), the largest commercial board in Singapore. Behind
    /// Cloudflare: always read through the Camoufox browser.
    JobStreet,
    /// The Bundesagentur für Arbeit's Jobbörse, Germany's public employment
    /// service and its largest board.
    Arbeitsagentur,
    /// StepStone, the largest commercial board in Germany. Behind Akamai:
    /// job pages are read through a browser.
    StepStone,
    /// France Travail, France's public employment service, through its
    /// official API: needs credentials in Settings.
    FranceTravail,
    /// Totaljobs, the UK's largest commercial board: the StepStone platform,
    /// so read like StepStone. robots.txt allows only its first search page.
    Totaljobs,
    /// Reed, through its official API: needs a key in Settings.
    Reed,
    /// Bayt, the largest board in the Middle East, for the UAE. Behind
    /// Cloudflare: always read through a browser.
    Bayt,
    /// IrishJobs, Ireland's largest board: the StepStone platform, so read like
    /// StepStone, but always through a browser.
    IrishJobs,
    /// stepstone.be and stepstone.at: the StepStone platform again. robots.txt
    /// allows only their first search page; Austria's refuses plain HTTP.
    StepStoneBelgium,
    StepStoneAustria,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::LinkedIn => "LinkedIn",
            Source::JobsCh => "jobs.ch",
            Source::Jobup => "jobup.ch",
            Source::Mcf => "MyCareersFuture",
            Source::JobStreet => "JobStreet",
            Source::Arbeitsagentur => "Arbeitsagentur",
            Source::StepStone => "StepStone",
            Source::FranceTravail => "France Travail",
            Source::Totaljobs => "Totaljobs",
            Source::Reed => "Reed",
            Source::Bayt => "Bayt",
            Source::IrishJobs => "IrishJobs",
            Source::StepStoneBelgium => "stepstone.be",
            Source::StepStoneAustria => "stepstone.at",
        }
    }

    pub fn class(self) -> &'static str {
        match self {
            Source::LinkedIn => "src-linkedin",
            Source::JobsCh => "src-jobsch",
            Source::Jobup => "src-jobup",
            Source::Mcf => "src-mcf",
            Source::JobStreet => "src-jobstreet",
            Source::Arbeitsagentur => "src-arbeitsagentur",
            Source::StepStone => "src-stepstone",
            Source::FranceTravail => "src-francetravail",
            Source::Totaljobs => "src-totaljobs",
            Source::Reed => "src-reed",
            Source::Bayt => "src-bayt",
            Source::IrishJobs => "src-irishjobs",
            Source::StepStoneBelgium | Source::StepStoneAustria => "src-stepstone",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Job {
    /// The source's own posting id — the dedupe key. LinkedIn's are numeric,
    /// JobCloud's are UUIDs shared by jobs.ch and jobup.ch, so a posting
    /// listed on both is one job here.
    pub id: String,
    /// Older saved jobs predate sources; they all came from LinkedIn.
    #[serde(default)]
    pub source: Source,
    pub title: String,
    pub company: String,
    pub location: String,
    pub url: String,
    /// Posting date as LinkedIn lists it (yyyy-mm-dd).
    #[serde(default)]
    pub posted: Option<String>,
    #[serde(default)]
    pub salary: Option<String>,
    pub found_at: DateTime<Utc>,
    /// The search that first found it.
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub status: Status,
    #[serde(default)]
    pub detail: Option<JobDetail>,
    /// Last time the job page could not be read (removed, or a block page):
    /// not asked for again until `DETAIL_RETRY_AFTER` has passed.
    #[serde(default)]
    pub detail_failed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub score: Option<Score>,
    /// A watch already raised a notification for it.
    #[serde(default)]
    pub notified: bool,
}

impl Job {
    /// The posting as plain text, for the clipboard: a header, the criteria,
    /// then the description when it has been fetched.
    pub fn as_text(&self) -> String {
        let mut out = format!("{}\n{} · {}\n", self.title, self.company, self.location);
        if let Some(p) = &self.posted {
            out.push_str(&format!("Posted {p}\n"));
        }
        if let Some(s) = &self.salary {
            out.push_str(&format!("{s}\n"));
        }
        out.push_str(&format!("{}\n", self.url));
        if let Some(d) = &self.detail {
            if !d.criteria.is_empty() {
                out.push('\n');
                for (k, v) in &d.criteria {
                    out.push_str(&format!("{k}: {v}\n"));
                }
            }
            out.push('\n');
            out.push_str(&d.description);
            out.push('\n');
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JobDetail {
    /// Plain text, never HTML: the webview shares a process with the app, so
    /// markup from a third-party page is not rendered as markup.
    pub description: String,
    #[serde(default)]
    pub criteria: Vec<(String, String)>,
    #[serde(default)]
    pub applicants: Option<String>,
    #[serde(default)]
    pub posted_ago: Option<String>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Score {
    pub score: u8,
    pub verdict: String,
    #[serde(default)]
    pub strengths: Vec<String>,
    #[serde(default)]
    pub gaps: Vec<String>,
    pub scored_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Posted {
    Any,
    Day,
    #[default]
    Week,
    Month,
}

impl Posted {
    pub const ALL: [Posted; 4] = [Posted::Any, Posted::Day, Posted::Week, Posted::Month];

    /// LinkedIn's `f_TPR` value.
    pub fn code(self) -> Option<&'static str> {
        match self {
            Posted::Any => None,
            Posted::Day => Some("r86400"),
            Posted::Week => Some("r604800"),
            Posted::Month => Some("r2592000"),
        }
    }

    /// The same window in days, for sources filtered on our side.
    pub fn days(self) -> Option<i64> {
        match self {
            Posted::Any => None,
            Posted::Day => Some(1),
            Posted::Week => Some(7),
            Posted::Month => Some(30),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Posted::Any => "Any time",
            Posted::Day => "Past 24 hours",
            Posted::Week => "Past week",
            Posted::Month => "Past month",
        }
    }
}

/// LinkedIn's `f_E` experience levels.
pub const EXPERIENCE: [(u8, &str); 6] = [
    (1, "Internship"),
    (2, "Entry"),
    (3, "Associate"),
    (4, "Mid-Senior"),
    (5, "Director"),
    (6, "Executive"),
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchParams {
    pub keywords: String,
    pub posted: Posted,
    pub onsite: bool,
    pub remote: bool,
    pub hybrid: bool,
    pub experience: Vec<u8>,
    /// Pages per source: 10 jobs a page on LinkedIn, about 20 elsewhere.
    pub pages: u32,
    pub sources: Vec<Source>,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self::for_country(Country::default())
    }
}

impl SearchParams {
    /// A first search in `country`: the whole country, on all its sources.
    pub fn for_country(country: Country) -> Self {
        Self {
            keywords: String::new(),
            posted: Posted::Week,
            onsite: false,
            remote: false,
            hybrid: false,
            experience: vec![],
            pages: 3,
            sources: country.sources().to_vec(),
        }
    }
}

/// A saved search that runs on its own every `every_hours`, scores what is
/// new and raises a notification for anything at or above `min_score`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Watch {
    pub id: String,
    pub search: SearchParams,
    pub every_hours: u32,
    pub min_score: u8,
    pub enabled: bool,
    #[serde(default)]
    pub last_run: Option<DateTime<Utc>>,
    #[serde(default)]
    pub last_result: Option<String>,
}

impl Watch {
    pub const INTERVALS: [u32; 6] = [1, 3, 6, 12, 24, 72];

    pub fn new(search: SearchParams) -> Self {
        Self {
            id: Utc::now().timestamp_millis().to_string(),
            search,
            every_hours: 6,
            min_score: 75,
            enabled: true,
            last_run: None,
            last_result: None,
        }
    }

    pub fn name(&self) -> String {
        self.search.keywords.trim().to_string()
    }

    pub fn is_due(&self, now: DateTime<Utc>) -> bool {
        self.enabled
            && self
                .last_run
                .is_none_or(|at| now - at >= Duration::hours(self.every_hours as i64))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Backend {
    /// Plain HTTP against LinkedIn's public guest endpoints.
    #[default]
    Http,
    /// The same URLs, loaded in a Camoufox browser through helper/camoufox_fetch.py.
    Camoufox,
    /// The same URLs, loaded in the WebKit view macOS ships (services::webkit).
    /// Nothing to install.
    WebKit,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub backend: Backend,
    /// Minimum gap between two LinkedIn requests; up to half as much again is
    /// added at random.
    pub delay_ms: u64,
    pub python: String,
    pub headed: bool,
    /// Any OpenAI-compatible chat endpoint: DeepSeek, OpenAI, ollama's /v1 …
    pub llm_base_url: String,
    pub llm_model: String,
    /// Name of the environment variable holding the API key — the key itself
    /// is never written to disk. Empty for a local model.
    pub llm_key_env: String,
    /// France Travail's API needs an application registered at
    /// francetravail.io: its client id, and the name of the environment
    /// variable holding its secret (never written to disk, like the LLM key).
    pub france_travail_client_id: String,
    pub france_travail_secret_env: String,
    /// Name of the environment variable holding a Reed API key.
    pub reed_key_env: String,
    /// The country whose context is loaded. The search itself is kept per
    /// country, next to its jobs.
    pub country: Country,
    /// How long a fetched description counts as fresh. Opening a job with an
    /// older one shows it and refreshes it in the background.
    pub detail_ttl_hours: u64,
    /// How long a search results page is reused for the same URL.
    pub search_cache_minutes: u64,
    /// Column widths in px, set by dragging the splitters. The job pane
    /// takes the rest.
    pub sidebar_width: f64,
    pub list_width: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            backend: Backend::Http,
            delay_ms: 2500,
            python: "python3".into(),
            headed: false,
            llm_base_url: "https://api.deepseek.com".into(),
            llm_model: "deepseek-chat".into(),
            llm_key_env: "DEEPSEEK_API_KEY".into(),
            france_travail_client_id: String::new(),
            france_travail_secret_env: "FRANCE_TRAVAIL_CLIENT_SECRET".into(),
            reed_key_env: "REED_API_KEY".into(),
            country: Country::default(),
            detail_ttl_hours: 12,
            search_cache_minutes: 10,
            sidebar_width: 270.0,
            list_width: 480.0,
        }
    }
}
