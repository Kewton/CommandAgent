#!/usr/bin/env node
// Transparent observation: run the real npm with identical arguments/environment.
// Never substitutes a build result, installs dependencies, or changes source.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { spawnSync } = require('node:child_process');
const args = process.argv.slice(2);
const config = JSON.parse(fs.readFileSync(path.join(__dirname, "observer.json")));
const real = config.real;
if (!real || !path.isAbsolute(real)) throw Error('ISSUE492_REAL_NPM must be absolute');
const cwd = process.cwd();
const log = path.join(config.logs, path.basename(cwd));
fs.mkdirSync(log, { recursive: true });
const eventPath = path.join(log, 'events.jsonl');
const events = fs.existsSync(eventPath) ? fs.readFileSync(eventPath, 'utf8').trim().split('\n').filter(Boolean).map(JSON.parse) : [];
const digest = p => crypto.createHash('sha256').update(fs.readFileSync(path.join(cwd, p))).digest('hex');
const hashes = Object.fromEntries(['src/app/page.tsx', 'src/app/layout.tsx', 'package.json', 'package-lock.json', 'tsconfig.json', 'next.config.mjs'].map(p => [p, digest(p)]));
const started = Date.now();
const env = { ...process.env, TMPDIR: config.tmp };
const record = { cwd, args, command: ['npm', ...args].join(' '), real, started, hashes,
  node: process.version, NODE_ENV: env.NODE_ENV ?? null, TMPDIR: env.TMPDIR, npm_config_cache: env.npm_config_cache,
  next: require(path.join(cwd, 'node_modules/next/package.json')).version,
  typescript: require(path.join(cwd, 'node_modules/typescript/package.json')).version,
  step: events.findLast(e => e.event === 'plan_step_started'),
  completed_steps: events.filter(e => e.event === 'plan_step_completed').map(e => e.step_id) };
fs.appendFileSync(path.join(log, 'commands.jsonl'), JSON.stringify({ ...record, status: 'started' }) + '\n');
const result = spawnSync(real, args, { cwd, env, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 });
fs.writeFileSync(path.join(log, `npm-${started}.stdout`), result.stdout || '');
fs.writeFileSync(path.join(log, `npm-${started}.stderr`), result.stderr || '');
fs.appendFileSync(path.join(log, 'commands.jsonl'), JSON.stringify({ ...record, status: 'finished', exit: result.status, signal: result.signal, error: result.error?.message, elapsed_ms: Date.now() - started, source_after: digest('src/app/page.tsx') }) + '\n');
process.stdout.write(result.stdout || '');
process.stderr.write(result.stderr || '');
process.exit(result.status ?? 1);
