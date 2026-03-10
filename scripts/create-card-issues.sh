#!/usr/bin/env bash
# Creates a GitHub issue for every card in the Vintage Supreme Draft format.
# Requires: gh CLI authenticated (gh auth login)
# Usage: ./scripts/create-card-issues.sh
#
# Options:
#   --dry-run     Print issue titles without creating them
#   --batch N     Create N issues then pause (default: all)
#   --start N     Start from card number N (1-indexed, for resuming)

set -euo pipefail

REPO="jacksonrnewhouse/mage"
DRY_RUN=false
BATCH_SIZE=0
START=1

while [[ $# -gt 0 ]]; do
  case $1 in
    --dry-run) DRY_RUN=true; shift ;;
    --batch) BATCH_SIZE=$2; shift 2 ;;
    --start) START=$2; shift 2 ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

CARDS_DIR="$(cd "$(dirname "$0")/../cards" && pwd)"

create_issue() {
  local card_name="$1"
  local color="$2"
  local label="$3"

  local title="[Rust Engine] Implement card: ${card_name}"

  local body
  body=$(cat <<BODY
## Card Implementation: ${card_name}

**Color/Category:** ${color}
**Format:** Vintage Supreme Draft
**Engine:** Rust (\`engine-rust/\`)

### Description

Implement the card **${card_name}** in the Rust game engine so that it can be used in Vintage Supreme Draft games.

### Acceptance Criteria

- [ ] Card is defined in the \`CardName\` enum in \`engine-rust/src/card.rs\`
- [ ] Card has correct static properties in \`build_card_db()\` (name, mana cost, types, power/toughness, keywords)
- [ ] All card abilities are implemented and functional:
  - [ ] Static abilities (if any)
  - [ ] Triggered abilities (if any)
  - [ ] Activated abilities (if any)
  - [ ] Replacement effects (if any)
  - [ ] Special rules text (if any)
- [ ] Card can be cast/played correctly (mana cost paid, targeting works)
- [ ] Card interacts correctly with other cards in the format
- [ ] Card behavior matches official Magic: The Gathering rules
- [ ] Move generation correctly enumerates legal actions involving this card
- [ ] Unit test(s) covering the card's core functionality
BODY
)

  if $DRY_RUN; then
    echo "[DRY RUN] Would create: ${title} (${color})"
    return
  fi

  echo "Creating issue: ${title}"
  gh issue create \
    --repo "${REPO}" \
    --title "${title}" \
    --body "${body}" \
    --label "card-implementation,${label}" \
    2>&1 || echo "  WARNING: Failed to create issue for ${card_name}"

  # Small delay to avoid rate limiting
  sleep 0.5
}

# Ensure labels exist
if ! $DRY_RUN; then
  echo "Ensuring labels exist..."
  for label in card-implementation white blue black red green colorless land multicolor; do
    gh label create "${label}" --repo "${REPO}" 2>/dev/null || true
  done
fi

# Read all card files and create issues
card_num=0
created=0

for file in "${CARDS_DIR}"/*.txt; do
  color=$(basename "${file}" .txt)

  # Map color to label
  case "${color}" in
    azorius|dimir|rakdos|gruul|selesnya|orzhov|izzet|golgari|boros|simic|multicolor)
      label="multicolor" ;;
    *) label="${color}" ;;
  esac

  while IFS= read -r card_name; do
    [[ -z "${card_name}" ]] && continue
    card_num=$((card_num + 1))

    # Skip until start
    [[ ${card_num} -lt ${START} ]] && continue

    create_issue "${card_name}" "${color}" "${label}"
    created=$((created + 1))

    # Check batch limit
    if [[ ${BATCH_SIZE} -gt 0 && ${created} -ge ${BATCH_SIZE} ]]; then
      echo "Batch limit reached (${BATCH_SIZE}). Resume with --start $((card_num + 1))"
      exit 0
    fi
  done < "${file}"
done

echo "Done! Created ${created} issues."
