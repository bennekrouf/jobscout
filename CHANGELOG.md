# Changelog

What changed in each version of JobScout, for the people using it. Also
published at <https://mayorana.ch/en/apps/jobscout/releases>.

## [0.1.29] - 2026-10-30

### Changed

- Build and packaging only — no user-visible change.

## [0.1.28] - 2026-10-30

### Changed

- Build and packaging only — no user-visible change.

## [0.1.27] - 2026-10-28

### Added

- JobScout now shows the occasional message from mayorana.ch, under the bar at
  the top, until you close it.

### Changed

- The update banner and the occasional message from mayorana.ch now sit just
  under the bar at the top of the window instead of covering it, and look the
  same in every mayorana app. Download opens the new build in your browser and
  shows its SHA-256 when one is published, to compare with the file you
  downloaded.
- The Pro window looks and reads the same in every mayorana app. Once you're Pro
  it names your plan and email, and pressing Enter in the licence number field
  activates it.
- The Pro window shows the searches used this month on every plan, Pro included.

## [0.1.26] - 2026-10-28

### Changed

- Build and packaging only — no user-visible change.

## [0.1.25] - 2026-10-28

### Changed

- The bar across the top of the window now looks and works the same in every
  mayorana app: the name and version on the left, and on the right Profile,
  Watches, the scoring model, Settings, MCP, Feedback, the light/dark switch and
  the Pro badge in the corner.
- The scoring model is in the bar, with its region's flag; a click opens
  Settings to change it.
- The Feedback link moved from the bottom of the search panel into the bar, and
  the light/dark switch is part of the bar instead of floating over it.

## [0.1.24] - 2026-10-28

### Changed

- Build and packaging only — no user-visible change.

## [0.1.23] - 2026-10-27

### Added

- The setup modal now includes steps to connect JobScout to Codex, alongside
  Claude.

## [0.1.22] - 2026-10-27

### Changed

- Build and packaging only — no user-visible change.

## [0.1.21] - 2026-10-26

### Added

- Claude can now switch where your jobs are scored: ask it to score in
  Europe, China or the United States, or with Ollama on your computer, as
  the buttons in Settings → Scoring model do. An open JobScout window picks
  up the change within a couple of seconds.

## [0.1.20] - 2026-10-26

### Changed

- Through Claude, JobScout now works hand in hand with cvenom. Asking for
  your CV opens it from cvenom, and asking to adapt or generate a CV for a
  job hands the job's link and description to cvenom, which tailors your
  real CV and returns the PDF, instead of Claude writing a CV of its own.
  Connect the cvenom MCP server alongside JobScout to use it.

## [0.1.19] - 2026-10-25

### Fixed

- Installing silently, as IT does for an unattended update with `/VERYSILENT`,
  no longer hangs when JobScout is already installed: setup waited on a
  question nobody could see. It now replaces the installed version without
  asking, and stops with an error, instead of waiting, when the Microsoft
  Edge WebView2 Runtime is missing.

## [0.1.18] - 2026-10-25

### Changed

- Activating a licence now shows a small spinner while it is checked, then
  a window that says you're Pro, with the AI credits left and the renewal
  date in small type below. Removing the licence and checking it again no
  longer sit beside it; Manage subscription is a small link. Press Esc or
  the × to close it.
- Text in buttons, fields and the rest of the window is now set in the same
  monospaced typeface as GitAgent and the other Mayorana apps.
- The new-version banner and the usage-statistics notice now look the same
  as in GitAgent: a blue bar across the top, or the bottom for the notice,
  with a Download link and a × to hide it.
- Every action button is now 30px tall, borders included, like the
  regular buttons beside it.

## [0.1.17] - 2026-10-23

### Changed

- Dialogs now have a title bar with a × to close them, and text fields have
  the same size and typeface as in GitAgent.
- An open dialog now dims the light/dark switch in the corner along with the
  rest of the window, instead of leaving it lit on top.
- The buttons along the top of the window now all have the same height,
  borders included, and the light/dark switch matches them.

### Fixed

- Exporting jobs to CSV now creates the folder if it does not exist yet, and
  a failed export says why in English, not only in Windows' display language.

## [0.1.16] - 2026-10-23

### Changed

- Buttons now have the same size, rounded corners and typeface as in
  GitAgent, so they look alike across the Mayorana apps.

## [0.1.15] - 2026-10-22

### Removed

- The "DeepSeek, your own key" button is gone from Settings: api0 scores in
  Europe, China or the United States, or a local Ollama does. A model set by
  hand keeps working.

## [0.1.14] - 2026-10-22

### Added

- Settings offers three places to score in: Europe, China and the United
  States, each one click with its flag. They run on your licence's AI credits
  with no API key to set up, and each shows about how many credits a score
  costs, so you can weigh them before scoring a list. Ollama on this computer
  sits beside them, and DeepSeek with your own key is still one click away.
  New installs score in Europe, and so do settings left on api0's plan
  default: you see which region scores, and can pick another.

## [0.1.13] - 2026-10-22

### Changed

- Plans now differ by searches, not watches: every country is open on every
  plan, and you keep as many watches as you like. The free version runs 10
  searches a month; Basic 50, Pro 500 and Max 1000. A search from the
  sidebar or from Claude counts as one, and a watch's run only when it finds
  new jobs, across all countries, and the count starts again on the 1st. The sidebar shows how
  many are left; the Pro window shows your plan's searches.

### Added

- Watch results by email. Tick Email on a watch and each run that finds
  something sends you the jobs too, with their score and a link, at your
  licence's email. Any licence works, including the free AI credits.

## [0.1.12] - 2026-10-22

### Added

- Score jobs with api0's model, paid for by your licence's AI credits, free
  or Pro. It is now the scoring model: no API key to set up, since it uses
  the licence number from the Pro window. If you were on DeepSeek as set up
  out of the box, JobScout switches you to api0; a model you chose yourself
  is kept, and DeepSeek is one click away under Scoring model in Settings.

### Changed

- One licence number for every Mayorana app on this computer. Paste it in any
  of them and the others are Pro too the next time they start, with nothing to
  paste again; a number already pasted in one app is shared the same way.
  Removing the licence in the Pro window removes it from all of them.

## [0.1.11] - 2026-10-21

### Added

- Free AI credits every month, without Pro. Enter your email under Get free
  AI credits in the Pro window and the licence number arrives by email; paste
  it in the same window. The window then shows how many credits are left this
  month. The free version's limits stay as they are.

## [0.1.10] - 2026-10-21

### Changed

- JobScout Pro now comes with a monthly plan on mayorana.ch instead of a
  one-time licence key. Paste the licence number from the plan's email (like
  ABCDE-FGHJK-LMNPQ-RSTUV) in the Pro window. JobScout checks it when it
  starts and once a day, and Pro keeps working for a week without a
  connection.
- The Pro window shows your plan, the AI credits left this month and when the
  plan renews. Manage subscription opens the page where you change or cancel
  it.

## [0.1.9] - 2026-10-20

### Added

- Feedback and mayorana.ch links at the bottom of the sidebar. Feedback opens
  the contact form with JobScout and your version already filled in, so an
  idea or a problem reaches us in two clicks.

### Changed

- Profile, Watches, Pro, MCP and Settings moved from the bottom of the
  sidebar to a bar across the top of the window, so the sidebar keeps to the
  search.

### Fixed

- On Windows, the taskbar and the window's title bar show the JobScout icon
  instead of a generic one.
- On Windows, the country buttons show flags instead of two-letter codes
  such as CH and SG. The flags now look the same on every system.

## [0.1.8] - 2026-10-20

### Added

- Search by contract type. Tick Permanent (CDI), Freelance or both in the
  sidebar and the search keeps to those contracts on the boards that can
  filter by them: LinkedIn, France Travail, Arbeitsagentur and Reed. The
  sidebar names the ones available in the country you have loaded; the other
  boards still bring every contract. With both ticked, Arbeitsagentur and
  Reed cannot leave out fixed-term jobs, so they may show some. Watches keep
  the contract types of the search they were made from.

### Changed

- The country buttons at the top of the sidebar show only their flags, so
  they fit in a row or two and leave more room for the search. Hover a flag
  to see which country it is.

## [0.1.7] - 2026-10-19

### Fixed

- A new search no longer shows the jobs from your previous searches mixed in
  with its own. Once you search, the list shows only what that search found,
  including jobs you already had, and the status tabs and filter still work
  inside it. Click "Show all jobs" above the list to see every job again.
  Watches running in the background leave the list as it is.

## [0.1.6] - 2026-10-19

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
