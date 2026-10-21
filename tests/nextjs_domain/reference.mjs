// Correct control for the frozen HTTP/disk oracles, not a generation template.
// Single replay process: a queue serializes the entire transaction.
import fs from 'node:fs/promises';
import path from 'node:path';
import { randomUUID } from 'node:crypto';

let pending = Promise.resolve();
export function control(request, adapter) {
  const result = pending.then(() => handle(request, adapter));
  pending = result.catch(() => {});
  return result;
}

async function read(name) {
  try {
    const data = JSON.parse(await fs.readFile(path.join('data', name + '.json'), 'utf8'));
    if (!Array.isArray(data)) throw new Error('invalid stored data');
    return data;
  } catch (e) {
    if (e.code === 'ENOENT') return [];
    throw e;
  }
}
async function write(name, value) {
  const file = path.join('data', name + '.json');
  await fs.mkdir('data', { recursive: true });
  try {
    const mode = (await fs.stat(file)).mode;
    if (!(mode & 0o222)) throw new Error('store is read-only');
  } catch (e) { if (e.code !== 'ENOENT') throw e; }
  const temp = file + '.' + randomUUID() + '.tmp';
  try {
    await fs.writeFile(temp, JSON.stringify(value));
    await fs.rename(temp, file);
  } finally { await fs.rm(temp, { force: true }); }
}
const reject = () => Response.json({ error: 'constraint violated' }, { status: 409 });
const minutes = t => Number(t.slice(0, 2)) * 60 + Number(t.slice(3));
function validShift(item, children) {
  const start = minutes(item.startTime), end = minutes(item.endTime);
  return Number.isFinite(start + end + item.breakMinutes) && start % 30 === 0 && end % 30 === 0
    && end > start && item.breakMinutes >= 0 && item.breakMinutes < end - start
    && !children.some(other => other.staffId === item.staffId && other.date === item.date
      && minutes(other.startTime) < end && start < minutes(other.endTime));
}

async function handle(request, a) {
  try {
    const url = new URL(request.url), segments = url.pathname.split('/');
    const name = segments[2], method = request.method;
    const parents = await read(a.parent), children = await read(a.child);
    const rows = name === a.parent ? parents : children;
    const reply = (value, status = 200) => Response.json(a.wrapped_collections.includes(name)
      ? { [Array.isArray(value) ? name : name === a.child ? 'shift' : 'staff']: value }
      : value, { status });
    if (method === 'GET') return reply(rows);
    const body = method === 'DELETE' && a.delete_style === 'path' ? {} : await request.json();
    const id = segments[3] || url.searchParams.get('id') || body.id;
    if (method === 'POST') {
      const item = { ...body, id: randomUUID() };
      if (name === a.child) {
        if (a.domain === 'shift' && (!parents.some(p => p.id === item.staffId) || !validShift(item, children))) return reject();
        if (a.domain === 'expense') {
          if (!Number.isFinite(item.amount) || item.amount <= 0 || !parents.some(p => p.id === item.departmentId)) return reject();
          item.status = '申請中';
        }
        if (a.domain === 'inventory') item.status = 'draft';
      }
      rows.push(item);
      await write(name, rows);
      return reply(item, 201);
    }
    const item = rows.find(row => row.id === id);
    if (!item) return Response.json({ error: 'missing' }, { status: 404 });
    if (method === 'DELETE') {
      if (name === a.parent && children.some(row => row.staffId === id)) return reject();
      await write(name, rows.filter(row => row.id !== id));
      return Response.json({ success: true });
    }
    if (a.domain === 'inventory' && body.status === 'confirmed') {
      const totals = new Map();
      for (const line of item.items) totals.set(line.productId, (totals.get(line.productId) || 0) + line.quantity);
      for (const [productId, quantity] of totals) {
        const product = parents.find(p => p.id === productId);
        if (!product || quantity > product.stock || quantity <= 0) return reject();
      }
      for (const [productId, quantity] of totals) parents.find(p => p.id === productId).stock -= quantity;
      await write(a.parent, parents);
      item.status = 'confirmed';
    } else if (a.domain === 'expense' && body.action === 'approve') {
      const department = parents.find(p => p.id === item.departmentId);
      const spent = children.filter(row => row.departmentId === item.departmentId
        && row.date.slice(0, 7) === item.date.slice(0, 7) && row.status === '承認')
        .reduce((total, row) => total + row.amount, 0);
      if (spent + item.amount > department.monthlyBudget) return reject();
      item.status = '承認';
    } else return reject();
    await write(name, rows);
    return reply(item);
  } catch (error) { return Response.json({ error: String(error) }, { status: 500 }); }
}
