# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-08-02

### Added

- Navigation rail leading with each bucket's count, set large because the counts
  are the reading the application exists to give
- Responsive multi-column grid, so a wide window shows every account at once
  instead of one narrow column
- Reciprocity glyph on every row: two dots, filled where a follow exists, which
  states the relationship without relying on which tab you are on
- Filter field, with the keyboard shortcuts to reach it
- Keyboard control throughout: digits switch buckets, Escape peels back one
  layer at a time, Enter opens the action sheet
- Single Proceed control opening an action sheet, which lists every selected
  account and offers the choices as cards carrying their consequence
- Toasts with an offer of undo after unfollowing, which restores exactly the
  accounts the batch touched
- Freshness read-out showing how long ago the lists were refreshed

### Changed

- Dark palette on a warm violet-charcoal ground, with surface steps kept inside
  the narrow band professional dark interfaces use, and translucent hairlines
  that compose correctly over every surface
- Bundled Inter and JetBrains Mono. Account names are set in mono because a
  login is an identifier
- Row height and column width tuned for density, roughly tripling how many
  accounts fit on screen

## [0.1.1] - 2026-08-01

### Fixed

- Connection strings that name no host, such as `postgresql:///goodbye`, now
  fall back to the local socket. `psql` applies that fallback in libpq, but the
  driver does not, and refused with "both host and hostaddr are missing". The
  short form is the natural one to write, so it is now honoured.

## [0.1.0] - 2026-08-01

### Added

- Comparison of followers against following, sorted into four buckets: not
  following back, keeping, mutuals, and fans
- Bulk unfollow with a confirmation dialogue listing every selected account
- Bulk follow back from the fans tab
- Keep-list persisted in PostgreSQL, keyed on the immutable GitHub account id so
  protection survives an account rename
- Reversal of any keep-list decision from the keeping tab
- Sync history recorded on every run, so follow-graph trends can be reported later
- Token borrowed from the `gh` command line tool, so the application stores no
  credential of its own
- Grey scale interface with semantic text colouring, all of it above the WCAG AA
  contrast threshold
