#!/usr/bin/env bash
# Prepare a release: bump the version, date the CHANGELOG, commit on a
# release branch. Pushing, the PR, the merge and the tag stay manual; the
# tag is what publishes (.github/workflows/release.yml).
#
#   tools/release.sh 0.2.0
#
# Needs a clean tree and a non-empty "## [Unreleased]" section in
# CHANGELOG.md, which becomes the new version's notes.
set -euo pipefail
cd "$(dirname "$0")/.."

new=${1:?usage: $0 <new-version>}
# Cargo's rules, checked before anything changes: three numeric parts, no
# leading zeros, each within u64 (at most 19 digits keeps it there).
part='(0|[1-9][0-9]{0,18})'
if ! [[ $new =~ ^$part\.$part\.$part$ ]]; then
  echo "version must be MAJOR.MINOR.PATCH with no leading zeros, got '$new'" >&2
  exit 1
fi
old=$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)
# Refuse anything not strictly above the current version.
if [ "$(printf '%s\n%s\n' "$old" "$new" | sort -t. -k1,1n -k2,2n -k3,3n | tail -1)" != "$new" ] || [ "$old" = "$new" ]; then
  echo "new version $new is not above the current $old" >&2
  exit 1
fi
if [ -n "$(git status --porcelain)" ]; then
  echo "working tree is not clean" >&2
  exit 1
fi
if git rev-parse -q --verify "refs/tags/v$new" >/dev/null; then
  echo "tag v$new already exists" >&2
  exit 1
fi
if ! tools/changelog-section.sh Unreleased >/dev/null 2>&1; then
  echo "CHANGELOG.md has nothing under ## [Unreleased]; describe the release there first" >&2
  exit 1
fi

repo=https://github.com/DrJesseGlass/ic-auction
today=$(date -u +%Y-%m-%d)
branch="release-v$new"
git switch -c "$branch"

# Cargo.toml: the package version is the first version line.
awk -v new="$new" '!done && /^version = "/ { print "version = \"" new "\""; done = 1; next } { print }' \
  Cargo.toml > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
# Cargo.lock records the crate's own version too.
cargo update --workspace --offline --quiet

# CHANGELOG: Unreleased becomes the new version, a fresh Unreleased goes
# above it, and the compare links move along.
awk -v new="$new" -v old="$old" -v today="$today" -v repo="$repo" '
  $0 == "## [Unreleased]" { print; print ""; print "## [" new "] - " today; next }
  index($0, "[Unreleased]: ") == 1 {
    print "[Unreleased]: " repo "/compare/v" new "...HEAD"
    print "[" new "]: " repo "/compare/v" old "...v" new
    next
  }
  { print }
' CHANGELOG.md > CHANGELOG.md.tmp && mv CHANGELOG.md.tmp CHANGELOG.md

tools/changelog-section.sh "$new" >/dev/null
cargo package --quiet --allow-dirty >/dev/null

git add Cargo.toml Cargo.lock CHANGELOG.md
git commit --quiet -m "Release $new"

cat <<EOM
Committed "Release $new" on $branch. Next:

  git push -u origin $branch        # then open a PR and merge it
  git switch main && git pull --ff-only
  git tag v$new && git push origin v$new    # publishes $new

Notes for the GitHub release:

$(tools/changelog-section.sh "$new")
EOM
