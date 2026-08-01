# goodbye

A desktop application that compares the GitHub accounts you follow against the
accounts following you, then lets you unfollow the ones that never reciprocated.

Accounts you want to keep following regardless are stored in a keep-list, which
removes them from the unfollow list entirely. That decision is reversible at any
time.

## Buckets

| Tab | Contents | Actions |
| --- | -------- | ------- |
| Not following back | followed, does not follow back, not on the keep-list | Unfollow selected, Keep selected |
| Keeping | followed, does not follow back, on the keep-list | Stop keeping |
| Mutuals | followed, follows back | read only |
| Fans | follows you, not followed by you | Follow selected |

The keep-list is subtracted from the first tab, so Select All there is safe by
construction rather than by care.

## Requirements

- Rust 1.97 or newer
- `gh`, authenticated
- PostgreSQL, running
- `cargo-nextest`, for the test suite

## Setup

### 1. Token scope

The application borrows its token from `gh`, so there is no credential to
configure and nothing written to disk. Reading the follow graph needs no special
scope, but following and unfollowing need `user:follow`:

```bash
gh auth refresh -h github.com -s user:follow
```

Without it the application still starts and every list still works. Only the
follow and unfollow actions fail, with a banner naming this command.

### 2. Database

The keep-list and the sync history live in PostgreSQL. On Arch Linux, if no
cluster has been initialised yet:

```bash
sudo -iu postgres initdb -D /var/lib/postgres/data
```

Then start the server and grant your account access:

```bash
sudo systemctl enable --now postgresql
```

```bash
sudo -iu postgres createuser --superuser "$USER"
```

Create the database and point the application at it:

```bash
createdb goodbye
```

```bash
export DATABASE_URL="postgresql:///goodbye"
```

That connection string uses the local unix socket and peer authentication, so no
password is stored anywhere. Put the export in your shell profile to make it
permanent.

Tables are created on first run. There is no migration step.

If `DATABASE_URL` is unset or the server is unreachable, the application still
starts and every list still works, but unfollowing is disabled. An empty
keep-list and an unloadable keep-list look identical in a set difference, and one
of those means unfollowing accounts that were meant to be spared.

### 3. Window placement

A Wayland client cannot position its own window, so placement belongs to the
compositor. Add these to `~/.config/hypr/hyprland.conf`:

```
windowrulev2 = float, class:^(goodbye)$
windowrulev2 = center, class:^(goodbye)$
```

The window then opens floating and centred, and drags with the stock `SUPER`
plus left-drag binding.

## Build and run

```bash
cargo run --release
```

## Tests

```bash
cargo nextest run
```

## Before committing

All three must be clean:

```bash
cargo fmt --check && cargo clippy --all-targets && cargo nextest run
```

## Backups

Read-only snapshots of the follow graph live in `../goodbye-backups/`,
deliberately outside this repository so that resetting or re-cloning it cannot
destroy the safety net. That directory documents how to take a fresh snapshot and
how to restore from one.

Take a snapshot before any bulk unfollow you are unsure about.
