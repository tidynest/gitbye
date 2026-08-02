# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.3] - 2026-08-02

### Fixed

- The window is no longer reported as "not responding". Presenting a frame
  waited for the compositor to return a buffer, which never arrives for a window
  on a hidden workspace, and that wait happens inside the event loop that also
  answers the compositor's liveness pings. Switching workspaces was enough to
  trigger it. Presenting no longer waits, which costs nothing on an interface
  that redraws only when something changes
- Animations are paced explicitly at roughly sixty frames a second, rather than
  redrawing as fast as the processor allows now that the display no longer paces
  them

### Changed

- The README explains why presenting does not wait, and records that Hyprland
  0.56 has no per-window `noanr` rule. It replaces an earlier note blaming CPU
  throttling, which was wrong: the dialog reproduced at normal priority on an
  idle machine, with every application thread asleep when it fired

## [0.5.2] - 2026-08-02

### Fixed

- The database connection is opened from a worker thread on first use rather
  than during startup. Connecting on the interface thread blocked the first
  frame, so an unreachable server was indistinguishable from a hung application
- A database that becomes reachable later is now picked up on the next sync
  instead of staying unavailable until restart
- The reason a store failed is carried through to the banner, rather than only
  the fact that it did

## [0.5.1] - 2026-08-02

### Fixed

- Mutuals was read-only, so an account that followed you back could not be
  unfollowed without first losing that status. Every bucket you follow from now
  offers Unfollow
- Keeping offered only Stop keeping, so parting with a shielded account took two
  trips. It now offers Unfollow directly. The keep-list shields an account from
  the scheduled sweep, never from you

### Added

- Keep is available on Mutuals, shielding an account while they still
  reciprocate so they are spared automatically if they ever stop

## [0.5.0] - 2026-08-02

### Changed

- Row marks now use the same pair as the launcher icon: a heart where affection
  exists, filled when returned and hollow when only offered, and a skull over
  crossed bones where a follow is one-sided and unprotected. That last set is
  exactly what the application exists to act on, so it is the one that looks
  like a warning
- The origin chevron moved alongside the symbol, since there is no longer a
  connector for it to sit on
- Presented name is GitBye. The binary, the Wayland app id and the database stay
  lowercase `gitbye`, because the compositor rule and the desktop entry match on
  the app id

## [0.4.0] - 2026-08-02

### Added

- History view, reached from the rail or with `5`: a plot of following and
  followers across every recorded sync, and a reverse-chronological list of what
  changed. Scaled to the range the data occupies rather than to zero, since a
  zero-based axis would hide the only thing the plot is for
- Recent changes derived from the relationship record rather than by diffing
  syncs. A start is listed only where the beginning was observed, because a
  first-sync timestamp records when the application looked, not when anything
  happened. A departure always counts, since leaving happens between two syncs

## [0.3.1] - 2026-08-02

### Added

- Origin marker on every row: an arrowhead on the connector pointing away from
  whoever moved first, so a follow you began is visible at a glance. Nothing is
  drawn where the beginning was never observed, since an absent mark is the
  honest rendering of an absent fact
- Hover text naming what the marker means, including that the scheduled sweep
  never touches a follow you began

## [0.3.0] - 2026-08-02

### Added

- Unattended sweep: `gitbye --sweep` runs the rule once and exits, `--dry-run`
  reports what it would do without touching GitHub. systemd units ship in
  `assets/systemd/`, defaulting to a dry run so enabling the timer arms a report
  rather than an unfollow
- Relationship history recording who moved first, updated on every sync. A
  relationship already mutual when first seen stays unknown permanently and is
  never eligible for automation, because the order is not recoverable after
  the fact
- Desktop entry and icon, so the application appears in the launcher
- `--help`

### Changed

- Renamed to GitBye. The binary, the Wayland app id and the database are all
  `gitbye` now, so a compositor rule matching the old class needs updating, as
  does `DATABASE_URL`

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

- Connection strings that name no host, such as `postgresql:///gitbye`, now
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
