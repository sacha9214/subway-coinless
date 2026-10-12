// POST /api/name { token, name }: the name shown on the leaderboard.
const { K, redis, cleanName, player, send, body, endpoint } = require('./_lib');

module.exports = endpoint('POST', async (req, res) => {
  const b = body(req);
  const id = await player(b.token);
  if (!id) return send(res, 401, { error: 'Sign in again' });
  const name = cleanName(b.name);
  if (name.length < 2) return send(res, 400, { error: 'A name has 2 to 20 letters or digits' });
  await redis([['HSET', K('names'), id, name]]);
  send(res, 200, { name });
});
