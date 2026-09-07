// Offline handler replay only: no Next.js build, routing cache or UI claims.
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import { createRequire, registerHooks } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { control } from './reference.mjs';

const fixture = path.resolve(process.argv[2]);
const correct = process.argv[3] === 'control';
const ts = createRequire(import.meta.url)(process.argv[4]);
const adapter = JSON.parse(fs.readFileSync(path.join(fixture, 'adapter.json')));
const stub = pathToFileURL(path.join(path.dirname(fileURLToPath(import.meta.url)), 'response.mjs')).href;
registerHooks({ resolve(specifier, context, next) {
  if (specifier === 'next/server') return { url: stub, shortCircuit: true };
  if (specifier.startsWith('@/')) {
    return { url: pathToFileURL(path.join(fixture, 'src', specifier.slice(2) + '.ts')).href, shortCircuit: true };
  }
  if (specifier.startsWith('.') && context.parentURL?.endsWith('.ts') && !path.extname(specifier)) {
    return next(specifier + '.ts', context);
  }
  return next(specifier, context);
}, load(url, context, next) {
  if (url.startsWith('file:') && url.endsWith('.ts')) {
    // Use the real TS compiler's type/import erasure, including pre-TS-5
    // type-only imports written without `import type` in the frozen sources.
    const source = ts.transpileModule(fs.readFileSync(fileURLToPath(url), 'utf8'), {
      compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 }
    }).outputText;
    return { format: 'module', source, shortCircuit: true };
  }
  return next(url, context);
} });

// A bounded scheduler seam makes the unsafe async RMW race repeatable. The
// original read still happens; only its completion is delayed. A serial writer
// times out the first barrier, then reads the newly committed version normally.
let barrier = null;
const originalRead = fs.promises.readFile.bind(fs.promises);
fs.promises.readFile = async function (file, ...args) {
  const value = await originalRead(file, ...args);
  if (barrier && path.resolve(String(file)) === barrier.file) {
    const current = barrier;
    await new Promise(resolve => {
      current.waiters.push(resolve);
      if (current.waiters.length === 2) current.release();
    });
  }
  return value;
};

const server = http.createServer(async (req, res) => {
  try {
    const chunks = [];
    for await (const chunk of req) chunks.push(chunk);
    const body = Buffer.concat(chunks);
    const url = new URL(req.url, 'http://127.0.0.1');
    if (url.pathname === '/__replay/race') {
      const file = path.resolve('data', adapter.parent + '.json');
      barrier = { file, waiters: [], release() {
        if (barrier !== this) return;
        barrier = null;
        clearTimeout(this.timer);
        this.waiters.forEach(resolve => resolve());
      } };
      barrier.timer = setTimeout(() => barrier?.release(), 300);
      res.end('{}');
      return;
    }
    const request = new Request(url, { method: req.method, headers: req.headers,
      ...(body.length ? { body } : {}) });
    request.nextUrl = url;
    let response;
    if (correct) response = await control(request, adapter);
    else {
      const segments = url.pathname.split('/').filter(Boolean);
      if (segments[0] !== 'api' || ![adapter.parent, adapter.child].includes(segments[1]) || segments.length > 3) {
        throw new Error('unknown replay route');
      }
      const route = path.join(fixture, 'src/app/api', segments[1], ...(segments[2] ? ['[id]'] : []), 'route.ts');
      const handlers = await import(pathToFileURL(route));
      response = await handlers[req.method](request, { params: { id: segments[2] } });
    }
    res.writeHead(response.status, Object.fromEntries(response.headers));
    res.end(Buffer.from(await response.arrayBuffer()));
  } catch (error) {
    // Uncaught handler exceptions are HTTP 500 in this adapter, like a failed
    // route invocation. Startup/import failures must be caught by baseline tests.
    res.writeHead(500, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ error: String(error) }));
  }
});
server.listen(0, '127.0.0.1', () => console.log(JSON.stringify({ port: server.address().port })));
process.on('SIGTERM', () => server.close(() => process.exit(0)));
