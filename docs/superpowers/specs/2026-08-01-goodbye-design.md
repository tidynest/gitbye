# goodbye: design

Date: 2026-08-01
Version: 0.1.0
Status: approved, in build

## Overview

A desktop GUI that compares the GitHub accounts you follow against the accounts
following you, sorts them into buckets, and lets you bulk unfollow the ones that
never followed back, while honouring a persisted keep-list of accounts you want to
follow regardless of whether they reciprocate.

Target account: `tidynest` (id `195969110`). At design time: 116 following,
59 followers, 58 not following back, 58 mutuals, 1 fan.

### Goals

- See at a glance who does not follow back, and who does
- Select any subset of non-followers and unfollow them in one action
- Spare chosen accounts from that list, persisted across restarts, and reverse that
  decision at any time so a change of heart is never blocked by the tool
- Follow back accounts that follow you but that you do not follow
- Retain a history of every sync, so follow-graph trends over time can be reported

### Planned, deliberately not built in version 0.1.0

- Analytics and trend reporting over the retained history
- Scheduling, automation, or unattended runs

Version 0.1.0 records the data these need but ships no user interface for them.
History is the one thing that cannot be recovered retroactively, so capture starts
now, while the reporting that reads it waits until it is actually wanted.

### Non-goals

- Any platform other than GitHub
- Multi-account support

## Engineering standards

Binding for every file in this repository.

1. No AI attribution anywhere: commits, code, comments, documentation.
2. No em-dashes and no en-dashes anywhere. Hyphens, commas and brackets only.
3. British English for everything we name: identifiers, comments, documentation,
   commit messages. Third-party APIs keep their own spelling, so `Color32` from
   egui and `html_url` from the GitHub API stay as they are.
4. `cargo nextest run` for tests, never `cargo test`.
5. `cargo fmt`, `cargo clippy` and `cargo nextest run` all clean before any commit.
   No warnings tolerated. Short conventional names such as `ctx` are exempt.
6. Tests live outside the code under test, in `tests/`, without exception. No
   `#[cfg(test)] mod tests` blocks inside source files. This is why the crate is a
   library with a thin binary on top: integration tests can only reach a public
   library API.
7. Test-driven. Tests are reasoned through before they are written, and they are
   correct on the first run. Coverage targets basic functionality, safety and
   stability, and stops there.
8. No duplicated code at three lines or longer. Extract a helper instead.
9. No nested `if` statements. Use early returns, `let ... else`, match, or iterator
   combinators.
10. Minimal dependencies, maximal standard library. YAGNI and KISS.
11. Semantic versioning, recorded in `Cargo.toml` and `CHANGELOG.md`, with a git
    tag per release.

## Safety constraints

Hard requirements, not preferences.

1. Follow and unfollow endpoints are never invoked during development. They exist
   in source only, driven by explicit user action in the interface.
2. A read-only snapshot of both lists lives at `~/RustroverProjects/goodbye-backups/`,
   outside this repository, with a documented restore procedure.
3. If the keep-list cannot be loaded, unfollow is disabled. An empty keep-list and
   an unloadable keep-list are indistinguishable in a set difference, and one of
   those means unfollowing accounts that were meant to be spared.

## Authentication

The application owns no credential. At startup it shells out once:

```
gh auth token
```

`gh` is already authenticated as `tidynest`, with the token held in the operating
system keyring. Nothing is written to disk, there is no configuration file, no
`keyring` dependency, and no token to paste per launch.

SSH keys cannot be used. SSH authenticates git transport only. The REST API needs a
bearer token, and GitHub withdrew API password authentication in 2021.

### Required scope

Reading followers and following needs no special scope. Writes need `user:follow`,
which the current token lacks. One-time setup:

```
gh auth refresh -h github.com -s user:follow
```

A 403 on a write raises a banner naming that exact command.

## Architecture

egui via eframe, with no asynchronous runtime. Blocking work runs on plain
`std::thread` workers reporting back over an `mpsc` channel.

Rejected: tokio with sqlx, because concurrency buys nothing at roughly forty
requests and `sqlx::query!` demands a live database at compile time. Rejected:
Tauri, because a node toolchain and an IPC layer is a heavy price for four lists
and a set of tick boxes.

### Layout

```
goodbye/
  Cargo.toml
  CHANGELOG.md
  README.md
  docs/
  src/
    lib.rs         module declarations and the public API surface
    main.rs        binary entry point, boots eframe
    model.rs       User, Buckets, bucketing, Msg
    github.rs      REST client and token retrieval
    db.rs          PostgreSQL: schema, keep-list, history
    theme.rs       the palette and its application to egui Visuals
    app.rs         AppState, worker dispatch, frame update
    ui.rs          tab rendering and the confirmation dialogue
  tests/
    bucketing.rs   the set difference and keep-list interaction
```

### Dependencies

`eframe`, `reqwest` (`blocking`, `json`, `rustls-tls`), `serde` (`derive`),
`postgres`, `anyhow`. Five, and no more without a written reason.

## Data model

```rust
struct User { id: i64, login: String }
```

`id` is the key everywhere, including in PostgreSQL. GitHub permits account
renames and the login string travels with the account, so a keep-list keyed on a
login silently stops protecting somebody the day they rename, and the next sweep
would unfollow them without warning. `login` is a cached display value, refreshed
on every sync.

No avatar field. Avatars would pull in `egui_extras`, an image loader, and 116
network fetches, to decorate a list that is already unambiguous by username.

### Schema

Applied at startup, idempotently. No migration tooling.

```sql
CREATE TABLE IF NOT EXISTS keep_list (
    id       BIGINT PRIMARY KEY,
    login    TEXT        NOT NULL,
    note     TEXT,
    added_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS sync_run (
    id       BIGSERIAL   PRIMARY KEY,
    taken_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS sync_member (
    run_id    BIGINT NOT NULL REFERENCES sync_run(id) ON DELETE CASCADE,
    direction TEXT   NOT NULL CHECK (direction IN ('following', 'follower')),
    user_id   BIGINT NOT NULL,
    login     TEXT   NOT NULL,
    PRIMARY KEY (run_id, direction, user_id)
);
```

`keep_list` serves the live feature. `sync_run` and `sync_member` are the history
capture: one row per sync, one row per member per direction, roughly 175 rows per
sync at current scale. Normalised rather than a JSON blob, so future trend queries
are ordinary SQL rather than document unpacking.

Connection string comes from `DATABASE_URL`. If it is unset the application does
not guess and does not fall back to another store. It starts with the keep-list
unavailable, shows the banner, and disables unfollow, exactly as when the server
is unreachable.

## Buckets

One pure function, no I/O, the only genuinely non-trivial logic in the codebase:

```rust
pub fn bucket(following: &[User], followers: &[User], keep: &[i64]) -> Buckets
```

| Bucket | Definition |
| ------ | ---------- |
| Not following back | `following - followers - keep_list` |
| Keeping | `(following - followers) & keep_list` |
| Mutuals | `following & followers` |
| Fans | `followers - following` |

Reciprocation wins over the keep-list, so a keep-list row for an account that
follows back appears under Mutuals rather than Keeping. The row is not deleted,
it simply lies dormant, and protection resumes the moment that account stops
reciprocating. Keeping therefore lists exactly who is being shielded right now,
which is the list worth reviewing before a purge.

The keep-list subtracts from the actionable bucket, so Select All on the first tab
is safe by construction rather than by care.

## Interface

Four tabs.

| Tab | Actions |
| --- | ------- |
| Not following back | tick to select, then Unfollow selected or Keep selected |
| Keeping | Stop keeping, which returns the row to the first tab |
| Mutuals | read-only |
| Fans | tick to select, then Follow selected |

One selection model drives two bulk actions on the first tab. Adding to the
keep-list reuses the same tick boxes as unfollowing, so there is no second
interaction pattern to learn. Stop keeping is the reverse of Keep selected, so no
decision made in this application is one-way.

Clicking a row opens the profile through `ctx.open_url()`. No browser-launching
dependency.

### Confirmation gate

Unfollow and follow both raise a modal listing every selected account by name with
the total count, offering Cancel and a confirm button. The visible list is the real
safety check. No typed confirmation.

### Palette

Neutral scale is achromatic, as specified: very light grey, grey, black. Colour
appears only in text, and only to carry meaning.

| Role | Value |
| ---- | ----- |
| Window background | `#EDEDED` very light grey |
| Panel surface | `#F7F7F7` |
| Sunken surface | `#E0E0E0` |
| Border | `#BFBFBF` grey |
| Primary text | `#141414` black |
| Secondary text | `#5E5E5E` grey |

| Semantic | Value | Applied to |
| -------- | ----- | ---------- |
| Reciprocated | `#0F6D63` teal | Mutuals |
| Unreciprocated | `#A8432A` terracotta | Not following back |
| Informational | `#3B4CA8` indigo | Fans |
| Protected | `#6A3D9A` violet | Keeping |
| Failure | `#A32020` red | Errors and failed rows |

Teal against terracotta is the complementary pair carrying the central opposition
of the application, reciprocated against unreciprocated. Indigo and violet are
neighbours of the teal, keeping the whole set within one harmonious sweep rather
than scattering unrelated hues.

The chrome (header, tab bar, action bar) is filled with `#EDEDED` and the list
with `#F7F7F7`. Two neutral tones separate controls from data more quietly than a
rule or a border would, which is why the window carries no dividing lines.

Every semantic colour was checked against the `#F7F7F7` panel surface and clears
the WCAG AA threshold of 4.5 to 1 for body text: teal 5.78, terracotta 5.60,
indigo 7.04, violet 7.13, red 7.04, secondary grey 6.05, primary text 17.2. A
colour is never the sole carrier of meaning, since each bucket also has its own
tab and heading.

### Window placement

A Wayland client cannot position its own window. `xdg-shell` has no set-position
request for toplevels, so `.with_position()` is inert under Hyprland and placement
belongs to the compositor.

Application side:

```rust
egui::ViewportBuilder::default()
    .with_app_id("goodbye")
    .with_inner_size([900.0, 640.0])
```

Compositor side, documented in the README:

```
windowrulev2 = float, class:^(goodbye)$
windowrulev2 = center, class:^(goodbye)$
```

Dragging then uses the stock Hyprland binding, `SUPER` with left-drag. Rejected: an
undecorated window with a hand-drawn title bar calling `ViewportCommand::StartDrag`,
which is roughly forty lines and would make this the only application on the system
with bespoke drag behaviour.

## Data flow

The interface thread never blocks. One pattern serves every background job.

```
interface thread                worker thread
   |                                 |
   |-- spawn(job) ------------------>|
   |                                 |-- reqwest::blocking / postgres
   |<-- Msg over mpsc::Sender -------|
   |-- ctx.request_repaint()         |
```

```rust
pub enum Msg {
    Synced { following: Vec<User>, followers: Vec<User> },
    KeepList(Vec<User>),
    Progress { done: usize, total: usize, login: String },
    Finished { ok: usize, failed: Vec<(String, String)> },
    Error(String),
}
```

`AppState` drains the receiver with `try_recv()` at the top of every frame. The
channel is the synchronisation, so there is no mutex and no shared lock.

Jobs: Sync (two paginated GETs, a keep-list SELECT, and a history INSERT),
Unfollow batch (N deletes), Follow batch (N puts).

## Error handling

| Domain | Policy |
| ------ | ------ |
| PostgreSQL unreachable or `DATABASE_URL` unset | Application starts. Banner shows the cause. Read tabs work. Unfollow disabled, follow stays enabled, since the keep-list only ever guards against unfollowing. |
| `gh auth token` fails | Startup error quoting the exact command that fixes it. |
| 403 on a write | Banner naming the `user:follow` scope and the `gh auth refresh` line. |
| Partial batch failure | Continue through the whole batch, collect every failure, report once at the end. |

Batch policy in detail: a failing entry does not abort the run. Each failure is
recorded as `(login, reason)` and the run continues. The completion summary lists
successes and every failure with its reason, so a token expiring mid-run produces
one readable report rather than a half-applied batch with no resume point.

## Testing

`tests/bucketing.rs`, driven by `cargo nextest run`. `bucket()` is pure, so there is
no mock HTTP server and no test database.

Cases:

- empty inputs
- total overlap
- zero overlap
- a keep-listed id appears in Keeping and is absent from Not following back
- a renamed login still matches by id

The API client is a thin wrapper, and a test over it would only assert that reqwest
works.

## Setup

PostgreSQL client tools are installed, the server is inactive, and the data
directory state is unknown without elevated privileges. The README covers both
initialising a cluster and starting an existing one, then creating the database and
exporting `DATABASE_URL`.

## Hosting

Private repository, mirrored to GitHub and GitLab, matching the existing workspace
pattern. Repository name: `goodbye`.

`goodbye-backups/` is deliberately outside this repository and is not tracked.
