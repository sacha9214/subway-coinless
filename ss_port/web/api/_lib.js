// Shared by the /api functions (the _ prefix keeps Vercel from making it an endpoint).
// Accounts are Google sign-ins; the database is Redis over REST (the Upstash integration of Vercel: KV_* or UPSTASH_*).
const crypto = require('node:crypto');

const REDIS_URL = process.env.COINLESS_REDIS_URL || process.env.KV_REST_API_URL || process.env.UPSTASH_REDIS_REST_URL;
const REDIS_TOKEN = process.env.COINLESS_REDIS_TOKEN || process.env.KV_REST_API_TOKEN || process.env.UPSTASH_REDIS_REST_TOKEN;
const GOOGLE_CLIENT_ID = process.env.GOOGLE_CLIENT_ID || '';
// Local trials only (web/dev-server.js): a database in memory and sign-ins of the form "dev:Name".
const DEV = process.env.COINLESS_DEV === '1';

// Every key of this game starts with this (the database may be shared with other sites).
const K = name => `coinless:${name}`;

const memory = { strings: new Map(), hashes: new Map(), zsets: new Map() };
function memoryCommand([cmd, key, ...a]) {
  const { strings, hashes, zsets } = memory;
  switch (cmd) {
    case 'GET': return strings.has(key) ? strings.get(key) : null;
    case 'SET': strings.set(key, a[0]); return 'OK';
    case 'DEL': return Number(strings.delete(key));
    case 'HSET': { const h = hashes.get(key) || new Map(); for (let i = 0; i < a.length; i += 2) h.set(a[i], a[i + 1]); hashes.set(key, h); return a.length / 2; }
    case 'HMGET': { const h = hashes.get(key) || new Map(); return a.map(f => (h.has(f) ? h.get(f) : null)); }
    case 'ZADD': { const z = zsets.get(key) || new Map(); z.set(a[a.length - 1], Number(a[a.length - 2])); zsets.set(key, z); return 1; }
    case 'ZSCORE': { const z = zsets.get(key); return z && z.has(a[0]) ? String(z.get(a[0])) : null; }
    case 'ZREVRANK': { const z = [...(zsets.get(key) || [])].sort((x, y) => y[1] - x[1]); const i = z.findIndex(e => e[0] === a[0]); return i < 0 ? null : i; }
    case 'ZREVRANGE': { const z = [...(zsets.get(key) || [])].sort((x, y) => y[1] - x[1]).slice(Number(a[0]), Number(a[1]) + 1); return z.flatMap(e => [e[0], String(e[1])]); }
    case 'ZCARD': return (zsets.get(key) || new Map()).size;
    default: throw new Error(`dev store: ${cmd}`);
  }
}

// Runs a list of Redis commands in one round trip.
async function redis(commands) {
  const list = commands.map(c => c.map(String));
  if (DEV) return list.map(memoryCommand);
  if (!REDIS_URL || !REDIS_TOKEN) throw Object.assign(new Error('The leaderboard database is not configured'), { status: 503 });
  const res = await fetch(`${REDIS_URL}/pipeline`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${REDIS_TOKEN}`, 'Content-Type': 'application/json' },
    body: JSON.stringify(list),
  });
  if (!res.ok) throw new Error(`Redis HTTP ${res.status}`);
  return (await res.json()).map(r => {
    if (r.error) throw new Error(r.error);
    return r.result;
  });
}

// Checks a Google ID token (JWT RS256) against Google's public keys, without a dependency.
let googleKeys = { keys: [], fetchedAt: 0 };
async function googleKey(kid) {
  let key = googleKeys.keys.find(k => k.kid === kid);
  if (!key || Date.now() - googleKeys.fetchedAt > 3600 * 1000) {
    const res = await fetch('https://www.googleapis.com/oauth2/v3/certs');
    if (!res.ok) throw new Error(`Google certs HTTP ${res.status}`);
    googleKeys = { keys: (await res.json()).keys || [], fetchedAt: Date.now() };
    key = googleKeys.keys.find(k => k.kid === kid);
  }
  return key;
}
async function verifyGoogleToken(token) {
  if (DEV && String(token).startsWith('dev:')) return { sub: `dev-${token.slice(4)}`, given_name: token.slice(4) };
  const parts = String(token).split('.');
  if (parts.length !== 3) throw new Error('Malformed token');
  const [head, body, sig] = parts;
  const header = JSON.parse(Buffer.from(head, 'base64url').toString('utf8'));
  const claims = JSON.parse(Buffer.from(body, 'base64url').toString('utf8'));
  if (header.alg !== 'RS256') throw new Error('Unexpected algorithm');
  const jwk = await googleKey(header.kid);
  if (!jwk) throw new Error('Unknown signing key');
  const publicKey = crypto.createPublicKey({ key: jwk, format: 'jwk' });
  if (!crypto.verify('RSA-SHA256', Buffer.from(`${head}.${body}`), publicKey, Buffer.from(sig, 'base64url'))) throw new Error('Bad signature');
  if (claims.aud !== GOOGLE_CLIENT_ID) throw new Error('Token is for another app');
  if (claims.iss !== 'https://accounts.google.com' && claims.iss !== 'accounts.google.com') throw new Error('Unexpected issuer');
  if (!(claims.exp * 1000 > Date.now())) throw new Error('Expired token');
  if (!claims.sub) throw new Error('No subject');
  return claims;
}

// Names: Latin letters, digits, spaces and _ . - ' only, 20 characters.
const cleanName = raw => String(raw || '').normalize('NFC').replace(/[^\p{Script=Latin}0-9 _.\-']/gu, '').replace(/\s+/g, ' ').trim().slice(0, 20);
const sha256 = s => crypto.createHash('sha256').update(s).digest('hex');

// The player behind a session token (given by /api/auth), or null.
async function player(token) {
  if (!/^[0-9a-f]{48}$/.test(token || '')) return null;
  const [id] = await redis([['GET', K(`session:${sha256(token)}`)]]);
  return id;
}

function send(res, status, body) {
  res.statusCode = status;
  res.setHeader('Content-Type', 'application/json');
  res.setHeader('Cache-Control', 'no-store');
  res.end(JSON.stringify(body));
}
const body = req => (typeof req.body === 'string' ? JSON.parse(req.body || '{}') : req.body || {});

// Wraps a handler: one method, JSON errors.
const endpoint = (method, handler) => async (req, res) => {
  if (req.method !== method) return send(res, 405, { error: `Use ${method}` });
  try {
    await handler(req, res);
  } catch (err) {
    send(res, err.status || 500, { error: err.status ? err.message : 'Something went wrong' });
    if (!err.status) console.error(err);
  }
};

module.exports = { K, GOOGLE_CLIENT_ID, DEV, redis, verifyGoogleToken, cleanName, sha256, player, send, body, endpoint };
