const fs = require('fs');

const spec = fs.readFileSync('app.spec.yaml', 'utf8');
const requiredSections = [
  'entities:',
  'views:',
  'actions:',
  'validations:',
  'computed:',
  'permissions:',
  'minIdentity:'
];
const requiredDefinitions = [
  '  User:',
  '  Expense:',
  '  Settlement:',
  '  ExpenseEntryView:',
  '  SettlementResultsView:',
  '  AddExpense:',
  '  CalculateSettlement:'
];

const complete = requiredSections.every((section) => spec.includes(section)) &&
  requiredDefinitions.every((definition) => spec.includes(definition));
process.exit(complete ? 0 : 1);
