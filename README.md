# Gitbye

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

Note that `gh auth token` returns `$GITHUB_TOKEN` when that variable is set,
in preference to the token in the keyring. If you export one, that is the token
this application receives, and the scope has to be present on it. `gh auth
refresh` cannot add a scope to a token it does not manage, so it will appear to
succeed while changing nothing that this application sees. Check which scopes
the effective token carries with:

```bash
curl -sS -I -H "Authorization: Bearer $(gh auth token)" https://api.github.com/user | grep -i x-oauth-scopes
```

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
createdb gitbye
```

```bash
export DATABASE_URL="postgresql:///gitbye"
```

That connection string uses the local unix socket and peer authentication, so no
password is stored anywhere. Put the export in your shell profile to make it
permanent.

Tables are created on first run. There is no migration step.

If the banner reports `DATABASE_URL is not set` even though you exported it,
check whether the terminal you launched from predates the export. A shell reads
`~/.zshrc` once, at startup, so an older terminal never sees a line added later.
Open a new one, or `source ~/.zshrc`.

If `DATABASE_URL` is unset or the server is unreachable, the application still
starts and every list still works, but unfollowing is disabled. An empty
keep-list and an unloadable keep-list look identical in a set difference, and one
of those means unfollowing accounts that were meant to be spared.

### 3. Window placement

A Wayland client cannot position its own window, so placement belongs to the
compositor. Add these to `~/.config/hypr/hyprland.conf`:

```
windowrulev2 = float, class:^(gitbye)$
windowrulev2 = center, class:^(gitbye)$
```

The window then opens floating and centred, and drags with the stock `SUPER`
plus left-drag binding.

## Install

Puts `gitbye` on your path and adds it to the application launcher:

```bash
cargo build --release && install -Dm755 "$(cargo metadata --format-version 1 --no-deps | grep -o '"target_directory":"[^"]*"' | cut -d'"' -f4)/release/gitbye" ~/.local/bin/gitbye
```

```bash
install -Dm644 assets/gitbye.desktop ~/.local/share/applications/gitbye.desktop && install -Dm644 assets/gitbye.svg ~/.local/share/icons/hicolor/scalable/apps/gitbye.svg && update-desktop-database ~/.local/share/applications
```

Then `gitbye` from a terminal, or Gitbye from the launcher.

## Modes

```
gitbye              open the window
gitbye --sweep      run the unattended rule once, then exit
gitbye --dry-run    say what --sweep would do, changing nothing
gitbye --help       usage
```

## The unattended sweep

The sweep returns follows that were never sincere. It unfollows an account only
when every one of these holds:

- they followed you first, and this application watched it happen
- you followed them back
- they have since unfollowed you
- less than ten weeks passed between their follow and their unfollow
- they are not on the keep-list

**A follow you began is never withdrawn automatically.** If you followed someone
first, that was your decision, and only you undo it. Those accounts still appear
in "Not following back" for manual action.

Coming back restarts the clock, so a leave-and-return cannot be used to run out
the ten weeks.

### What it cannot know

The sweep can only judge relationships it observed from the beginning. Anything
that already existed when you first ran the application is recorded as unknown
and is permanently exempt, because there is no way to discover who moved first
after the fact. That exemption is reported on every run.

### Scheduling it

```bash
install -Dm644 assets/systemd/gitbye-sweep.service ~/.config/systemd/user/gitbye-sweep.service && install -Dm644 assets/systemd/gitbye-sweep.timer ~/.config/systemd/user/gitbye-sweep.timer
```

```bash
systemctl --user daemon-reload && systemctl --user enable --now gitbye-sweep.timer
```

The shipped unit runs `--dry-run`, so enabling the timer arms a **report, not an
unfollow**. Watch it for a few days:

```bash
journalctl --user -u gitbye-sweep --since today
```

Once it selects who you expect, change `ExecStart` in the service file from
`--dry-run` to `--sweep` and run `systemctl --user daemon-reload`.

## Build and run from source

```bash
cargo run --release
```

## Tests

```bash
cargo nextest run
```

## Before committing

All four must be clean:

```bash
cargo fmt --check && cargo clippy --all-targets && cargo nextest run && cargo deny check
```

Clippy runs with `pedantic` denied, so warnings are build failures by design.
`cargo deny` reads `deny.toml`, which is also what the pre-push gate enforces.

## Backups

Read-only snapshots of the follow graph live in `../gitbye-backups/`,
deliberately outside this repository so that resetting or re-cloning it cannot
destroy the safety net. That directory documents how to take a fresh snapshot and
how to restore from one.

Take a snapshot before any bulk unfollow you are unsure about.
