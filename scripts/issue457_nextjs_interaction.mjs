import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

export async function interaction(chromium, url, root) {
  const browser = await chromium.launch({ headless: true });
  const settings = { viewport: { width: 1280, height: 900 }, locale: 'ja-JP', timezoneId: 'Asia/Tokyo' };
  const page = await browser.newPage(settings);
  const evidence = path.join(root, 'browser-evidence');
  fs.mkdirSync(evidence);
  const metadata = { method: 'standalone Playwright, isolated temporary Chromium profile', browser: browser.version(), headless: true, ...settings, target: url };
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const stages = [];
  const taskRow = () => page.locator('li').filter({ hasText: 'Fixture task' });
  const assigned = async expected => {
    await page.waitForFunction(({ expected }) => {
      const row = [...document.querySelectorAll('li')].find(li => li.textContent.includes('Fixture task'));
      return row?.querySelector('select')?.value === expected;
    }, { expected });
  };
  const saved = () => JSON.parse(fs.readFileSync(path.join(root, 'data/tasks.json'), 'utf8'))[0];
  const errorContract = async () => {
    const file = path.join(root, 'data/projects.json');
    const previous = fs.readFileSync(file);
    const tasksFile = path.join(root, 'data/tasks.json');
    const tasksBefore = fs.existsSync(tasksFile) ? fs.readFileSync(tasksFile) : null;
    const projects = JSON.parse(previous);
    const projectId = projects[0].id;
    fs.writeFileSync(tasksFile, '{invalid-tasks');
    try {
      for (const response of [
        await page.request.get(`${url}/api/projects/${projectId}/tasks`),
        await page.request.post(`${url}/api/projects/${projectId}/tasks`, { data: { title: 'Error probe', dueDate: '2026-10-01' } }),
        await page.request.patch(`${url}/api/tasks/probe`, { data: { status: 'completed' } }),
        await page.request.delete(`${url}/api/tasks/probe`),
      ]) {
        assert.equal(response.status(), 500);
        assert.equal(typeof (await response.json()).error, 'string');
      }
      assert.equal(fs.readFileSync(tasksFile, 'utf8'), '{invalid-tasks');
      stages.push('task-route-store-errors-return-500-strings');
    } finally {
      if (tasksBefore) fs.writeFileSync(tasksFile, tasksBefore);
      else fs.unlinkSync(tasksFile);
    }
    fs.writeFileSync(file, '{invalid-json');
    try {
      for (const response of [await page.request.get(url + '/api/projects'), await page.request.post(url + '/api/projects', { data: { name: 'Failure probe', description: '' } })]) {
        assert.equal(response.status(), 500);
        const json = await response.json();
        assert.equal(typeof json.error, 'string');
        assert(json.error.includes('Failed to read projects.json'));
      }
      await page.reload();
      await page.locator('[role=alert]:not(#__next-route-announcer__)').waitFor();
      const visible = await page.locator('[role=alert]:not(#__next-route-announcer__)').innerText();
      assert(visible.includes('Failed to read projects.json'));
      assert(!visible.includes('[object Object]'));
      assert.equal(fs.readFileSync(file, 'utf8'), '{invalid-json');
      if (tasksBefore) assert.deepEqual(fs.readFileSync(tasksFile), tasksBefore);
      await page.screenshot({ path: path.join(evidence, 'store-error.png'), fullPage: true });
      stages.push('store-error-500-and-readable-ui-no-overwrite');
    } finally { fs.writeFileSync(file, previous); }
  };
  const reload = async expected => {
    await page.reload();
    await page.getByText('Fixture project', { exact: true }).click();
    await assigned(expected);
    stages.push(`reload:${expected || 'unassigned'}`);
  };
  try {
    await page.goto(url);
    assert((await page.locator('body').innerText()).includes('プロジェクト作成'));
    await page.getByPlaceholder('プロジェクト名').fill('Preflight');
    assert.equal(await page.getByPlaceholder('プロジェクト名').inputValue(), 'Preflight');
    await page.screenshot({ path: path.join(evidence, 'preflight.png'), fullPage: true });
    assert(fs.statSync(path.join(evidence, 'preflight.png')).size > 0);
    metadata.preflight = { readable_state: 'プロジェクト作成', harmless_interaction: 'Fill project draft with Preflight before submitting any request', screenshot: 'browser-evidence/preflight.png' };
    stages.push('browser-preflight-passed');
    await page.getByPlaceholder('プロジェクト名').fill('Fixture project');
    await page.getByRole('button', { name: '作成', exact: true }).click();
    await page.getByPlaceholder('タスク名').waitFor();
    stages.push('project-created');
    const directory = await (await page.request.get(url + '/api/projects')).json();
    const createForm = page.locator('form').filter({ has: page.getByPlaceholder('タスク名') });
    const directorySelect = createForm.locator('select').nth(1);
    if (!directory.members) {
      assert.equal(await directorySelect.locator('option').count(), 1);
      assert.equal(await directorySelect.inputValue(), '');
      await page.screenshot({ path: path.join(evidence, 'result.png'), fullPage: true });
      await errorContract();
      return { status: 'missing-list', metadata, stages, failure: 'No selectable team member supplied by the application' };
    }
    assert.deepEqual(directory.members.map(m => m.id), ['member-1', 'member-2']);
    assert.equal(await directorySelect.locator('option').count(), 3);
    stages.push('directory-listed');
    await page.getByPlaceholder('タスク名').fill('Fixture task');
    await createForm.locator('input[type=date]').fill('2026-10-01');
    await directorySelect.selectOption('member-1');
    stages.push('member-selected');
    await page.getByRole('button', { name: '追加', exact: true }).click();
    await assigned('member-1');
    assert.deepEqual(saved().assignee, directory.members[0]);
    stages.push('task-created-with-assignee');
    await reload('member-1');
    const responsePromise = page.waitForResponse(r => r.request().method() === 'PATCH');
    await taskRow().locator('select').selectOption('member-2');
    const response = await responsePromise;
    assert.deepEqual(response.request().postDataJSON(), { assignee: directory.members[1] });
    if (response.status() === 400) {
      assert((await response.json()).error.includes('No valid update fields'));
      assert.deepEqual(saved().assignee, directory.members[0]);
      await page.locator('[role=alert]:not(#__next-route-announcer__)').waitFor();
      await reload('member-1');
      await page.screenshot({ path: path.join(evidence, 'result.png'), fullPage: true });
      await errorContract();
      return { status: 'wrong-field', metadata, stages, failure: 'UI assignment PATCH rejected with 400; committed member unchanged after reload' };
    }
    assert.equal(response.status(), 200);
    assert.deepEqual((await response.json()).item.assignee, directory.members[1]);
    await assigned('member-2');
    assert.deepEqual(saved().assignee, directory.members[1]);
    stages.push('assignment-committed');
    await reload('member-2');
    await page.screenshot({ path: path.join(evidence, 'assigned-reloaded.png'), fullPage: true });
    const clearPromise = page.waitForResponse(r => r.request().method() === 'PATCH');
    await taskRow().locator('select').selectOption('');
    const clear = await clearPromise;
    assert.equal(clear.status(), 200);
    assert.deepEqual(clear.request().postDataJSON(), { assignee: null });
    assert.equal((await clear.json()).item.assignee, null);
    await assigned('');
    assert.equal(saved().assignee, null);
    stages.push('unassignment-committed');
    await reload('');
    await taskRow().getByRole('button', { name: '進行中 →' }).click();
    await taskRow().getByText('進行中', { exact: true }).waitFor();
    assert.equal(saved().status, 'in_progress');
    const filter = page.locator('select').filter({ has: page.locator('option[value=all]') });
    await filter.selectOption('completed');
    await taskRow().waitFor({ state: 'detached' });
    await filter.selectOption('in_progress');
    await taskRow().waitFor();
    await taskRow().getByRole('button', { name: '削除', exact: true }).click();
    await taskRow().waitFor({ state: 'detached' });
    assert.deepEqual(JSON.parse(fs.readFileSync(path.join(root, 'data/tasks.json'), 'utf8')), []);
    stages.push('status-filter-delete-preserved');
    assert.deepEqual(errors, []);
    await page.screenshot({ path: path.join(evidence, 'result.png'), fullPage: true });
    await errorContract();
    return { status: 'passed', metadata, stages };
  } finally { await browser.close(); }
}
