# goodbye: design

Date: 2026-08-01
Status: approved for planning

## Overview

A desktop GUI that compares the GitHub accounts you follow against the accounts
following you, sorts them into buckets, and lets you bulk unfollow the ones that
never followed back, while protecting a persisted keep-list of accounts you want
to follow regardless.

Target account: `tidynest` (id `195969110`). At design time: 116 following,
59 followers, 58 not following back, 58 mutuals, 1 fan.

### Goals

- See at a glance who does not follow back, and who does
- Select any subset of non-followers and unfollow them in one action
- Permanently spare chosen accounts from that list, surviving restarts
- Follow back accounts that follow you but that you do not follow

### Non-goals

- Any platform other than GitHub
- Scheduling, automation, or unattended runs
- Analytics, history, or follow-graph trends over time
- Multi-account support

## Safety constraints

These are hard requirements, not preferences.

1. Follow and unfollow endpoints are never invoked during development. They exist
   in source only, driven by explicit user action in the GUI.
2. A read-only snapshot of both lists exists at `~/RustroverProjects/goodbye-backups/`,
   outside this repo, with a documented restore procedure.
3. If the keep-list cannot be loaded, unfollow is disabled. An empty keep-list and
   an unloadable keep-list are indistinguishable in a set difference, and one of
   them means unfollowing accounts that were meant to be spared.

## Authentication

The app owns no credential. At startup it shells out once:

```
gh auth token
```

`gh` is already authenticated as `tidynest` with the token held in the OS keyring.
No token is written to disk, no config file, no `keyring` crate, no paste-per-launch.

SSH keys cannot be used. SSH authenticates git transport only. The REST API needs a
bearer token, and GitHub removed API password auth in 2021.

### Required scope

Reading followers and following is public data and needs no special scope.
Unfollow and follow require `user:follow`, which the current token lacks.
One-time setup:

```
gh auth refresh -h github.com -s user:follow
```

A 403 on a write is surfaced as a banner naming this exact command.

## Architecture

egui via eframe, with no async runtime. Blocking I/O runs on plain
`std::thread` workers that report back over an `mpsc` channel.

Rejected: tokio plus sqlx (concurrency buys nothing at roughly 40 requests, and
`sqlx::query!` wants a live database at compile time). Rejected: Tauri (a node
toolchain and an IPC layer for three lists and a set of checkboxes).

### Files

```
goodbye/
  Cargo.toml
  README.md
  docs/
  src/
    main.rs      eframe boot, AppState, UI, worker spawn
    github.rs    REST client: following, followers, follow, unfollow
    db.rs        postgres: ensure_schema, load/add/remove keep-list
    model.rs     User, Buckets, set difference, Msg enum
```

### Dependencies

`eframe`, `reqwest` (`blocking`, `json`, `rustls-tls`), `serde` (`derive`),
`postgres`, `anyhow`. Five total.

## Data model

```rust
struct User { id: i64, login: String }
```

No avatar field. Rendering avatars would pull in `egui_extras` plus an image
loader and 116 network fetches, to decorate a list that is already unambiguous
by username.

`id` is the key everywhere, including in Postgres. GitHub allows account renames
and the login string moves with the account. A keep-list keyed on a login string
silently stops protecting someone the day they rename, and the next sweep would
unfollow them without warning. `login` is a cached display value, refreshed on
every sync.

### Schema

Applied at startup. No migration tooling.

```sql
CREATE TABLE IF NOT EXISTS keep_list (
    id         BIGINT PRIMARY KEY,
    login      TEXT        NOT NULL,
    note       TEXT,
    added_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

PostgreSQL is a stated requirement. Connection string comes from `DATABASE_URL`.
If that variable is unset, the app does not guess a connection string and does not
fall back to another store. It starts with the keep-list unavailable, shows the
banner, and disables unfollow, exactly as it does when the server is unreachable.

## Buckets

Computed by one pure function with no I/O:

```rust
fn bucket(following: &[User], followers: &[User], keep: &[i64]) -> Buckets
```

| Bucket | Definition |
| ------ | ---------- |
| Not following back | `following - followers - keep_list` |
| Keeping | `keep_list` |
| Mutuals | `following & followers` |
| Fans | `followers - following` |

The keep-list subtracts from the actionable bucket, so Select All on tab one is
always safe by construction.

## UI

Four tabs.

| Tab | Actions |
| --- | ------- |
| Not following back | checkbox select, then Unfollow selected or Keep selected |
| Keeping | Stop keeping, returns the row to tab one |
| Mutuals | read-only |
| Fans | checkbox select, then Follow selected |

One selection model drives two bulk actions on tab one. Adding to the keep-list
reuses the same checkboxes as unfollowing, so there is no second interaction
pattern to learn.

Clicking a row opens the profile via `ctx.open_url()`. No browser-launching crate.

### Confirm gate

Unfollow and follow both open a modal listing every selected account by name plus
the total count, with Cancel and a confirm button. The visible list is the real
safety check. No typed confirmation.

### Window placement

A Wayland client cannot position its own window. `xdg-shell` has no set-position
request for toplevels, so eframe's `.with_position()` is a no-op under Hyprland
and placement belongs to the compositor.

App side:

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

Dragging then uses the stock Hyprland bind, `SUPER` plus left-drag. Rejected: an
undecorated window with a hand-drawn title bar calling `ViewportCommand::StartDrag`,
which is roughly 40 lines and makes this the only app on the system with bespoke
drag behaviour.

## Data flow

The UI thread never blocks. One pattern serves all three background jobs.

```
UI thread                       worker thread
   |                                 |
   |-- spawn(job) ------------------>|
   |                                 |-- reqwest::blocking / postgres
   |<-- Msg over mpsc::Sender -------|
   |-- ctx.request_repaint()         |
```

```rust
enum Msg {
    Synced { following: Vec<User>, followers: Vec<User> },
    KeepList(Vec<User>),
    Progress { done: usize, total: usize, login: String },
    Finished { ok: usize, failed: Vec<(String, String)> },
    Error(String),
}
```

`AppState` drains the receiver with `try_recv()` at the top of each frame. The
channel is the synchronisation, so there is no mutex and no shared lock.

Jobs: Sync (two paginated GETs plus a keep-list SELECT), Unfollow batch
(N deletes), Follow batch (N puts).

## Error handling

| Domain | Policy |
| ------ | ------ |
| Postgres unreachable | App starts. Banner shows the error. Read tabs work. Unfollow disabled, Follow stays enabled, since the keep-list only ever protects against unfollowing. |
| `gh auth token` fails | Startup error quoting the exact command to fix it. |
| 403 on a write | Banner naming the `user:follow` scope and the `gh auth refresh` line. |
| Partial batch failure | Continue through the whole batch, collect every failure, report once at the end. |

Batch policy in detail: a failing entry does not abort the run. Each failure is
recorded as `(login, reason)` and the run finishes. The completion summary lists
successes and every failure with its reason, so a token that expires mid-run
produces one readable report rather than a half-applied batch with no resume point.

## Testing

One runnable check, covering the only non-trivial logic. `bucket()` is pure, so no
mock HTTP server and no test database.

Cases:

- empty inputs
- total overlap
- zero overlap
- a keep-listed id appears in Keeping and is absent from Not following back
- a renamed login still matches by id

The API client is a thin wrapper and a test over it would only assert that reqwest
works.

## Setup

PostgreSQL client tools are installed but the server is not running and the data
directory state is unknown. README covers both initialising a cluster and starting
an existing one, plus creating the database and exporting `DATABASE_URL`.

## Hosting

Private repository, mirrored to GitHub and GitLab, matching the existing workspace
pattern. Repository name: `goodbye`.

`goodbye-backups/` is deliberately not part of this repo and is not tracked.
