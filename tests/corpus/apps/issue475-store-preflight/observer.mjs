// Scripted build/server/interaction transport through the product observer.
// It executes the saved store; it does not compile Next.js or drive a browser.
import { promises as fs } from 'node:fs';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';
const mode = process.argv[2];
const files = ['data/projects.json', 'data/tasks.json'];
async function hashes() {
  const result = {};
  for (const p of files) {
    try { result[p] = createHash('sha256').update(await fs.readFile(p)).digest('hex'); }
    catch (e) { if (e.code !== 'ENOENT') throw e; result[p] = null; }
  }
  return result;
}
async function record(operation, before) {
  await fs.mkdir('.commandagent/evidence', {recursive:true});
  await fs.appendFile('.commandagent/evidence/issue475-operations.jsonl', JSON.stringify({operation, before, after:await hashes()})+'\n');
}
const beforeImport = await hashes();
const { loadProjects, loadTasks, saveProjects } = await import('./src/lib/store.ts');
if (mode === 'build') {
  assert.deepEqual(await hashes(), beforeImport);
  await record('build:import-store', beforeImport);
  await fs.mkdir('.anvil/evidence', {recursive:true});
  await fs.writeFile('.anvil/evidence/interaction-probe-availability.json', JSON.stringify({available:true}));
  await fs.writeFile('.anvil/evidence/interaction-probe-node-program.json', JSON.stringify({node_program:resolve('interaction.sh')}));
} else if (mode === 'start') {
  assert.deepEqual(await hashes(), beforeImport);
  await record('start:import-store', beforeImport);
  createServer(async (req,res) => {
    try {
      const before = await hashes();
      let value = '<html><body>Controlled store observer</body></html>';
      if (req.url === '/api/projects') {
        if (req.method === 'POST') await saveProjects([...(await loadProjects()), {id:'observer', name:'Controlled'}]);
        value = await loadProjects();
      }
      if (req.url === '/api/tasks') value = await loadTasks();
      await record(`${req.method} ${req.url}`, before);
      res.writeHead(200, {'Content-Type': req.url === '/' ? 'text/html' : 'application/json'});
      res.end(typeof value === 'string' ? value : JSON.stringify(value));
    } catch (e) { res.writeHead(500); res.end(String(e)); }
  }).listen(Number(process.env.PORT), '127.0.0.1');
} else if (mode === 'interaction') {
  const url = process.argv[3];
  for (const path of ['/api/projects', '/api/tasks']) {
    const res = await fetch(new URL(path,url)); assert.equal(res.status,200); assert.ok(Array.isArray(await res.json()));
  }
  const res = await fetch(new URL('/api/projects',url), {method:'POST'});
  assert.equal(res.status,200); assert.equal((await res.json()).at(-1).id,'observer');
  // A storage roundtrip never fabricates UI/business acceptance evidence.
  await fs.writeFile(process.argv[4], JSON.stringify({ok:false, status:'failed', failure_kind:'issue475_business_unobserved', failure_category:'behavior', state_changed:false, visible_state_changed:false, steps:['controlled_HTTP_storage_roundtrip']}));
} else { throw new Error('unknown mode'); }
