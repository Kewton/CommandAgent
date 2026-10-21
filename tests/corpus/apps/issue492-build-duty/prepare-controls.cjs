// node prepare-controls.cjs <new-apps-directory> <new-log-directory>
// Uses only this committed fixture, the real npm, and the selected npm cache.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { spawnSync } = require('node:child_process');
const [apps, logs] = process.argv.slice(2).map(p => path.resolve(p));
if (fs.existsSync(apps) || fs.existsSync(logs)) throw Error('Use fresh control directories');
fs.mkdirSync(apps, { recursive: true });
fs.mkdirSync(logs, { recursive: true });
const real = fs.realpathSync(process.env.ISSUE492_REAL_NPM || spawnSync('which', ['npm'], { encoding: 'utf8' }).stdout.trim());
const seed = path.join(apps, 'seed');
fs.cpSync(path.join(__dirname, 'nextjs'), seed, { recursive: true });
const env = { ...process.env, NEXT_TELEMETRY_DISABLED: '1' };
delete env.NODE_ENV;
delete env.NODE_OPTIONS;
const install = spawnSync(real, ['ci', '--include=dev', '--no-audit', '--no-fund'], { cwd: seed, env, encoding: 'utf8' });
fs.writeFileSync(path.join(logs, 'npm-ci.log'), (install.stdout || '') + (install.stderr || ''));
if (install.status !== 0) throw Error(`npm ci failed: ${install.status}`);
const hash = p => crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex');
const inventory = root => {
  const list = [];
  function walk(p) {
    for (const e of fs.readdirSync(p, { withFileTypes: true }).sort((a,b) => a.name.localeCompare(b.name))) {
      const full = path.join(p, e.name), relative = path.relative(root, full);
      if (e.isDirectory()) walk(full);
      else list.push([relative, e.isSymbolicLink() ? fs.readlinkSync(full) : hash(full)]);
    }
  }
  walk(root);
  return crypto.createHash('sha256').update(JSON.stringify(list)).digest('hex');
};
const dependencies = inventory(path.join(seed, 'node_modules'));
const cases = [];
for (const id of ['R1', 'R2', 'R3', 'R4-port', 'R4-build']) {
  const root = path.join(apps, id);
  fs.cpSync(path.join(__dirname, 'nextjs'), root, { recursive: true });
  if (id !== 'R3') fs.cpSync(path.join(seed, 'node_modules'), path.join(root, 'node_modules'), { recursive: true, verbatimSymlinks: true });
  if (id === 'R2') fs.copyFileSync(path.join(__dirname, 'broken-page.tsx'), path.join(root, 'src/app/page.tsx'));
  if (id.startsWith('R4')) {
    const p = JSON.parse(fs.readFileSync(path.join(root, 'package.json')));
    if (id === 'R4-port') p.scripts.dev = 'next dev -p 60303';
    else p.scripts.build = 'next --help';
    fs.writeFileSync(path.join(root, 'package.json'), JSON.stringify(p, null, 2) + '\n');
  }
  const dependencyHash = id === 'R3' ? null : inventory(path.join(root, 'node_modules'));
  if (dependencyHash !== null && dependencyHash !== dependencies) throw Error('dependency mismatch');
  cases.push({ id, root, dependencies: dependencyHash, source: hash(path.join(root, 'src/app/page.tsx')), lockfile: hash(path.join(root, 'package-lock.json')), next_output_exists: fs.existsSync(path.join(root, '.next')) });
}
fs.mkdirSync(path.join(logs, 'bin'));
fs.writeFileSync(path.join(logs, 'bin/observer.json'), JSON.stringify({ real, logs, tmp: process.env.TMPDIR }, null, 2));
fs.copyFileSync(path.join(__dirname, 'npm-observer.cjs'), path.join(logs, 'bin/npm'));
fs.chmodSync(path.join(logs, 'bin/npm'), 0o755);
fs.writeFileSync(path.join(logs, 'environment.json'), JSON.stringify({ node: process.version, npm: spawnSync(real, ['--version'], { encoding: 'utf8' }).stdout.trim(), NODE_ENV: null, devDependencies: true, dependencies, cases }, null, 2));
console.log(JSON.stringify({ apps, logs, dependencies, cases }, null, 2));
