# Sources of truth

Every fact this application acts on comes from exactly one authority. This lists
them, says what each one is authoritative for, and says what must never be
treated as authoritative instead.

A test enforces the database half of this: `tests/sources_of_truth.rs` reads the
schema out of `src/db.rs` and fails if a table exists that is not described
below. A table cannot be added without this document being updated in the same
commit.

## The live follow graph

**Authority: the GitHub REST API.** `/user/following` and `/user/followers`,
read fresh on every sync.

Nothing local is ever treated as the current graph. The stored history records
what was true at each sync, which is a different claim: it says what was seen and
when, not what is so now. Every bucket on screen is computed from the live read.

## Credentials

**Authority: `gh`.** The token is borrowed at startup by running `gh auth token`,
and `GITHUB_TOKEN` overrides it when set, because that is what `gh` itself does.

This application stores no credential, writes none to disk, and has no
configuration file for one. If the token is wrong, the fix is in `gh`.

## The database

`postgresql:///gitbye` unless `DATABASE_URL` says otherwise. Tables are created
on first run; there is no migration step.

### `keep_list`

**Authority for: who is protected from automation.** Keyed on the immutable
GitHub account id, never the login, so protection survives a rename.

An unreadable keep-list is never treated as an empty one. The two are
indistinguishable in a set difference, and one of them means unfollowing people
who were meant to be spared, so failure disables unfollowing entirely.

### `relationship`

**Authority for: who moved first, and when.** One row per account, holding the
first time each of the three events was observed: they followed, you followed
back, they left.

This is the only basis on which anything is unfollowed unattended. Attribution
is recorded as it happens and never inferred afterwards: a relationship that
already existed when the record began is stored as `unknown` and is permanently
exempt, because the order genuinely cannot be recovered.

### `sync_run` and `sync_member`

**Authority for: what the graph looked like at each past sync.** `sync_run` is
one row per sync with its timestamp; `sync_member` is the membership of both
sides at that moment.

These drive the History plot and nothing else. They are a record of observations,
never a substitute for a live read.

### `preference`

**Authority for: the sweep window.** One row, keyed `sweep_window_seconds`.

Stored rather than held per caller so that the timer, the command line and the
window cannot each judge against a different figure. Absent means the shipped
default. `--grace` overrides it for a single run without writing, which is what
makes `--dry-run --grace` safe to experiment with.

## Application state

**Authoritative for nothing.** Everything in `GitbyeApp` is derived: the buckets
from the live graph and the keep-list, the history from the store, the window
being edited from `preference`.

The one deliberate exception is the pair holding the sweep window while it is
being edited. It is kept apart from the stored figure so a value can be dragged
through without every intermediate number being written, and it is only adopted
back from the store when the stored figure actually changed. Re-deriving it on
every write once made whole ranges of values unreachable.

## Window placement

**Authority: the compositor.** A Wayland client cannot position itself, so the
size hints in the source are hints and the `windowrule` lines in the compositor
configuration decide the outcome.

## Backups

**Authority for: nothing, deliberately.** The snapshots in `../gitbye-backups/`
are read-only records kept outside the repository so that resetting or
re-cloning cannot destroy them. Nothing reads them automatically. They exist so
a person can see what was true before a bulk change, and put it back by hand.
