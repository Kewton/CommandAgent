#!/usr/bin/env node
// Smoke check: verifies key structural elements in page.tsx

const fs = require("fs");
const path = require("path");

const pagePath = path.join(__dirname, "src", "app", "page.tsx");
const pkgPath = path.join(__dirname, "package.json");

const page = fs.readFileSync(pagePath, "utf8");
const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf8"));

let failures = [];

// Check 'use client'
if (!page.includes('"use client"') && !page.includes("'use client'")) {
  failures.push("page.tsx missing 'use client' directive");
}

// Check data-anvil-action="primary"
if (!page.includes('data-anvil-action="primary"')) {
  failures.push("page.tsx missing data-anvil-action=primary");
}

// Check data-anvil-action="input"
if (!page.includes('data-anvil-action="input"')) {
  failures.push("page.tsx missing data-anvil-action=input");
}

// Check data-anvil-state
if (!page.includes("data-anvil-state")) {
  failures.push("page.tsx missing data-anvil-state");
}

// Check status filters
if (!page.includes("未着手") || !page.includes("進行中") || !page.includes("完了")) {
  failures.push("page.tsx missing status labels");
}

// Check useState usage
if (!page.includes("useState")) {
  failures.push("page.tsx missing useState");
}

// Check port 60302 in dev script
const devScript = pkg.scripts && pkg.scripts.dev;
if (!devScript || !devScript.includes("60302")) {
  failures.push("package.json dev script missing port 60302");
}

// Check build script
if (!pkg.scripts || pkg.scripts.build !== "next build") {
  failures.push("package.json build script not 'next build'");
}

if (failures.length > 0) {
  console.error("SMOKE CHECK FAILED:");
  failures.forEach((f) => console.error("  - " + f));
  process.exit(1);
} else {
  console.log("SMOKE CHECK PASSED: all structural checks OK");
  process.exit(0);
}
