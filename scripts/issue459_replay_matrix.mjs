// Run every registered scenario against the normally compiled product library.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const args = Object.fromEntries(process.argv.slice(2).reduce((items, value, i, all) => i % 2 ? items : [...items, [value, all[i + 1]]], []));
assert(args['--work-root'] && args['--node-modules'] && args['--playwright-module'], 'Pass --work-root NEW_DIR --node-modules ABS_DIR --playwright-module ABS_MODULE');
const root = path.resolve(args['--work-root']);
assert(!fs.existsSync(root), 'Never overwrite previous evidence');
fs.mkdirSync(root, { recursive: true });
const cases = JSON.parse(fs.readFileSync(path.join(repo, 'tests/corpus/apps/issue459-create-recovery-integration/replay-cases.json')));
const results = [];
for (const c of cases) {
  console.log(`${c.name}: starting real Recovery replay`);
  const log = fs.openSync(path.join(root, c.name + '.log'), 'wx');
  let run;
  try {
    run = spawnSync('cargo', ['test', '--test', 'issue459_recovery', 'issue459_installed', '--', '--ignored', '--nocapture'], {
      cwd: repo, stdio: ['ignore', log, log], timeout: 600000,
      env: { ...process.env, ISSUE459_SCENARIO: c.name, ISSUE459_WORK_ROOT: path.join(root, c.name), ISSUE459_NODE_MODULES: path.resolve(args['--node-modules']), ISSUE459_PLAYWRIGHT_MODULE: path.resolve(args['--playwright-module']) },
    });
  } finally { fs.closeSync(log); }
  assert.ifError(run.error);
  assert.equal(run.status, 0, `${c.name}: see ${path.join(root, c.name + '.log')}`);
  const result = JSON.parse(fs.readFileSync(path.join(root, c.name, 'result.json')));
  results.push({ scenario: c.name, test_status: 'passed', candidate_success: result.success, starts: result.starts.length, decisions: result.decisions.filter(e => ['recovery_promotion_decision', 'recovery_plan_auto_run_stopped', 'recovery_plan_auto_run_complete'].includes(e.event)), control_before: result.control_before, control_after: result.control_after, candidates: result.candidates });
  console.log(`${c.name}: assertions passed; candidate ${result.success ? 'promoted' : 'rejected'}`);
}
fs.writeFileSync(path.join(root, 'summary.json'), JSON.stringify({ status: 'passed', scenarios: results }, null, 2) + '\n', { flag: 'wx' });
