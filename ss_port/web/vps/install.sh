#!/usr/bin/env bash
# Installs the Coinless Surfers API on the VPS, next to RNG∞'s database: the systemd service coinless-api and the
# route https://db.rng-infinite.com/coinless/* in Caddy. Run it on the server (asks for the sudo password once):
#   bash ~/subway-coinless/ss_port/web/vps/install.sh
# Idempotent. No secret to type: the service reads the database relay's token from its file, and the Google client
# ID from ~/.config/coinless-api.env (GOOGLE_CLIENT_ID=…; until it is there, the page shows no sign-in button).
# After changing that file: bash deploy.sh (no password).
set -euo pipefail

DOMAIN="${DOMAIN:-db.rng-infinite.com}"
PORT=8790
USER_NAME="$(id -un)"
REPO="$(cd "$(dirname "$0")/../../.." && pwd)"
ENV_FILE="$HOME/.config/coinless-api.env"

echo "== 1/4 Checks"
command -v node >/dev/null || { echo "Node.js is missing" >&2; exit 1; }
grep -q '^GOOGLE_CLIENT_ID=.' "$ENV_FILE" 2>/dev/null || echo "   (no GOOGLE_CLIENT_ID in $ENV_FILE yet: no sign-in until it is set)"
systemctl is-active --quiet rng-relay || { echo "The database relay (rng-relay) is not running" >&2; exit 1; }
sudo test -r /etc/rng-relay/token || { echo "/etc/rng-relay/token is missing" >&2; exit 1; }
echo "   node $(node -v)"

echo "== 2/4 Service coinless-api (127.0.0.1:$PORT)"
sudo tee /etc/systemd/system/coinless-api.service >/dev/null <<UNIT
[Unit]
Description=Coinless Surfers - leaderboard API (Node)
After=network.target rng-relay.service
Wants=rng-relay.service

[Service]
User=$USER_NAME
WorkingDirectory=$REPO
Environment=NODE_ENV=production
Environment=PORT=$PORT
Environment=COINLESS_REDIS_URL=http://127.0.0.1:7379
Environment=COINLESS_REDIS_TOKEN_FILE=/etc/rng-relay/token
EnvironmentFile=-$ENV_FILE
ExecStart=$(command -v node) $REPO/ss_port/web/vps/server.js
Restart=always
RestartSec=1
NoNewPrivileges=true
MemoryMax=300M

[Install]
WantedBy=multi-user.target
UNIT
sudo systemctl daemon-reload
sudo systemctl enable coinless-api >/dev/null 2>&1
sudo systemctl restart coinless-api

echo "== 3/4 Caddy: $DOMAIN/coinless/* to the API (previous file kept as /etc/caddy/Caddyfile.bak-coinless)"
if grep -q "reverse_proxy 127.0.0.1:$PORT" /etc/caddy/Caddyfile; then
  echo "   already there"
else
  new="$(mktemp)"
  # The route goes into the site block of $DOMAIN, whatever else that file holds.
  python3 -I -c '
import re, sys
domain, port = sys.argv[1], sys.argv[2]
text = sys.stdin.read()
site = re.compile(r"^" + re.escape(domain) + r"(,[^{]*)?\s*\{[ \t]*$", re.M).search(text)
if not site:
    sys.exit("No site block for " + domain + " in the Caddyfile")
route = "\n\thandle /coinless/* {\n\t\turi strip_prefix /coinless\n\t\treverse_proxy 127.0.0.1:" + port + "\n\t}"
sys.stdout.write(text[:site.end()] + route + text[site.end():])
' "$DOMAIN" "$PORT" </etc/caddy/Caddyfile >"$new"
  if sudo caddy validate --config "$new" --adapter caddyfile >/dev/null 2>&1; then
    sudo cp /etc/caddy/Caddyfile /etc/caddy/Caddyfile.bak-coinless
    sudo cp "$new" /etc/caddy/Caddyfile
    sudo systemctl reload caddy
  else
    rm -f "$new"
    echo "   Caddy refused the new configuration: nothing was changed in Caddy." >&2
    exit 1
  fi
  rm -f "$new"
fi

echo "== 4/4 Checks"
sleep 2
systemctl is-active coinless-api
curl -fsS "http://127.0.0.1:$PORT/health" && echo
curl -fsS "https://$DOMAIN/coinless/health" && echo
curl -fsS "https://$DOMAIN/coinless/api/leaderboard" && echo
echo
echo "Done. The API answers at https://$DOMAIN/coinless/api/…"
