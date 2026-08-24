const fs = require('fs');

function read(path) {
  return fs.readFileSync(path, 'utf8');
}

// The L2 schema is intentionally checked without an undeclared YAML dependency.
// This parser extracts the unambiguous top-level mapping keys used by the pinned spec.
function parseTopLevelSections(yaml) {
  return new Set(yaml.split(/\r?\n/)
    .map((line) => line.match(/^([A-Za-z][A-Za-z0-9_]*):\s*(?:$|\S)/))
    .filter(Boolean)
    .map((match) => match[1]));
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

try {
  const spec = read('app.spec.yaml');
  const app = read('src/app-zone/app.js');
  const html = read('src/app-zone/index.html');
  const sections = parseTopLevelSections(spec);
  ['entities', 'views', 'actions', 'validations', 'computed', 'permissions', 'minIdentity']
    .forEach((section) => assert(sections.has(section), `missing L2 section: ${section}`));
  ['User:', 'Expense:', 'Settlement:', 'ExpenseEntryView:', 'SettlementResultsView:', 'AddExpense:', 'CalculateSettlement:']
    .forEach((definition) => assert(spec.includes(definition), `missing definition: ${definition}`));
  assert(spec.includes('type: "number"'), 'amount type is not statically declared');
  assert(spec.includes('function: "calculateNetBalances"'), 'net balance computation is not declared');
  assert(spec.includes('function: "calculateOptimalPairwiseTransfers"'), 'settlement computation is not declared');
  assert(app.includes('function calculateNetBalances'), 'net balance implementation is missing');
  assert(app.includes('function calculateOptimalPairwiseTransfers'), 'settlement implementation is missing');
  assert(app.includes('module.exports'), 'pure functions are not exported');
  assert(html.includes('id="expense-form"') && html.includes('src="app.js"'), 'expense app is not wired');
  process.exit(0);
} catch (error) {
  console.error(`spec verification failed: ${error.message}`);
  process.exit(1);
}
