#!/usr/bin/env node
import { stableLabel } from "../lib/label.mjs";

if (stableLabel("health") !== "health") {
  process.exit(1);
}
