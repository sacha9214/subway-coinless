// POST /api/run { token, seed, frames, time, score, keys }: saves a finished run. The leaderboard keeps each
// player's longest; its seed and keys are kept with it, so that the run can be played again (and checked).
const { K, redis, player, send, body, endpoint } = require('./_lib');

module.exports = endpoint('POST', async (req, res) => {
  const b = body(req);
  const id = await player(b.token);
  if (!id) return send(res, 401, { error: 'Sign in again' });
  const [seed, frames, time, score] = [b.seed, b.frames, b.time, b.score].map(Number);
  const keys = String(b.keys || '');
  const sane =
    Number.isInteger(seed) && seed >= 0 && Number.isInteger(frames) && frames > 0 && frames < 60 * 86400 &&
    time > 0 && time <= frames / 60 + 1 && Number.isInteger(score) && score >= 0 &&
    keys.length < 400000 && /^(\d+[udlr](,\d+[udlr])*)?$/.test(keys);
  if (!sane) return send(res, 400, { error: 'Not a run of this game' });

  const [previous] = await redis([['ZSCORE', K('lb'), id]]);
  const improved = previous === null || time > Number(previous);
  if (improved) {
    await redis([
      ['ZADD', K('lb'), time, id],
      ['HSET', K('best'), id, JSON.stringify({ score, at: Date.now() })],
      ['SET', K(`run:${id}`), JSON.stringify({ seed, frames, time, score, keys })],
    ]);
  }
  const [rank, total] = await redis([['ZREVRANK', K('lb'), id], ['ZCARD', K('lb')]]);
  send(res, 200, { improved, best: improved ? time : Number(previous), rank: Number(rank) + 1, players: Number(total) });
});
