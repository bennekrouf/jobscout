# Changelog

What changed in each version of JobScout, for the people using it. Also
published at <https://mayorana.ch/en/apps/jobscout/releases>.

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
  needs the Camoufox backend: the built-in browser only runs in the window.
- Germany. Pick the German flag to search LinkedIn and the Arbeitsagentur's
  Jobbörse, the federal employment agency's board and the largest in the
  country, with salaries and full descriptions where the employer gives them.
  Like Switzerland and Singapore, Germany keeps its own jobs, watches, profile
  and last search; nothing found there shows up in the other countries.

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
