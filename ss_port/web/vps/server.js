// The leaderboard API on a server of our own (the OVH VPS that also runs RNG∞'s database), in place of Vercel
// functions: the same api/*.js, unchanged, served by one Node process behind Caddy at
// https://db.rng-infinite.com/coinless/api/… (Caddy strips /coinless).
//   node ss_port/web/vps/server.js      (systemd service coinless-api: see install.sh)
'use strict';
const fs = require('node:fs');
const http = require('node:http');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const PORT = Number(process.env.PORT || 8790);
const HOST = process.env.HOST || '127.0.0.1';
// A saved run carries its key presses (up to 400,000 characters, see api/run.js).
const MAX_BODY = 1024 * 1024;
// The pages allowed to call this API from a browser.
const ORIGINS = (process.env.COINLESS_ORIGINS || 'https://sacha9214.github.io').split(',');

// The database: the VPS's Redis through its REST relay, with the relay's token read from its own file. Set before
// loading the functions: api/_lib.js reads these when it loads. Every key of this game starts with "coinless:".
if (!process.env.COINLESS_REDIS_URL) process.env.COINLESS_REDIS_URL = 'http://127.0.0.1:7379';
if (!process.env.COINLESS_REDIS_TOKEN && process.env.COINLESS_REDIS_TOKEN_FILE) {
  process.env.COINLESS_REDIS_TOKEN = fs.readFileSync(process.env.COINLESS_REDIS_TOKEN_FILE, 'utf8').trim();
}

const API = Object.fromEntries(['auth', 'config', 'leaderboard', 'name', 'run'].map(name => [name, require(path.join(__dirname, '../api', `${name}.js`))]));
let version = '';
try { version = execFileSync('git', ['rev-parse', '--short', 'HEAD'], { cwd: __dirname, encoding: 'utf8' }).trim(); } catch (e) { /* not a checkout */ }

function reply(res, status, body) {
  if (res.headersSent) return res.end();
  res.statusCode = status;
  res.setHeader('Content-Type', 'application/json');
  res.setHeader('Cache-Control', 'no-store');
  res.end(JSON.stringify(body));
}

// The body as Vercel hands it to the functions: an object parsed from JSON.
async function readBody(req) {
  const chunks = [];
  let size = 0;
  for await (const chunk of req) {
    size += chunk.length;
    if (size > MAX_BODY) throw Object.assign(new Error('Body too large'), { status: 413 });
    chunks.push(chunk);
  }
  const raw = Buffer.concat(chunks).toString('utf8');
  if (!raw.trim()) return {};
  try { return JSON.parse(raw); } catch (e) { throw Object.assign(new Error('Invalid JSON'), { status: 400 }); }
}

const server = http.createServer(async (req, res) => {
  const pathname = String(req.url || '/').split('?')[0];
  // The page is on another domain (GitHub Pages): it may read the answers, and send JSON (hence the preflight).
  const origin = req.headers.origin;
  if (origin && ORIGINS.includes(origin)) res.setHeader('Access-Control-Allow-Origin', origin);
  res.setHeader('Vary', 'Origin');
  try {
    if (req.method === 'OPTIONS') {
      res.setHeader('Access-Control-Allow-Methods', 'GET, POST');
      res.setHeader('Access-Control-Allow-Headers', 'Content-Type');
      res.setHeader('Access-Control-Max-Age', '86400');
      res.statusCode = 204;
      return res.end();
    }
    if (pathname === '/health' || pathname === '/api/health') return reply(res, 200, { ok: true, version, up: Math.round(process.uptime()) });
    const name = pathname.startsWith('/api/') ? pathname.slice(5) : '';
    const handler = Object.hasOwn(API, name) ? API[name] : null;
    if (!handler) return reply(res, 404, { error: 'Not found' });
    req.body = req.method === 'POST' ? await readBody(req) : {};
    await handler(req, res);
  } catch (err) {
    if (!err.status) console.error(`${req.method} ${pathname}: ${err.stack || err}`);
    reply(res, err.status || 500, { error: err.status ? err.message : 'Server error' });
  }
});
server.requestTimeout = 15000;
server.headersTimeout = 10000;
server.keepAliveTimeout = 65000;

// One lost error must not stop the leaderboard for everyone.
process.on('unhandledRejection', err => console.error('unhandledRejection:', (err && err.stack) || err));
process.on('uncaughtException', err => console.error('uncaughtException:', (err && err.stack) || err));

// Clean stop (an update): finish the requests in progress; systemd starts the new code right after.
for (const signal of ['SIGTERM', 'SIGINT']) {
  process.on(signal, () => {
    server.close(() => process.exit(0));
    server.closeIdleConnections();
    setTimeout(() => process.exit(0), 5000).unref();
  });
}

server.listen(PORT, HOST, () => console.log(`Coinless Surfers API ${version || '(dev)'}: http://${HOST}:${PORT}`));
