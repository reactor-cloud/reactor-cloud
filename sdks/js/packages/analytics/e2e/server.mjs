import { createServer } from 'http';
import { readFile } from 'fs/promises';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const PORT = 3001;

const mimeTypes = {
  '.html': 'text/html',
  '.js': 'application/javascript',
  '.mjs': 'application/javascript',
  '.json': 'application/json',
  '.css': 'text/css',
};

const server = createServer(async (req, res) => {
  const url = new URL(req.url, `http://localhost:${PORT}`);
  
  // API mock endpoints
  if (url.pathname.startsWith('/api/analytics')) {
    res.writeHead(202, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ success: true }));
    return;
  }

  // Serve files
  let filePath;
  if (url.pathname === '/' || url.pathname === '/index.html') {
    filePath = join(__dirname, 'fixtures', 'index.html');
  } else if (url.pathname.startsWith('/dist/')) {
    filePath = join(__dirname, '..', url.pathname);
  } else {
    res.writeHead(404);
    res.end('Not Found');
    return;
  }

  try {
    const content = await readFile(filePath);
    const ext = filePath.slice(filePath.lastIndexOf('.'));
    const contentType = mimeTypes[ext] || 'application/octet-stream';
    res.writeHead(200, { 'Content-Type': contentType });
    res.end(content);
  } catch (err) {
    res.writeHead(404);
    res.end(`File not found: ${url.pathname}`);
  }
});

server.listen(PORT, () => {
  console.log(`E2E test server running at http://localhost:${PORT}`);
});
