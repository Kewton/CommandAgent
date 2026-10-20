// Offline handler replay only: no Next.js build, routing cache or UI claims.
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import { AsyncLocalStorage } from 'node:async_hooks';
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

// Owned diagnostics: one JSON line per event on stderr (the caller's server.log).
// Identity and barrier facts only; the process environment is never recorded.
const log = (event, fields = {}) => process.stderr.write('[replay] ' + JSON.stringify({ event, ...fields }) + '\n');
const real = file => { try { return fs.realpathSync(file); } catch { return path.resolve(file); } };
log('start', { pid: process.pid, cwd: real('.'), fixture: real(fixture), compiler: real(process.argv[4]),
  replay: real(fileURLToPath(import.meta.url)), mode: correct ? 'control' : 'original' });

// A bounded scheduler seam makes the unsafe async RMW race repeatable. Reads
// of the parent file are tagged with (request, read ordinal). The barrier opens
// one level per read ordinal and a level only ends in a rendezvous when two
// DISTINCT requests reach that same ordinal, so a request's seed read cannot
// stand in for the read that feeds its write. Only completion of the original
// read is delayed. Once either request writes the file the race is measured
// and the barrier disarms. A serial writer cannot rendezvous: its level times
// out (clock starts only once both requests arrived) and is reported as such.
// The outcome stays queryable at /__replay/barrier, so an unmet precondition
// is an explicit, readable failure rather than a silent non-measurement.
const LEVEL_TIMEOUT_MS = 300;
const parentFile = path.resolve('data', adapter.parent + '.json');
const requestContext = new AsyncLocalStorage();
let barrier = null;
let lastBarrier = { state: 'never-armed', levels: [] };
const originalRead = fs.promises.readFile.bind(fs.promises);
const originalWrite = fs.promises.writeFile.bind(fs.promises);
const originalRename = fs.promises.rename.bind(fs.promises);

function openLevel(current) {
  current.level += 1;
  current.waiters = [];
  clearTimeout(current.timer);
  if (current.arrived.size >= 2) startClock(current);
}
function startClock(current) {
  clearTimeout(current.timer);
  const level = current.level;
  current.timer = setTimeout(() => {
    if (barrier === current && current.level === level) finish(current, 'timeout');
  }, LEVEL_TIMEOUT_MS);
}
function finish(current, reason) {
  // reason: 'timeout' (unmet) or 'write' (measured, race window passed)
  if (barrier !== current) return;
  clearTimeout(current.timer);
  barrier = null;
  const pending = current.waiters;
  const reached = [...new Set(pending.map(w => w.request))];
  const unmet = reason === 'timeout';
  if (unmet || pending.length) {
    current.levels.push({ ordinal: current.level, reached, released_by: reason });
  }
  lastBarrier = {
    state: unmet ? 'unmet' : current.levels.length && current.levels.every(l => l.released_by === 'rendezvous') ? 'measured' : 'unmet',
    reason: unmet
      ? `timeout at read ordinal ${current.level}: ${reached.length} of 2 requests reached it (arrived: ${current.arrived.size})`
      : current.levels.length ? 'rendezvous at every read ordinal before the first write' : 'write before any rendezvous',
    levels: current.levels,
  };
  log('barrier-end', lastBarrier);
  pending.forEach(w => w.resolve());
}
function disarmOnWrite(file) {
  if (!barrier || !barrier.arrived.size) return;
  const target = path.resolve(String(file));
  if (target === barrier.file || target.startsWith(barrier.file + '.')) finish(barrier, 'write');
}
// A failed read (e.g. ENOENT) still counts as a read ordinal; it is re-thrown after the barrier.
const settle = (value, failure) => { if (failure) throw failure.error; return value; };
fs.promises.readFile = async function (file, ...args) {
  let value, failure;
  try { value = await originalRead(file, ...args); } catch (error) { failure = { error }; }
  const context = requestContext.getStore();
  if (!context || path.resolve(String(file)) !== parentFile) return settle(value, failure);
  context.reads += 1;
  const current = barrier;
  if (!current || !current.arrived.has(context.id) || context.reads !== current.level) return settle(value, failure);
  log('read', { request: context.id, ordinal: context.reads, level: current.level, armed: true });
  await new Promise(resolve => {
    current.waiters.push({ request: context.id, resolve });
    const distinct = new Set(current.waiters.map(w => w.request));
    log('barrier-reach', { request: context.id, ordinal: context.reads, distinct: distinct.size });
    if (distinct.size === 2) {
      current.levels.push({ ordinal: current.level, reached: [...distinct], released_by: 'rendezvous' });
      log('barrier-release', { ordinal: current.level, reason: 'rendezvous' });
      const released = current.waiters;
      openLevel(current);
      released.forEach(w => w.resolve());
    }
  });
  return settle(value, failure);
};
fs.promises.writeFile = async function (file, ...args) {
  disarmOnWrite(file);
  return originalWrite(file, ...args);
};
fs.promises.rename = async function (from, to, ...args) {
  disarmOnWrite(to);
  return originalRename(from, to, ...args);
};

let nextRequest = 0;
const server = http.createServer(async (req, res) => {
  try {
    const chunks = [];
    for await (const chunk of req) chunks.push(chunk);
    const body = Buffer.concat(chunks);
    const url = new URL(req.url, 'http://127.0.0.1');
    if (url.pathname === '/__replay/race') {
      const file = parentFile;
      barrier = { file, level: 0, waiters: [], levels: [], arrived: new Set(), timer: null };
      lastBarrier = { state: 'armed', levels: [] };
      openLevel(barrier);
      log('barrier-arm', { file: real(file), timeout_ms: LEVEL_TIMEOUT_MS });
      res.end('{}');
      return;
    }
    if (url.pathname === '/__replay/barrier') {
      res.end(JSON.stringify(barrier ? { state: 'armed', levels: barrier.levels } : lastBarrier));
      return;
    }
    const context = { id: ++nextRequest, reads: 0 };
    log('request', { request: context.id, method: req.method, path: url.pathname + url.search });
    if (barrier && barrier.arrived.size < 2) {
      barrier.arrived.add(context.id);
      if (barrier.arrived.size === 2) startClock(barrier);
    }
    const request = new Request(url, { method: req.method, headers: req.headers,
      ...(body.length ? { body } : {}) });
    request.nextUrl = url;
    const response = await requestContext.run(context, async () => {
      if (correct) return control(request, adapter);
      const segments = url.pathname.split('/').filter(Boolean);
      if (segments[0] !== 'api' || ![adapter.parent, adapter.child].includes(segments[1]) || segments.length > 3) {
        throw new Error('unknown replay route');
      }
      const route = path.join(fixture, 'src/app/api', segments[1], ...(segments[2] ? ['[id]'] : []), 'route.ts');
      const handlers = await import(pathToFileURL(route));
      return handlers[req.method](request, { params: { id: segments[2] } });
    });
    res.writeHead(response.status, Object.fromEntries(response.headers));
    res.end(Buffer.from(await response.arrayBuffer()));
  } catch (error) {
    // Uncaught handler exceptions are HTTP 500 in this adapter, like a failed
    // route invocation. Startup/import failures must be caught by baseline tests.
    res.writeHead(500, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ error: String(error) }));
  }
});
server.listen(0, '127.0.0.1', () => {
  const port = server.address().port;
  log('listen', { pid: process.pid, port });
  console.log(JSON.stringify({ port }));
});
process.on('SIGTERM', () => server.close(() => process.exit(0)));
