# Changelog

What changed in each version of JobScout, for the people using it. Also
published at <https://mayorana.ch/en/apps/jobscout/releases>.

## [Unreleased]

### Added

- Added support for IrishJobs and StepStone in Belgium and Austria. You can now
  search and view jobs from these sources directly within JobScout.

## [0.1.5] - 2026-10-19

### Added

- France. Pick the French flag to search LinkedIn and France Travail, the
  public employment service and the largest job database in France, with full
  descriptions, contract type, experience and salary as the employer gives
  them. France Travail only lets apps in through its official API, so it
  needs a free application of your own: register one at francetravail.io,
  subscribe it to the "Offres d'emploi v2" API, then enter its client id in
  Settings and export its secret as `FRANCE_TRAVAIL_CLIENT_SECRET` (or the
  variable you name there). The secret is never saved. Until then, France
  searches LinkedIn only and the log says what is missing. Like the other
  countries, France keeps its own jobs, watches, profile, last search and
  activity log.
- United Kingdom. Pick the British flag to search LinkedIn, Totaljobs and
  Reed across the whole UK. Totaljobs is the country's largest commercial job
  board; it only lets apps read the first page of a search, so each search
  brings its 25 best matches from there. Its descriptions open in the built-in
  browser, so on Windows and Linux they need the Camoufox backend. Reed needs
  a free API key from reed.co.uk/developers: export it as `REED_API_KEY` (or
  the variable you name in Settings). The key is never saved, and until it is
  set Reed is skipped and the log says so. The UK keeps its own jobs,
  watches, profile, last search and activity log, like every country.
- United Arab Emirates. Pick the UAE flag to search LinkedIn and Bayt,
  the largest job board in the Middle East, across the whole UAE. Bayt opens
  in the built-in browser, searches included, so on Windows and Linux it
  needs the Camoufox backend. The UAE keeps its own jobs, watches, profile,
  last search and activity log, like every country.
- Ireland. Pick the Irish flag to search LinkedIn and IrishJobs, the
  country's largest job board, across all of Ireland, with salaries where the
  employer gives them. IrishJobs opens in the built-in browser, searches
  included, so on Windows and Linux it needs the Camoufox backend. Ireland
  keeps its own jobs, watches, profile, last search and activity log, like
  every country.
- Belgium and Austria. Pick the Belgian or Austrian flag to search LinkedIn
  and StepStone's local site, stepstone.be or stepstone.at. Each search
  brings the 25 best matches from StepStone, the most the site lets apps
  read. Descriptions open in the built-in browser, and in Austria searches
  do too, so on Windows and Linux they need the Camoufox backend. Each
  country keeps its own jobs, watches, profile, last search and activity log.

## [0.1.4] - 2026-10-18

### Added

- Added support for StepStone, Germany's largest commercial job board. You can
  now search and view jobs from StepStone directly within JobScout.

## [0.1.3] - 2026-10-18

### Added

- Use JobScout from Claude. The new **MCP** button at the bottom of the
  sidebar shows what to paste into Claude Desktop, or the one line to run for
  Claude Code, already pointing at where JobScout is installed. Claude can
  then search the boards, list and read the jobs you keep, score them against
  your profile, mark them Saved, Applied or Hidden, read and update your
  profile, manage your watches and export a list to CSV, in any country. What
  Claude changes shows up in the open window within a couple of seconds.
  Scoring uses the model and key set in Settings. Through Claude, JobStreet
  and StepStone's job descriptions need the Camoufox backend: the built-in
  browser only runs in the window.
- Germany. Pick the German flag to search LinkedIn, the Arbeitsagentur's
  Jobbörse (the federal employment agency's board, the largest in the
  country) and StepStone, Germany's biggest commercial job board. Salaries
  show where the employer gives them; on StepStone, a salary marked
  "StepStone estimate" is the site's own guess. StepStone descriptions open
  in the built-in browser, like JobStreet's, so on Windows and Linux they
  need the Camoufox backend. A job posted on more than one board is listed
  once. Like Switzerland and Singapore, Germany keeps its own jobs, watches,
  profile and last search; nothing found there shows up in the other
  countries.

### Changed

- The activity log now belongs to the country it happened in. Switching
  country shows that country's own searches and messages, and switching back
  brings the earlier ones back.

### Removed

- The Location field in the search form. The country you pick at the top is
  where JobScout searches, so typing it again was redundant. Searches cover
  the whole country; a town you had typed there before is no longer used.

## [0.1.2] - 2026-10-16

### Fixed

- JobScout has its own icon in the Dock, Finder, the Start menu and your
  Linux app launcher, instead of a blank or generic one.
- Switching country loads that country's own search. Before, the form kept
  the last country's search, so searching right after picking Singapore
  could fill Singapore's list with Swiss jobs from jobs.ch and jobup.

## [0.1.1] - 2026-10-16

### Added

- JobScout is available as a signed download for macOS (Apple Silicon and
  Intel), Windows and Linux on [JobScout's page](https://mayorana.ch/en/apps/jobscout).
  It tells you when a newer version is out, with a link to download it.
- JobScout Pro. The free version keeps one watch: one search that runs on its
  own and notifies you about good matches. Searching, scoring and export are
  never limited. Pro keeps any number of watches, in every country. Buy it on
  JobScout's page, then click **Get Pro…** at the bottom of the sidebar and
  paste the key. The key is checked on your computer: no account, nothing
  sent anywhere.
- A sun/moon button at the top right switches between light and dark. JobScout
  starts in the same mode as your computer and follows it when it changes,
  until you pick one with the button.
