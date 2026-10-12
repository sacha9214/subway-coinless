// GET /api/config: what the page needs to show the sign-in button.
const { GOOGLE_CLIENT_ID, DEV, send, endpoint } = require('./_lib');

module.exports = endpoint('GET', async (req, res) => send(res, 200, { googleClientId: GOOGLE_CLIENT_ID, dev: DEV }));
