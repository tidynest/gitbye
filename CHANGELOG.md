# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
