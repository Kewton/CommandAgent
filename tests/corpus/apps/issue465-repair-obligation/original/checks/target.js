import assert from 'node:assert/strict';
import { POST } from '../src/api.js';
assert.equal(POST({hours: 8}), true);
assert.equal(POST({hours: 9}), false);
assert.equal(POST({hours: 0}), false);
console.log('target reached: POST -> validateShift -> policy; positive and negative inputs passed');
