#!/usr/bin/env bash
# Print one version's section of CHANGELOG.md, without its heading: the
# release notes for that version. Fails if the section is missing or empty.
#
#   tools/changelog-section.sh 0.1.0
set -euo pipefail
version=${1:?usage: $0 <version>}
file=${CHANGELOG:-$(dirname "$0")/../CHANGELOG.md}
notes=$(awk -v v="$version" '
  /^## \[/ { if (found) exit; if (index($0, "## [" v "]") == 1) { found = 1; next } }
  /^\[[^]]+\]: / { if (found) exit }
  found { print }
' "$file" | sed -e '/./,$!d' | sed -e ':a' -e '/^\n*$/{$d;N;ba' -e '}')
if [ -z "$notes" ]; then
  echo "no CHANGELOG section for $version" >&2
  exit 1
fi
printf '%s\n' "$notes"
