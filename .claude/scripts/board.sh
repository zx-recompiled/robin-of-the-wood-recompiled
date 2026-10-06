#!/usr/bin/env bash
# board.sh — the ticket state baton, on the GitHub Project's Status field.
#
# State lives in the Status field of the "Robin of the Wood Recompiled" org Project,
# so the maintainer can drag a card (web or mobile) and Claude can set the same
# value from the CLI — one source of truth either way. `ready to merge` is a PR
# LABEL and is not managed here.
#
# The opaque node ids below are why this file exists: they belong in one place
# rather than copy-pasted into every skill. Ported from starquake/numbergame.
#
#   ./board.sh state <issue> "<Status>"   set an issue's state
#   ./board.sh get   <issue>              print an issue's state
#   ./board.sh list  "<Status>"           list issue numbers in that state
#   ./board.sh states                     print the valid state names
#
# Requires the `project` scope on the gh token.
set -euo pipefail

# The board belongs to the org that owns the repo. While it was a user project
# on an org repo, issues stopped listing it under `projectItems` (starquake-recompiled#137).
PROJECT_NUMBER=2
PROJECT_OWNER=zx-recompiled
PROJECT_REPO=robin-of-the-wood-recompiled
PROJECT_ID="PVT_kwDOE-P1Gc4Bl5yn"
STATUS_FIELD_ID="PVTSSF_lADOE-P1Gc4Bl5ynzhkkztM"

# Where `state` records its own writes for the board monitor to ignore
# (work-the-board). Transient by design — losing it costs one spurious
# notification, never a missed maintainer move.
SELF_SET_FILE="${BOARD_SELF_SET_FILE:-${TMPDIR:-/tmp}/robin-board-selfset}"

# Status name -> single-select option id, looked up LIVE by name.
#
# Reordering a single-select's options REPLACES every option, minting new ids
# and clearing every item's value. Names are the stable handle; ids are not,
# so they are never hardcoded here.
option_id() {
  local id
  id=$(gh api graphql -f query="{ organization(login:\"$PROJECT_OWNER\"){ projectV2(number: $PROJECT_NUMBER){
        field(name:\"Status\"){ ... on ProjectV2SingleSelectField { options{ id name } } } } } }" \
      --jq ".data.organization.projectV2.field.options[] | select(.name==\"$1\") | .id" 2>/dev/null)
  if [ -z "$id" ]; then
    echo "unknown state: $1" >&2
    echo "valid: $(gh api graphql -f query="{ organization(login:\"$PROJECT_OWNER\"){ projectV2(number: $PROJECT_NUMBER){
          field(name:\"Status\"){ ... on ProjectV2SingleSelectField { options{ name } } } } } }" \
        --jq '[.data.organization.projectV2.field.options[].name] | join(", ")')" >&2
    return 1
  fi
  printf '%s' "$id"
}

# Item id for an issue number, adding the issue to the project if it is missing
# (a hand-filed issue may never have been added).
#
# Asks the ISSUE for its project items rather than listing the whole board:
# `gh project item-list` is by far the most expensive GraphQL query here.
item_id() {
  local issue="$1" id
  id=$(gh api graphql -f query="{ repository(owner:\"$PROJECT_OWNER\", name:\"$PROJECT_REPO\"){
        issue(number: $issue){ projectItems(first:10){ nodes{ id project{ number } } } } } }" \
      --jq ".data.repository.issue.projectItems.nodes[] | select(.project.number==$PROJECT_NUMBER) | .id" 2>/dev/null | head -1)
  if [ -z "$id" ]; then
    id=$(gh project item-add "$PROJECT_NUMBER" --owner "$PROJECT_OWNER" \
          --url "https://github.com/$PROJECT_OWNER/$PROJECT_REPO/issues/$issue" \
          --format json --jq .id)
  fi
  printf '%s' "$id"
}

case "${1:-}" in
  state)
    issue="$2"; want="$3"
    opt=$(option_id "$want")
    item=$(item_id "$issue")
    gh api graphql -f query="mutation{updateProjectV2ItemFieldValue(input:{
        projectId:\"$PROJECT_ID\", itemId:\"$item\", fieldId:\"$STATUS_FIELD_ID\",
        value:{singleSelectOptionId:\"$opt\"}}){projectV2Item{id}}}" >/dev/null
    # Record that this move was OURS, so the board monitor does not wake the
    # agent to report a change the agent just made. The monitor consumes (and
    # removes) the matching entry when it sees the transition.
    printf '%s|%s\n' "$issue" "$want" >> "$SELF_SET_FILE"
    echo "#$issue -> $want"
    ;;
  get)
    gh api graphql -f query="{ repository(owner:\"$PROJECT_OWNER\", name:\"$PROJECT_REPO\"){
        issue(number: $2){ projectItems(first:10){ nodes{ project{ number }
          fieldValueByName(name:\"Status\"){ ... on ProjectV2ItemFieldSingleSelectValue { name } } } } } } }" \
      --jq ".data.repository.issue.projectItems.nodes[] | select(.project.number==$PROJECT_NUMBER) | .fieldValueByName.name // \"(unset)\""
    ;;
  list)
    # The targeted query (1 GraphQL point a page) rather than `gh project
    # item-list` (~100 points a call), so a loop can afford it. Paged, so the
    # board growing past 100 items does not silently drop cards.
    gh api graphql --paginate -f query="query(\$endCursor: String){ organization(login:\"$PROJECT_OWNER\"){
        projectV2(number: $PROJECT_NUMBER){ items(first:100, after: \$endCursor){ pageInfo{ hasNextPage endCursor } nodes{
        content{ ... on Issue { number } }
        fieldValueByName(name:\"Status\"){ ... on ProjectV2ItemFieldSingleSelectValue { name } } } } } } }" \
      --jq ".data.organization.projectV2.items.nodes[] | select(.fieldValueByName.name==\"$2\") | .content.number // empty" | sort -n
    ;;
  states)
    gh api graphql -f query="{ organization(login:\"$PROJECT_OWNER\"){ projectV2(number: $PROJECT_NUMBER){
        field(name:\"Status\"){ ... on ProjectV2SingleSelectField { options{ name } } } } } }" \
      --jq '.data.organization.projectV2.field.options[].name'
    ;;
  *)
    sed -n '2,18p' "$0" | sed 's/^# \{0,1\}//'
    exit 1
    ;;
esac
