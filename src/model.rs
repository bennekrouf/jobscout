//! What JobScout keeps: job posts, the search that finds them, and settings.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::services::{licence, llm};

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
    /// Whether its search can be limited to `SearchParams::contract`.
    pub fn filters_contract(self) -> bool {
        matches!(
            self,
            Source::LinkedIn | Source::Arbeitsagentur | Source::Reed | Source::FranceTravail
        )
    }

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

/// Contract types a search can ask for. Only the boards where
/// `Source::filters_contract` holds can filter on them; the others search as
/// if none were picked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Contract {
    /// CDI, unbefristet: open-ended employment.
    Permanent,
    Freelance,
}

impl Contract {
    pub const ALL: [Contract; 2] = [Contract::Permanent, Contract::Freelance];

    pub fn label(self) -> &'static str {
        match self {
            Contract::Permanent => "Permanent (CDI)",
            Contract::Freelance => "Freelance",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchParams {
    pub keywords: String,
    pub posted: Posted,
    pub onsite: bool,
    pub remote: bool,
    pub hybrid: bool,
    pub experience: Vec<u8>,
    /// Empty = any contract.
    pub contract: Vec<Contract>,
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
            contract: vec![],
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
    /// Also email what a run finds to the licence's email, through api0.
    #[serde(default)]
    pub email: bool,
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
            email: false,
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
    /// Set once settings from before api0 was the default have been looked at,
    /// so DeepSeek picked again afterwards stays picked. Missing from a file
    /// saved before then, so false there, while new settings start true.
    #[serde(default)]
    pub api0_default_applied: bool,
    /// Set once settings from before api0's regions have been looked at, so
    /// "default" typed again afterwards stays. False in a file saved before
    /// then, like the flag above.
    #[serde(default)]
    pub region_default_applied: bool,
}

impl Settings {
    /// Moves settings still on the old DeepSeek default to api0, whose free
    /// credits every user has. A model the user chose is kept. Whether
    /// anything changed, to save it.
    ///
    /// Then moves api0's "default" model, which left the choice to the plan,
    /// to the first region, so Settings shows which one scores.
    pub fn apply_api0_default(&mut self) -> bool {
        let mut changed = false;
        if !self.api0_default_applied {
            self.api0_default_applied = true;
            if self.llm_base_url.trim_end_matches('/') == "https://api.deepseek.com"
                && self.llm_model == "deepseek-chat"
                && self.llm_key_env == "DEEPSEEK_API_KEY"
            {
                self.llm_base_url = licence::LLM_BASE_URL.into();
                self.llm_model = llm::REGIONS[0].id.into();
                self.llm_key_env = licence::LLM_KEY_ENV.into();
            }
            changed = true;
        }
        if !self.region_default_applied {
            self.region_default_applied = true;
            if llm::is_api0(&self.llm_base_url) && self.llm_model.trim() == licence::LLM_MODEL {
                self.llm_model = llm::REGIONS[0].id.into();
            }
            changed = true;
        }
        changed
    }
}

impl Settings {
    /// The api0 region scoring runs in, if it is set to one.
    pub fn region(&self) -> Option<&'static llm::Region> {
        if !llm::is_api0(&self.llm_base_url) {
            return None;
        }
        llm::REGIONS.iter().find(|r| r.id == self.llm_model.trim())
    }

    /// One click in Settings: api0, in this region.
    pub fn use_region(&mut self, id: &str) {
        self.llm_base_url = licence::LLM_BASE_URL.into();
        self.llm_model = id.into();
        self.llm_key_env = licence::LLM_KEY_ENV.into();
    }

    /// Whether scoring runs on ollama on this computer.
    pub fn is_ollama(&self) -> bool {
        self.llm_base_url.contains(":11434")
    }

    /// One click in Settings: ollama on this computer. A region or api0's
    /// "default" is no ollama model, so the model is left to be typed.
    pub fn use_ollama(&mut self) {
        if self.region().is_some() || self.llm_model == licence::LLM_MODEL {
            self.llm_model.clear();
        }
        self.llm_base_url = OLLAMA_URL.into();
        self.llm_key_env.clear();
    }

    /// The scoring model in a few words, for the overview.
    pub fn scoring_model(&self) -> String {
        match self.region() {
            Some(r) => format!("api0 in {}", r.label),
            None => format!("{} at {}", self.llm_model, self.llm_base_url),
        }
    }
}

const OLLAMA_URL: &str = "http://localhost:11434/v1";

impl Default for Settings {
    fn default() -> Self {
        Self {
            backend: Backend::Http,
            delay_ms: 2500,
            python: "python3".into(),
            headed: false,
            llm_base_url: licence::LLM_BASE_URL.into(),
            llm_model: llm::REGIONS[0].id.into(),
            llm_key_env: licence::LLM_KEY_ENV.into(),
            france_travail_client_id: String::new(),
            france_travail_secret_env: "FRANCE_TRAVAIL_CLIENT_SECRET".into(),
            reed_key_env: "REED_API_KEY".into(),
            country: Country::default(),
            detail_ttl_hours: 12,
            search_cache_minutes: 10,
            sidebar_width: 270.0,
            list_width: 480.0,
            api0_default_applied: true,
            region_default_applied: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn old_default() -> Settings {
        Settings {
            llm_base_url: "https://api.deepseek.com".into(),
            llm_model: "deepseek-chat".into(),
            llm_key_env: "DEEPSEEK_API_KEY".into(),
            api0_default_applied: false,
            region_default_applied: false,
            ..Settings::default()
        }
    }

    #[test]
    fn settings_saved_before_api0_are_moved_to_it() {
        let saved: Settings = serde_json::from_str(
            r#"{"llm_base_url":"https://api.deepseek.com","llm_model":"deepseek-chat","llm_key_env":"DEEPSEEK_API_KEY"}"#,
        )
        .unwrap();
        assert_eq!(saved, old_default());
        let mut s = saved;
        assert!(s.apply_api0_default());
        assert_eq!(s.llm_base_url, licence::LLM_BASE_URL);
        assert_eq!(s.llm_key_env, licence::LLM_KEY_ENV);
        assert!(!s.apply_api0_default());
    }

    #[test]
    fn a_chosen_model_is_kept() {
        let mut ollama = Settings {
            llm_base_url: "http://localhost:11434/v1".into(),
            llm_key_env: String::new(),
            ..old_default()
        };
        assert!(ollama.apply_api0_default());
        assert_eq!(ollama.llm_base_url, "http://localhost:11434/v1");
    }

    #[test]
    fn api0_default_from_before_the_regions_moves_to_europe_once() {
        let saved: Settings = serde_json::from_str(
            r#"{"llm_base_url":"https://gateway.api0.ai/v1","llm_model":"default","llm_key_env":"API0_API_KEY","api0_default_applied":true}"#,
        )
        .unwrap();
        let mut s = saved;
        assert!(s.apply_api0_default());
        assert_eq!(s.region().map(|r| r.id), Some("europe"));
        // "default" typed again afterwards stays.
        s.llm_model = "default".into();
        assert!(!s.apply_api0_default());
        assert_eq!(s.llm_model, "default");
    }

    #[test]
    fn a_model_other_than_api0_default_is_not_moved_to_a_region() {
        let mut ollama: Settings = serde_json::from_str(
            r#"{"llm_base_url":"http://localhost:11434/v1","llm_model":"default","llm_key_env":"","api0_default_applied":true}"#,
        )
        .unwrap();
        assert!(ollama.apply_api0_default());
        assert_eq!(ollama.llm_model, "default");
    }

    #[test]
    fn a_region_is_api0_with_the_region_as_its_model() {
        let mut s = old_default();
        assert_eq!(s.region(), None);
        s.use_region("china");
        assert_eq!(s.region().map(|r| r.id), Some("china"));
        assert_eq!(s.scoring_model(), "api0 in China");
        // api0 with a model named by hand is not a region.
        s.llm_model = "default".into();
        assert_eq!(s.region(), None);

        s.use_region("usa");
        s.use_ollama();
        assert!(s.is_ollama() && s.region().is_none());
        assert!(s.llm_model.is_empty() && s.llm_key_env.is_empty());
    }

    #[test]
    fn new_settings_score_in_europe() {
        assert_eq!(Settings::default().region().map(|r| r.id), Some("europe"));
    }

    #[test]
    fn deepseek_picked_again_after_the_move_stays() {
        let mut s = Settings {
            api0_default_applied: true,
            region_default_applied: true,
            ..old_default()
        };
        assert!(!s.apply_api0_default());
        assert_eq!(s.llm_model, "deepseek-chat");
    }
}
