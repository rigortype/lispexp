---
name: lispexp-release-prep
description: Prepare a lispexp crates.io release by bumping the crate version, sealing the changelog, running release verification, and tagging so GitHub Actions publishes. Use when the user asks to prepare the next version, cut a release, refresh release metadata, or make versioned files consistent before tagging.
metadata:
  internal: true
---

# lispexp Release Prep

Follow this workflow to release a new `lispexp` version to [crates.io](https://crates.io/crates/lispexp).

**Publishing is automated.** A human/agent prepares the release locally (version
bump, changelog, verify, commit) and pushes a `vX.Y.Z` tag; the
[`release.yml`](../../../.github/workflows/release.yml) workflow then runs
`cargo publish` and creates the GitHub Release. You never run `cargo publish` or
handle the crates.io token by hand (a manual fallback is documented at the end).

## One-time setup (skip if already done)

- A crates.io API token is stored as the repository secret
  `CARGO_REGISTRY_TOKEN` (GitHub → Settings → Secrets and variables → Actions).
  Get the token from <https://crates.io/settings/tokens> with the `publish-new`
  and `publish-update` scopes.
- The first-ever publish claims the crate name; after that, `CARGO_REGISTRY_TOKEN`
  can be narrowed to `publish-update` only, or migrated to crates.io
  [Trusted Publishing](https://crates.io/docs/trusted-publishing) (OIDC, no
  stored secret) — a later hardening, not required to ship.

## Update release metadata

Decide the next semantic version first, then update all versioned files together.

Update:

- `Cargo.toml` — the `version` field.
- `CHANGELOG.md` — seal `[Unreleased]` into the new version section (below).

`Cargo.lock` is not tracked for this library, so there is no lockfile to bump.

### Seal the `[Unreleased]` entries — the load-bearing step

This is the highest-value, most-skipped part of a release, and `cargo test`
cannot check it. The changelog is written for humans; make it read like release
notes, not commit messages.

1. Read the whole `[Unreleased]` block. Classify each top-level bullet:
   release-style (leave) or commit-style (rewrite).
2. Rewrite every commit-style bullet — one self-contained sentence per bullet;
   move "why / how / measured numbers" into a child item (`  - …`); delete
   internal-only detail (private refactors, test additions) outright. Ask of each
   entry: "would a user of the crate care if they weren't reading the source?"
3. Consolidate: fold several commits' entries into one user-recognisable change;
   split any merge artefacts. A changelog entry is not a commit message.
4. Re-read the sealed section as a user would.

### Release mechanics

- Add a `## [x.y.z] - YYYY-MM-DD` section immediately below `## [Unreleased]`.
- Optionally open it with a 2–4 sentence prose summary (the release's themes)
  before the `###` sections.
- Use Keep a Changelog headings verbatim: `Added`, `Changed`, `Deprecated`,
  `Removed`, `Fixed`, `Security`. Group like changes; do not inline text into a
  heading; do not use `####` inside a version block.
- **Do not hard-wrap entries.** Each bullet and the summary paragraph is a single
  physical line, however long. `release.yml` extracts the section verbatim as the
  GitHub Release body, and wrapping degrades rendering there.
- Preserve the Keep a Changelog / Semantic Versioning note at the top and the
  release date in every version heading.
- Update the bottom-of-file links: point `[Unreleased]` at
  `compare/vx.y.z...HEAD` and add `[x.y.z]:
  https://github.com/rigortype/lispexp/releases/tag/vx.y.z`.

## Verify the release

Run before committing (this is exactly what `ci.yml` enforces, plus the package
check):

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo doc --no-deps
cargo publish --dry-run
git diff --check
```

`cargo publish --dry-run` packages and compiles the crate as crates.io will;
confirm it reports a small file count (the `tests/corpus/` submodules are
excluded via `Cargo.toml`'s `exclude`). If a check needs formatting or other
non-version cleanup, commit that separately — do not fold it into the version
bump.

## Commit

A single release-prep commit containing the `Cargo.toml` bump and the
`CHANGELOG.md` update:

```text
Bump up version to x.y.z
```

Keep any verification cleanup in earlier commits; the version bump is the final
release-prep commit.

## Push, then tag to publish

```sh
git push origin master              # runs ci.yml
gh run watch                        # wait for the CI gate to go green
git tag vx.y.z                      # tag the release commit
git push origin vx.y.z              # runs release.yml -> publishes + GitHub Release
gh run watch                        # watch the publish
```

The tag push triggers [`release.yml`](../../../.github/workflows/release.yml),
which checks the tag matches `Cargo.toml`, runs `cargo publish` with
`CARGO_REGISTRY_TOKEN`, and creates the GitHub Release from this version's
`CHANGELOG.md` section. Do not tag until `ci.yml` is green.

Optional: land the bump through a PR (`gh pr create --base master`) instead of a
direct push if you want the change reviewed; tag the merged commit afterwards.

## Manual fallback (if Actions is unavailable)

Publish from an up-to-date clean `master` at the release commit:

```sh
cargo login                         # paste a crates.io token, once
cargo publish
git tag vx.y.z && git push origin vx.y.z
gh release create vx.y.z --title vx.y.z \
  --notes "$(awk -v v=x.y.z '$0 ~ "^## \\["v"\\]"{p=1;next} p&&/^## \\[/{exit} p' CHANGELOG.md)"
```

## Quick checklist

- Working tree starts clean or every pending change is understood.
- `Cargo.toml` `version` equals the new `x.y.z`.
- Every former `[Unreleased]` bullet was classified and, if commit-style,
  rewritten; no bullet in the new section has two sentences, an internal-only
  detail, or a merge artefact. (Confirm by eye — CI cannot.)
- `[Unreleased]` / `[x.y.z]` links at the bottom of `CHANGELOG.md` resolve.
- `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`,
  `cargo doc`, and `cargo publish --dry-run` all pass.
- The final commit message is `Bump up version to x.y.z`.
- `ci.yml` is green before tagging; the `vx.y.z` tag matches `Cargo.toml`.
- After publish: the crate version is on crates.io, the `vx.y.z` tag is on
  `origin`, and the GitHub Release exists.
