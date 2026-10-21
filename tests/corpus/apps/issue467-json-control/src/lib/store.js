import { promises as fs } from "node:fs";
import { join } from "node:path";

const DATA_DIR = join(process.cwd(), "data");
const DATA_FILE = join(DATA_DIR, "inquiries.json");
let writeChain = Promise.resolve();

export async function readData() {
  try {
    return JSON.parse(await fs.readFile(DATA_FILE, "utf-8"));
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
    const sample = { inquiries: [{ id: "initial" }], assignees: [] };
    await writeData(sample);
    return sample;
  }
}

export async function writeData(data) {
  const task = writeChain.then(async () => {
    await fs.mkdir(DATA_DIR, { recursive: true });
    const tmp = DATA_FILE + `.tmp-${process.pid}-${Date.now()}`;
    const json = JSON.stringify(data, null, 2);
    await fs.writeFile(tmp, json, "utf-8");
    await fs.rename(tmp, DATA_FILE);
  });
  writeChain = task.catch(() => {});
  await task;
}
