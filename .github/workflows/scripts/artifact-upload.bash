#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<EOF
Usage: $0 <owner> <repo> <branch> <target-path> <artifact-file> <commit-message> <github-token>

Arguments:
  owner           : GitHub user or organization of the target repo
  repo            : Target repo name
  branch          : Branch name (e.g. main)
  target-path     : Full path (including folders + filename) in the target repo where to upload
  artifact-file   : Local file path to upload
  commit-message  : Commit message for creating/updating the file
  github-token    : GitHub token (PAT or equivalent) with write access to the target repo

This will perform a GitHub API PUT to /repos/{owner}/{repo}/contents/{target-path}.
If a file already exists at that path, it will auto-detect the SHA and update; otherwise it will create.
EOF
  exit 1
}

if [ $# -ne 7 ]; then
  usage
fi

OWNER="$1"
REPO="$2"
BRANCH="$3"
TARGET_PATH="$4"
ARTIFACT_PATH="$5"
COMMIT_MSG="$6"
TOKEN="$7"

if [ ! -f "$ARTIFACT_PATH" ]; then
  echo "Error: artifact file not found: $ARTIFACT_PATH" >&2
  exit 1
fi

LOCAL_SHA=$(git hash-object "$ARTIFACT_PATH")
echo "Local blob SHA: $LOCAL_SHA"

GET_URL="https://api.github.com/repos/${OWNER}/${REPO}/contents/${TARGET_PATH}?ref=${BRANCH}"
GET_RESPONSE=$(curl -s -H "Authorization: token ${TOKEN}" "$GET_URL")

REMOTE_SHA=$(echo "$GET_RESPONSE" | jq -r .sha 2>/dev/null || echo "")

if [ "$REMOTE_SHA" != "null" ] && [ -n "$REMOTE_SHA" ]; then
  echo "Remote blob SHA: $REMOTE_SHA"
  if [ "$LOCAL_SHA" = "$REMOTE_SHA" ]; then
    echo "The remote file is identical. Skipping upload."
    exit 0
  else
    echo "Remote file differs. Preparing to upload."
  fi
else
  echo "No existing remote file or no SHA found. Creating."
fi

CONTENT_TMP_BASE64=$(mktemp)
if base64 --help 2>&1 | grep -q -- "-w"; then
  base64 -w 0 "$ARTIFACT_PATH" > "$CONTENT_TMP_BASE64"
else
  base64 "$ARTIFACT_PATH" | tr -d '\n' > "$CONTENT_TMP_BASE64"
fi

PAYLOAD_TMP=$(mktemp)
jq -n \
  --arg message "$COMMIT_MSG" \
  --arg branch "$BRANCH" \
  --arg sha "$REMOTE_SHA" \
  --rawfile content "$CONTENT_TMP_BASE64" \
  '{
     message: $message,
     content: $content,
     branch: $branch
   } + (if ($sha != "" and $sha != "null") then { sha: $sha } else {} end)' \
  > "$PAYLOAD_TMP"

UPLOAD_RESPONSE=$(curl -s -X PUT \
  -H "Authorization: token ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d @"$PAYLOAD_TMP" \
  "https://api.github.com/repos/${OWNER}/${REPO}/contents/${TARGET_PATH}")

echo "Upload Response:"
echo "$UPLOAD_RESPONSE"

rm -f "$CONTENT_TMP_BASE64" "$PAYLOAD_TMP"
