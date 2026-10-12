// POST /api/auth { credential }: a Google sign-in becomes a session of this site (a token kept by the browser), so
// that a run can be saved long after the Google token expired.
const crypto = require('node:crypto');
const { K, redis, verifyGoogleToken, cleanName, sha256, send, body, endpoint } = require('./_lib');

module.exports = endpoint('POST', async (req, res) => {
  let google;
  try {
    google = await verifyGoogleToken(body(req).credential || '');
  } catch (err) {
    return send(res, 401, { error: 'Invalid Google sign-in' });
  }
  const id = google.sub;
  const token = crypto.randomBytes(24).toString('hex');
  const [[saved]] = await redis([['HMGET', K('names'), id]]);
  // the first name only: the leaderboard is public
  const name = saved || cleanName(google.given_name || google.name) || 'Player';
  await redis([
    ['SET', K(`session:${sha256(token)}`), id, 'EX', 90 * 86400],
    ['HSET', K('names'), id, name],
  ]);
  send(res, 200, { token, name });
});
