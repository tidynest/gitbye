# GitBye

There is a species of GitHub user who follows you, waits politely for you to
follow back, and then leaves. Not because you disappointed them. They were never
staying. You were a number.

The manoeuvre is simple enough that a small script can do it, and frequently
does. Follow a few hundred people. Some fraction follow back out of ordinary
politeness. Wait a week or two, unfollow the lot, and repeat. The end state is a
profile reading 4,000 followers and 12 following, which is meant to be read as
"important person, extremely selective", and is in fact read as "person who has
discovered a for-loop".

GitBye keeps the receipts.

It watches who followed whom first, and when. If somebody follows you, you
return the courtesy, and they slip away shortly afterwards, GitBye notices and
can quietly undo your half of the arrangement. Their follower count returns to
being an honest description of reality. Everyone moves on with their lives.

That is the point of this application. It is not a tool for culling people who
have not followed you back, and it takes no view on whether anybody owes you a
follow. Plenty of the best accounts on GitHub follow nobody at all. The thing
being measured here is not popularity, it is whether a follow was ever meant.

## What it actually does

It records the shape of your follow graph every time it syncs, so it can tell
the difference between these two things, which look identical in a snapshot:

- somebody you followed who never followed back (fine, normal, their prerogative)
- somebody who followed you, collected the follow-back, and left (the thing)

Only the second is ever acted on automatically. That distinction is the entire
reason there is a database involved.

## Buckets

| Tab | Contents | Actions |
| --- | -------- | ------- |
| Not following back | followed, does not follow back, not on the keep-list | Unfollow, Keep |
| Keeping | followed, does not follow back, on the keep-list | Stop keeping, Unfollow |
| Mutuals | followed, follows back | Unfollow, Keep |
| Fans | follows you, not followed by you | Follow back |

These are for looking, and for the occasional deliberate tidy-up. Nothing here
happens on its own.

The keep-list is for the accounts you follow because you want to, and whose
opinion of you is beside the point. Maintainers of libraries you depend on, that
one person who posts brilliant things and has never noticed you exist, and so
on. Put them on the keep-list and the automation will never touch them.

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

Note that `gh auth token` returns `$GITHUB_TOKEN` when that variable is set, in
preference to the token in the keyring. If you export one, that is the token
this application receives, and the scope has to be present on it. `gh auth
refresh` cannot add a scope to a token it does not manage, so it will appear to
succeed while changing nothing that this application sees. Check which scopes
the effective token carries with:

```bash
curl -sS -I -H "Authorization: Bearer $(gh auth token)" https://api.github.com/user | grep -i x-oauth-scopes
```

Note also that a launcher entry does not inherit your shell environment. A
terminal that exports `GITHUB_TOKEN` in its profile passes it on, whereas the
same application started from the launcher falls back to `gh auth token` and may
receive a different token entirely. The surest fix is to grant the scope to the
token `gh` itself holds, so every way of starting the application agrees:

```bash
env -u GITHUB_TOKEN gh auth refresh -h github.com -s user:follow
```

Either way the application checks on each sync and says so in a banner before
you select anything, instead of failing once a batch is under way.

### 2. Database

The record of who moved first lives in PostgreSQL, along with the keep-list and
the sync history. On Arch Linux, if no cluster has been initialised yet:

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

Create the database:

```bash
createdb gitbye
```

That is the whole setup. The application looks for `postgresql:///gitbye` unless
told otherwise, which uses the local unix socket and peer authentication, so no
password is stored anywhere and nothing needs exporting.

Set `DATABASE_URL` only to point somewhere else:

```bash
export DATABASE_URL="postgresql://user@host/other"
```

Note that an exported variable reaches the application only when it is started
from a shell that has it. A launcher entry inherits no shell environment, which
is exactly why the default exists.

Tables are created on first run. There is no migration step.

If the server is unreachable, the application still starts and every list still
works, but unfollowing is disabled. An empty keep-list and an unloadable
keep-list look identical in a set difference, and one of those means unfollowing
accounts that were meant to be spared.

### 3. Window placement

A Wayland client cannot position its own window, so placement belongs to the
compositor. Add these to `~/.config/hypr/hyprland.conf`:

```
windowrule = float on, match:class ^(gitbye)$
windowrule = center on, match:class ^(gitbye)$
```

The window then opens floating and centred, and drags with the stock `SUPER`
plus left-drag binding.

Note the rule matches the **app id**, `gitbye`, which stays lowercase even
though the application presents itself as GitBye. `windowrulev2` is deprecated
in Hyprland 0.56 and is silently ignored when loaded from a config file, so a
rule written that way appears to be accepted while doing nothing.

## Install

Puts `gitbye` on your path and adds it to the application launcher:

```bash
cargo build --release && install -Dm755 "$(cargo metadata --format-version 1 --no-deps | grep -o '"target_directory":"[^"]*"' | cut -d'"' -f4)/release/gitbye" ~/.local/bin/gitbye
```

```bash
install -Dm644 assets/gitbye.desktop ~/.local/share/applications/gitbye.desktop && install -Dm644 assets/gitbye.svg ~/.local/share/icons/hicolor/scalable/apps/gitbye.svg && update-desktop-database ~/.local/share/applications
```

Then `gitbye` from a terminal, or GitBye from the launcher.

### Why this window does not wait for the display

A compositor pings each window and declares it dead after a few unanswered
replies. Hyprland pings about once a second and gives up after five.

Presenting a frame normally waits for the compositor to hand back a buffer to
draw into. A window on a hidden workspace is never given one, because there is
nothing to present to. Waiting for it happens inside the event loop, which is
also what answers the pings, so the window stops replying and gets reported as
not responding while being perfectly healthy.

This is why switching workspaces triggered it, and why the dialog appeared
shortly after each launch: the opening sync animates, and an animating window
keeps trying to present.

The window therefore runs with `vsync: false`, so presenting returns
immediately. Nothing is lost by it. The interface is static, redraws only when
something changes, and the compositor still synchronises what reaches the
screen. Animations are paced explicitly instead, at roughly sixty frames a
second, since the display is no longer doing the pacing.

There is no per-window escape hatch to use instead: `noanr` is not a valid rule
in Hyprland 0.56, which rejects it with `invalid field type noanr`. The only
alternative is `misc:enable_anr_dialog = false`, which silences the warning for
every application including ones that have genuinely hung.

## Modes

```
gitbye                  open the window
gitbye --sweep          run the unattended rule once, then exit
gitbye --dry-run        say what --sweep would do, changing nothing
gitbye --grace 6w       judge this run against a different window
gitbye --set-grace 6w   change the stored window, then exit
gitbye --help           usage
```

## The unattended sweep

This is the part that deals with the follow-and-run. It unfollows an account
only when every one of these holds:

- they followed you first, and this application watched it happen
- you followed them back
- they have since unfollowed you
- less than the sweep window passed between their follow and their unfollow
- they are not on the keep-list

All five. A person who followed you, was followed back, and stayed is not
touched, whatever they do later. Somebody who drifts off after a year has simply
changed their mind, which people are allowed to do. The rule is aimed at the
narrow case where the follow was a transaction and the unfollow was the second
half of it.

**A follow you began is never withdrawn automatically.** If you followed someone
first, that was your decision, and only you undo it. Those accounts still appear
in "Not following back" for manual action, and the automation ignores them
entirely.

Coming back restarts the clock, so a leave-and-return cannot be used to run out
the window. The obvious next move, refollowing right before the deadline, is
therefore just the same manoeuvre performed twice as slowly.

### The window

Ten weeks by default. It is a judgement about how long a follow has to last
before it counts as sincere, so it is yours to set, from one day to five years.

Ten weeks is deliberately generous. Nobody stumbles into it by accident: a
follow that lasted two months was a real follow, and if it ends after that then
something ordinary happened. The people this catches are typically gone in
under a fortnight, because the whole approach depends on cycling quickly.

Change it in the window under History, where the plot shows what has happened
and the control sets the rule for what happens next. Or from the command line:

```bash
gitbye --set-grace 6w
```

Written in days, weeks, months or years: `45d`, `6w`, `3m`, `1y`, or a bare
number of days. A month is thirty days and a year is three hundred and
sixty-five, stated here because neither is a fixed length and the window is a
rule of thumb rather than a date.

In the window, the count steps one at a time by button or by drag, and the unit
sits beside it. Choosing a different unit converts the window rather than
reinterpreting the count, so six weeks becomes forty-two days rather than six
days. Every figure between the bounds is reachable.

Anything outside one day to five years is refused rather than quietly clamped,
because a window of nothing sweeps everyone who ever left and a window nothing
can fall inside disables the sweep without saying so.

The figure lives in PostgreSQL beside the keep-list, so the timer, the command
line and the window all judge against the same one.

To try a figure without adopting it, pass it to a single run:

```bash
gitbye --dry-run --grace 6w
```

That governs the one run and leaves the stored window untouched. Note that the
rule is applied when the sweep runs, so shortening the window makes accounts
eligible that were not before. `--dry-run` is how to see that before it acts.

### What it cannot know

The sweep can only judge relationships it observed from the beginning. Anything
that already existed when you first ran the application is recorded as unknown
and is permanently exempt, because there is no way to discover who moved first
after the fact.

This means a fresh install does nothing at all for a while, which is correct and
slightly disappointing. Every run reports the exemption so the silence is never
mistaken for a verdict.

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

## A note on proportion

This is a follow button. Nobody has been wronged here, and the correct emotional
response to being farmed is mild amusement rather than a grudge.

The reason to run this is not revenge, it is accuracy. A follower count is
supposed to mean something, and it stops meaning anything when a portion of it
is manufactured. GitBye just declines to keep subsidising the illusion, on your
behalf, automatically, so you never have to think about it again.

Then it gets out of the way, which is more than can be said for the people it
is designed to notice.

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
