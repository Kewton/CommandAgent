// Reproduce archived contracts only in new disposable workspaces.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { interaction } from './issue457_nextjs_interaction.mjs';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const fixture = path.join(repo, 'tests/corpus/apps/issue457-nextjs-contracts');
const original = path.join(repo, 'tests/corpus/apps/issue456-create-recovery/original');
const args = Object.fromEntries(process.argv.slice(2).reduce((a, v, i, all) => i % 2 ? a : [...a, [v, all[i + 1]]], []));
assert(args['--work-root'] && args['--output'] && args['--playwright-module'], 'Pass --work-root NEW_DIR --output NEW_JSON --playwright-module ABSOLUTE_MODULE');
assert(!fs.existsSync(args['--output']), 'Never overwrite evidence');
const work = path.resolve(args['--work-root']);
assert(!fs.existsSync(work), 'Work root must be new');
fs.mkdirSync(work, { recursive: true });
const env = { ...process.env, NEXT_TELEMETRY_DISABLED: '1', CI: '1', NO_COLOR: '1', npm_config_cache: path.join(work, 'npm-cache') };
delete env.NODE_ENV;
delete env.NODE_OPTIONS;
const hash = p => createHash('sha256').update(fs.readFileSync(p)).digest('hex');
function files(root, prefix = '') {
  return Object.fromEntries(fs.readdirSync(path.join(root, prefix), { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name)).flatMap(e => {
    const p = path.join(prefix, e.name);
    return e.isDirectory() ? Object.entries(files(root, p)) : [[p, hash(path.join(root, p))]];
  }));
}
function run(command, cwd) {
  const r = spawnSync(command[0], command.slice(1), { cwd, env, encoding: 'utf8', timeout: 240000, maxBuffer: 4 * 1024 * 1024 });
  assert.ifError(r.error);
  return { exit: r.status, output: (r.stdout + r.stderr).replace(/\x1b\[[0-9;]*m/g, '') };
}
const before = files(fixture);
const provenance = JSON.parse(fs.readFileSync(path.join(repo, 'tests/corpus/apps/issue456-create-recovery/provenance.json')));
assert.deepEqual(files(original), provenance.original_product_files_sha256);
assert.equal(hash(path.join(fixture, 'package-lock.json')), '95d887c375a5a4d5cf093ee39119f558e61ea999bb906e5895138babae74f832');
const deps = path.join(work, 'dependencies');
fs.mkdirSync(deps);
fs.copyFileSync(path.join(original, 'package.json'), path.join(deps, 'package.json'));
fs.copyFileSync(path.join(fixture, 'package-lock.json'), path.join(deps, 'package-lock.json'));
console.log('Installing frozen dependencies');
const install = run(['npm', 'ci', '--include=dev', '--no-audit', '--no-fund'], deps);
assert.equal(install.exit, 0, install.output);
const require = createRequire(path.join(deps, 'package.json'));
const ts = require('typescript');
assert.equal(ts.version, '5.9.3');
const { chromium } = require(args['--playwright-module']);
assert(fs.existsSync(chromium.executablePath()), 'Installed browser executable required');
const controls = JSON.parse(fs.readFileSync(path.join(fixture, 'controls.json')));
for (const c of controls) {
  const root = path.join(work, 'controls', c.name);
  fs.mkdirSync(root, { recursive: true });
  fs.symlinkSync(path.join(deps, 'node_modules'), path.join(root, 'node_modules'), 'dir');
  for (const name of ['client', 'server']) fs.writeFileSync(path.join(root, name + '.tsx'), c[name]);
  const program = ts.createProgram(['client', 'server'].map(n => path.join(root, n + '.tsx')), { strict: true, noEmit: true, skipLibCheck: true, target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS, moduleResolution: ts.ModuleResolutionKind.Node10, jsx: ts.JsxEmit.ReactJSX });
  const diagnostics = ts.getPreEmitDiagnostics(program);
  assert.equal(diagnostics.length, 0, c.name + ': ' + diagnostics.map(d => ts.flattenDiagnosticMessageText(d.messageText, '\n')).join('\n'));
}
const results = [];
for (const c of JSON.parse(fs.readFileSync(path.join(fixture, 'cases.json')))) {
  const root = path.join(work, c.name);
  fs.cpSync(original, root, { recursive: true, errorOnExist: true });
  for (const overlay of c.overlays) fs.cpSync(path.join(fixture, 'overlays', overlay), root, { recursive: true });
  const sourceHashes = files(root);
  fs.symlinkSync(path.join(deps, 'node_modules'), path.join(root, 'node_modules'), 'dir');
  // Identical resource limit in every variant; no type/lint/build gate override.
  fs.writeFileSync(path.join(root, 'next.config.js'), 'module.exports = { experimental: { cpus: 2 } };\n');
  const cfg = ts.readConfigFile(path.join(root, 'tsconfig.json'), ts.sys.readFile);
  assert.equal(cfg.error, undefined);
  const parsed = ts.parseJsonConfigFileContent(cfg.config, ts.sys, root);
  assert.equal(parsed.errors.length, 0);
  assert.equal(parsed.options.strict, true);
  const program = ts.createProgram(parsed.fileNames, { ...parsed.options, noEmit: true, incremental: false });
  const diagnostics = ts.getPreEmitDiagnostics(program).map(d => {
    const pos = d.file?.getLineAndCharacterOfPosition(d.start || 0);
    return { code: d.code, file: d.file && path.relative(root, d.file.fileName), line: pos && pos.line + 1, column: pos && pos.character + 1, message: ts.flattenDiagnosticMessageText(d.messageText, '\n') };
  });
  assert.equal(diagnostics.length, c.diagnostics, JSON.stringify({ case: c.name, diagnostics }, null, 2));
  console.log(`${c.name}: ${diagnostics.length} diagnostics; building`);
  const build = run(['npm', 'run', 'build'], root);
  assert.equal(build.exit === 0, c.build, build.output);
  assert(build.output.includes('Linting and checking validity of types'), build.output);
  let observed = null;
  if (c.build) {
    for (const route of ['/api/projects', '/api/projects/[id]/tasks', '/api/tasks/[id]']) assert(build.output.includes(route), build.output);
    const server = spawn(process.execPath, [path.join(deps, 'node_modules/next/dist/bin/next'), 'start', '-p', '0', '-H', '127.0.0.1'], { cwd: root, env, stdio: ['ignore', 'pipe', 'pipe'] });
    let logs = '';
    server.stdout.on('data', b => { logs += b; });
    server.stderr.on('data', b => { logs += b; });
    try {
      const deadline = Date.now() + 30000;
      while (!logs.includes('Ready in') && Date.now() < deadline && server.exitCode === null) await new Promise(r => setTimeout(r, 100));
      assert(logs.includes('Ready in'), logs);
      const port = logs.match(/http:\/\/127\.0\.0\.1:(\d+)/)?.[1] || logs.match(/http:\/\/localhost:(\d+)/)?.[1];
      assert(port, logs);
      observed = await interaction(chromium, `http://127.0.0.1:${port}`, root);
      assert.equal(observed.status, c.interaction, JSON.stringify(observed));
    } finally {
      if (server.exitCode === null) { server.kill('SIGTERM'); await once(server, 'exit'); }
    }
  }
  for (const [p, sha] of Object.entries(sourceHashes)) assert.equal(hash(path.join(root, p)), sha, `${c.name}: source changed: ${p}`);
  results.push({ case: c.name, diagnostics, build_exit: build.exit, type_check_observed: true, interaction: observed, source_sha256: sourceHashes });
  console.log(`${c.name}: expected result verified`);
}
assert.deepEqual(files(fixture), before);
assert.deepEqual(files(original), provenance.original_product_files_sha256);
fs.writeFileSync(args['--output'], JSON.stringify({ node: process.version, typescript: ts.version, next: require('next/package.json').version, playwright: require(path.join(args['--playwright-module'], 'package.json')).version, browser_executable: chromium.executablePath(), typed_controls: controls.map(c => c.name), fixture_sha256: before, results, limitation: 'Deterministic archived-source fixture, not live model improvement or R0 final acceptance.' }, null, 2) + '\n', { flag: 'wx' });
