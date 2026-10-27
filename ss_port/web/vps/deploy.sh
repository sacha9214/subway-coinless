#!/usr/bin/env bash
# Updates the API on the VPS after a push to main (no password: the service runs as our user).
#   ssh emma@<vps> 'bash ~/subway-coinless/ss_port/web/vps/deploy.sh'
set -euo pipefail
cd "$(dirname "$0")/../../.."

before="$(git rev-parse --short HEAD)"
git fetch -q origin main
git merge -q --ff-only origin/main
after="$(git rev-parse --short HEAD)"

# The new code must load before the old one stops: otherwise back to where we were, and stop there.
if ! COINLESS_DEV=1 node -e '
  for (const n of ["auth", "config", "leaderboard", "name", "run"]) require("./ss_port/web/api/" + n + ".js");
' || ! node --check ss_port/web/vps/server.js; then
  git reset -q --hard "$before"
  echo "The new code does not load: back to $before, service unchanged." >&2
  exit 1
fi

# SIGTERM: the server finishes its requests, then systemd starts it again (1 s) with the new code.
pkill -TERM -u "$(id -u)" -f 'ss_port/web/vps/server.js' || true
for i in 1 2 3 4 5 6 7 8 9 10; do
  sleep 1
  if out="$(curl -fs http://127.0.0.1:8790/health)"; then echo "$before -> $after  $out"; exit 0; fi
done
echo "The API does not answer after the update (journalctl -u coinless-api -n 50)." >&2
exit 1
