// The unmodified saved TypeScript store runs on Node 24. This is a deterministic
// storage-contract check, not evidence of the original R0 business application.
import { loadProjects, loadTasks, saveProjects } from './src/lib/store.ts';
import { promises as fs } from 'node:fs';
import assert from 'node:assert/strict';
const projects = await loadProjects();
assert.ok(Array.isArray(projects));
assert.ok(Array.isArray(await loadTasks()));
await saveProjects([...projects, {id: 'controlled', name: 'Fixture'}]);
assert.equal((await loadProjects()).at(-1).id, 'controlled');
if (process.argv[2] === 'extra') await fs.writeFile('data/unregistered.json', '[]');
if (process.argv[2] === 'source') await fs.appendFile('src/lib/types.ts', '\n// forbidden\n');
if (process.argv[2] === 'config') await fs.writeFile('tsconfig.json', '{}');
if (process.argv[2] === 'business-failure') process.exitCode = 1;
