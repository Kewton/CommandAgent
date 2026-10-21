const fs = require('node:fs');
if (fs.readFileSync('value.txt', 'utf8').trim() !== '42') {
  console.error('value must equal 42');
  process.exit(1);
}
