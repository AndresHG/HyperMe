// Serves the built pages (app/web/dist, spec 257) for the checks, with mock-tauri.js put into every
// page in place of Tauri's API.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';

const ui = path.resolve(import.meta.dirname, '../web/dist');
const mock = fs.readFileSync(path.join(import.meta.dirname, 'mock-tauri.js'), 'utf8');
const types = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript' };

http.createServer((req, res) => {
  const name = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  const file = path.join(ui, name);
  if (!file.startsWith(ui)) { res.writeHead(403); res.end(); return; }
  fs.readFile(file, (err, data) => {
    if (err) { res.writeHead(404); res.end(); return; }
    let body = data;
    if (file.endsWith('.html')) {
      body = data.toString().replace('<head>', `<head><script>${mock}</script>`);
    }
    res.writeHead(200, { 'content-type': types[path.extname(file)] || 'application/octet-stream' });
    res.end(body);
  });
}).listen(Number(process.env.PORT || 4173));
