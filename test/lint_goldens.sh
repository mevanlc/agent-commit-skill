#!/usr/bin/env bash
# lint_goldens.sh - Structural checks over every expected.txt
#
# Golden-file tests compare against a blessed snapshot, so a defect that is
# already baked into the snapshot is invisible: the diff is empty and
# regen_expecteds.sh re-blesses it. These assertions check properties the
# snapshots themselves cannot, and are what catch a malformed block that has
# been "expected" for months.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FAILURES=0

fail() {
  printf 'LINT FAIL: %s\n' "$1"
  FAILURES=$((FAILURES + 1))
}

for golden in "${SCRIPT_DIR}"/*/expected.txt; do
  [[ -f "$golden" ]] || continue
  name="$(basename "$(dirname "$golden")")"

  # 1. Code fences must be balanced. An odd count means a block never closes,
  #    which swallows every following section into the code block.
  fences=$(grep -c '^```' "$golden" || true)
  if (( fences % 2 != 0 )); then
    fail "${name}: unbalanced code fences (${fences})"
  fi

  # 2. A heading must start its own line. `\`\`\`# Instructions` is the shape
  #    this catches: a closing fence with the next heading glued onto it. The
  #    char before the `#` run must be neither `#` (that is just `## Heading`)
  #    nor whitespace (indented `#` is not a heading we emit).
  glued_re='[^#[:space:]]#{1,6} [A-Z]'
  if grep -qE "$glued_re" "$golden"; then
    fail "${name}: heading glued to preceding content"
    grep -nE "$glued_re" "$golden" | sed 's/^/    /'
  fi

  # 3. Top-level instruction steps must be numbered 1..N with no gaps, so a
  #    renumbering mistake cannot ship.
  # (a plain loop rather than mapfile -- /bin/bash on macOS is still 3.2)
  expected=1
  while IFS= read -r n; do
    [[ -z "$n" ]] && continue
    if (( n != expected )); then
      fail "${name}: instruction steps out of sequence (saw ${n}, expected ${expected})"
      break
    fi
    expected=$((expected + 1))
  done < <(sed -n '/^# Instructions/,/^# Final Report/p' "$golden" |
    grep -oE '^[0-9]+\.' | tr -d '.')

  # 4. No duplicated top-level headings within one document.
  dupes=$(grep '^# ' "$golden" | sort | uniq -d)
  if [[ -n "$dupes" ]]; then
    fail "${name}: duplicate headings: $(echo $dupes)"
  fi

  # 5. Counts interpolated into the output must not carry `wc -l` padding.
  if grep -qE '\([[:space:]]+[0-9]+ (lines|characters|bytes)\)' "$golden"; then
    fail "${name}: padded numeric count in output"
  fi
done

if (( FAILURES > 0 )); then
  printf '\nGolden lint: %d failure(s)\n' "$FAILURES"
  exit 1
fi

echo "Golden lint: all checks passed"
