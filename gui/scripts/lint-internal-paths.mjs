import { readFile, readdir } from "node:fs/promises";
import { extname, join, relative } from "node:path";

const root = new URL("..", import.meta.url).pathname;
const sourceRoots = ["app", "components", "lib"];
const helper = join(root, "lib", "base-path.ts");
const forbidden = [
  /(?:href|src)\s*=\s*["'{`]\s*\//g,
  /fetch\s*\(\s*["'`]\s*\//g,
  /new\s+URL\s*\(\s*["'`]\s*\//g,
];

async function filesBelow(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await filesBelow(path)));
    } else if ([".ts", ".tsx"].includes(extname(path))) {
      files.push(path);
    }
  }
  return files;
}

const findings = [];
for (const sourceRoot of sourceRoots) {
  for (const path of await filesBelow(join(root, sourceRoot))) {
    if (path === helper) continue;
    const source = await readFile(path, "utf8");
    for (const pattern of forbidden) {
      for (const match of source.matchAll(pattern)) {
        const line = source.slice(0, match.index).split("\n").length;
        findings.push(`${relative(root, path)}:${line}: ${match[0].trim()}`);
      }
    }
  }
}

if (findings.length > 0) {
  console.error("Internal GUI URLs must use lib/base-path.ts:\n" + findings.join("\n"));
  process.exitCode = 1;
} else {
  console.log("basePath audit: all internal links and fetches use the helper");
}
