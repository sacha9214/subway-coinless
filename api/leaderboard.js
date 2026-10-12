// GET /api/leaderboard: the 50 longest runs, one per player.
const { K, redis, send, endpoint } = require('./_lib');

module.exports = endpoint('GET', async (req, res) => {
  const [flat] = await redis([['ZREVRANGE', K('lb'), 0, 49, 'WITHSCORES']]);
  const ids = flat.filter((_, i) => i % 2 === 0);
  if (!ids.length) return send(res, 200, { runs: [] });
  const [names, bests] = await redis([['HMGET', K('names'), ...ids], ['HMGET', K('best'), ...ids]]);
  const runs = ids.map((id, i) => {
    const best = JSON.parse(bests[i] || '{}');
    return { name: names[i] || 'Player', time: Number(flat[2 * i + 1]), score: best.score || 0, at: best.at || 0 };
  });
  res.setHeader('Cache-Control', 'no-store');
  send(res, 200, { runs });
});
