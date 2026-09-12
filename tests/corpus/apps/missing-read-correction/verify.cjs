const assert = require('node:assert/strict');
assert.deepEqual(require('./src/lib/persistence.cjs').loadProjects(), []);
