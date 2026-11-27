#!/usr/bin/env bash
# One-time setup of the key that signs desktop updates, and hands it to GitHub so
# release builds are signed there. The tools ask for the password themselves;
# this script never sees or stores it.
#
# Every installed copy accepts updates signed with this key only. Keep a copy of
# the key file and its password somewhere off this computer: if either is lost,
# installed apps can never update again.
set -euo pipefail
cd "$(dirname "$0")/.."

repo=sudsarkar13/tourledger
key="${TOURLEDGER_UPDATE_KEY:-$HOME/.tourledger/updater.key}"
tauri=ui/node_modules/.bin/tauri

[ "$(gh api user --jq .login)" = "${repo%%/*}" ] || { echo "Sign gh in as ${repo%%/*} first: gh auth switch" >&2; exit 1; }

mkdir -p "$(dirname "$key")"
chmod 700 "$(dirname "$key")"
if [ -f "$key" ]; then
  echo "Using the existing key at $key"
else
  echo "Choose a password for the update key. It is a new one: do not reuse another."
  "$tauri" signer generate --write-keys "$key"
  chmod 600 "$key"
fi

gh secret set TAURI_SIGNING_PRIVATE_KEY --repo "$repo" < "$key"
echo
echo "Now give GitHub the same password:"
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo "$repo"

# The public half goes into the app. It is not a secret.
pub=$(cat "$key.pub")
node -e '
  const fs = require("fs");
  const file = "src-tauri/tauri.conf.json";
  const text = fs.readFileSync(file, "utf8");
  if (!/"pubkey": "[^"]*"/.test(text)) throw new Error("No pubkey field in " + file);
  fs.writeFileSync(file, text.replace(/"pubkey": "[^"]*"/, `"pubkey": "${process.argv[1]}"`));
' "$pub"

echo
echo "Done. Back up these two things somewhere off this computer:"
echo "  $key"
echo "  its password"
