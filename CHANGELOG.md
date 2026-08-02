# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.2] - 2026-08-02

### Changed

- The README describes what the application is for. It had opened by saying the
  point was unfollowing people who do not follow back, which is not the point,
  reads as though a follow is owed, and describes a tool nobody needs. The
  subject is the follow-and-run: an account that follows, collects the
  follow-back, and leaves, so that its ratio flatters it
- Two statements were out of date and are corrected: choosing a unit converts
  the window rather than reinterpreting the count, and the upper bound is five
  years rather than one
- The buckets are described as somewhere to look rather than as the feature, and
  the keep-list as what it is for, which is following people whose opinion of
  you is beside the point

## [0.7.1] - 2026-08-02

### Fixed

- Day counts that are multiples of seven no longer become unreachable. The unit
  was re-derived from the stored figure after every write, so setting seven days
  came back as one week and the next step jumped to fourteen, putting eight days
  out of reach entirely. The unit being edited in is now left alone unless the
  figure itself changed
- Choosing a unit converts the window instead of reinterpreting the count beside
  it. Six weeks becomes forty-two days, not six days. Combined with the above,
  moving from one week to days had produced one day and stored it immediately,
  which silently replaced the rule while only a unit was being chosen
- A conversion never rounds down to nothing, which would have produced a rule
  that sweeps everybody from a control asked only to change units

## [0.7.0] - 2026-08-02

### Added

- Windows can be written and adjusted in days, weeks, months or years, on the
  command line and in the window. A month is thirty days and a year is three
  hundred and sixty-five, stated rather than left to a calendar, because the
  window is a rule of thumb about sincerity and not a date
- The count is a value that can be typed or dragged, with steps of one, so every
  figure in range is reachable

### Changed

- The control stepped a whole week at a time, which put every figure that was
  not a multiple of seven out of reach of the buttons. It now moves one unit at
  a time, and the unit is chosen beside it
- Changing the unit keeps the count and reinterprets it, so six days becomes six
  weeks. That is how a value and its unit normally behave
- A window is named in the largest unit that divides it exactly, so ninety days
  reads as three months while seventy-one stays in days rather than being
  rounded into a figure the rule is not using
- The upper bound is five years rather than one, since years are now offered. A
  bound still exists so a stray keystroke cannot disable the sweep in silence

## [0.6.1] - 2026-08-02

### Fixed

- The store is found without `DATABASE_URL` being set. It defaults to
  `postgresql:///gitbye`, the local socket and this application's own database,
  which is what the setup steps asked everyone to export anyway. A launcher
  entry inherits no shell environment, so starting from the desktop had left the
  keep-list unreadable and unfollowing withheld, while the same build worked
  from a terminal

### Changed

- `DATABASE_URL` is now an override for pointing elsewhere rather than a
  requirement, and the setup steps lost a step
- A connection failure names the address it tried, which an unset variable
  never did

## [0.6.0] - 2026-08-02

### Added

- The sweep window is configurable, from the command line and from the window,
  having been fixed at ten weeks. `--set-grace 6w` stores a new one and
  `--grace 6w` governs a single run without adopting it, so a figure can be
  tried with `--dry-run` first
- A control under History sets it, placed where the plot already shows what the
  rule has been doing
- Windows are written in weeks or days, and named back in the unit they were
  most likely meant in
- The figure is stored in PostgreSQL beside the keep-list, so the timer, the
  command line and the window cannot each judge against a different one

### Changed

- Anything outside one day to a year is refused rather than clamped. A window of
  nothing withdraws a follow from anyone who ever left, and a window of years
  stops selecting anybody, so both are stated rather than silently accepted
- `should_sweep` takes the window as an argument instead of reading a constant

Closes #1

## [0.5.6] - 2026-08-02

### Fixed

- An empty "Not following back" no longer claims everyone reciprocates. The
  keep-list is subtracted from that bucket, so emptying it means nobody who
  fails to reciprocate is left unprotected, which is a different statement. With
  accounts in Keeping the old wording contradicted the count displayed directly
  beside it
- The empty state now reports how many kept accounts do not follow back, and
  reserves "All square" for when that number is genuinely zero

## [0.5.5] - 2026-08-02

### Fixed

- Toasts no longer cover the action bar. They were anchored to the window's
  bottom-right corner, which put them directly on top of Proceed, and a failure
  stays on screen for twenty seconds. The interface was still responding
  throughout, but the one control worth reaching was underneath the report of
  what had just happened, which reads as the application having seized up
- They are now placed inside the content region, which excludes the panels, so
  they float above the action bar and follow it if it ever changes height

## [0.5.4] - 2026-08-02

### Added

- The token is checked for the `user:follow` scope during each sync. Without it
  a banner names the command that grants it, and both follow and unfollow are
  withheld, rather than the problem surfacing only after a batch has been
  chosen, confirmed and attempted

### Changed

- The two capability flags are grouped, so the reason an action is unavailable
  is stated where it is decided rather than spread across loose booleans
- A token that reports no scopes at all is treated as permitted. Only classic
  tokens list their scopes, and refusing fine-grained ones would withdraw the
  feature from tokens that work

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
