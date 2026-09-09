// Additional, explicitly registered fixture oracle. Observe a fresh copy of the
// just-built candidate, preserving the runner's source and runtime namespace.
import assert from 'node:assert/strict';
import test from 'node:test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { createHash } from 'node:crypto';
import { interaction } from './interaction.mjs';

test('candidate member listing, selection, assignment and reload', async () => {
  const candidate = process.cwd();
  const require = createRequire(path.join(candidate, 'package.json'));
  const { chromium } = require('playwright');
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'issue459-oracle-'));
  const files = ['src', '.next', 'package.json', 'package-lock.json', 'next.config.js', 'tsconfig.json', 'next-env.d.ts', 'postcss.config.js', 'tailwind.config.ts'];
  const hashes = {};
  function copy(name) {
    const source = path.join(candidate, name);
    if (fs.statSync(source).isDirectory()) {
      fs.mkdirSync(path.join(root, name), { recursive: true });
      for (const entry of fs.readdirSync(source)) copy(path.join(name, entry));
    } else {
      fs.copyFileSync(source, path.join(root, name));
      if (!name.startsWith('.next/')) hashes[name] = createHash('sha256').update(fs.readFileSync(source)).digest('hex');
    }
  }
  for (const name of files) copy(name);
  fs.symlinkSync(path.join(candidate, 'node_modules'), path.join(root, 'node_modules'), 'dir');
  const server = spawn(process.execPath, [require.resolve('next/dist/bin/next'), 'start', '-p', '0', '-H', '127.0.0.1'], {
    cwd: root, env: { ...process.env, NEXT_TELEMETRY_DISABLED: '1' }, stdio: ['ignore', 'pipe', 'pipe'],
  });
  let logs = '';
  server.stdout.on('data', b => { logs += b; });
  server.stderr.on('data', b => { logs += b; });
  try {
    const deadline = Date.now() + 30000;
    while (!logs.includes('Ready in') && Date.now() < deadline && server.exitCode === null) await new Promise(r => setTimeout(r, 100));
    assert(logs.includes('Ready in'), logs);
    const port = logs.match(/http:\/\/127\.0\.0\.1:(\d+)/)?.[1] || logs.match(/http:\/\/localhost:(\d+)/)?.[1];
    assert(port, logs);
    const result = await interaction(chromium, `http://127.0.0.1:${port}`, root);
    fs.mkdirSync(path.join(candidate, 'evidence'), { recursive: true });
    fs.writeFileSync(path.join(candidate, 'evidence/issue459-assignment.json'), JSON.stringify({ ...result, source_sha256: hashes }, null, 2));
    console.log(JSON.stringify({ status: result.status, stages: result.stages }));
    assert.equal(result.status, 'passed', 'Assignment listing/selection/update/reload must all execute and pass');
  } finally {
    if (server.exitCode === null) { server.kill('SIGTERM'); await once(server, 'exit'); }
  }
});
