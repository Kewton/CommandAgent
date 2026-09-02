#!/usr/bin/env node
import assert from "node:assert/strict";

import { stableLabel } from "../lib/label.mjs";

assert.equal(stableLabel("health"), "health");
