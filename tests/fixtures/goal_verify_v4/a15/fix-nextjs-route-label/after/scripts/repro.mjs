#!/usr/bin/env node
import { readFile } from "node:fs/promises";
import { formatTask } from "../lib/label.mjs";

if (process.argv.length !== 3 || !process.argv[2].startsWith("fixture/task-")) {
  process.exit(2);
}
const fixture = JSON.parse(await readFile(process.argv[2], "utf8"));
const actual = formatTask(fixture.task);
if (actual !== fixture.expected) {
  console.error(`expected ${fixture.expected}, got ${actual}`);
  process.exit(1);
}
