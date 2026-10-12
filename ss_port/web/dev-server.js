// Local trial of the site with its /api functions: node dev-server.js DIR [port]
// (COINLESS_DEV=1: a database in memory and sign-ins without Google.)
process.env.COINLESS_DEV = '1';
const http = require('node:http');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(process.argv[2] || '.');
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.json': 'application/json', '.webp': 'image/webp', '.png': 'image/png', '.ogg': 'audio/ogg' };

http.createServer((req, res) => {
  const url = new URL(req.url, 'http://x');
  if (url.pathname.startsWith('/api/')) {
    let data = '';
    req.on('data', c => (data += c));
    req.on('end', () => {
      req.body = data;
      require(path.join(root, url.pathname + '.js'))(req, res);
    });
    return;
  }
  const file = path.join(root, url.pathname === '/' ? 'index.html' : url.pathname);
  if (!file.startsWith(root) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
    res.statusCode = 404;
    return res.end('not found');
  }
  res.setHeader('Content-Type', types[path.extname(file)] || 'application/octet-stream');
  fs.createReadStream(file).pipe(res);
}).listen(Number(process.argv[3] || 8766), () => console.log('http://localhost:' + (process.argv[3] || 8766)));
