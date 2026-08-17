// Integrated L2 expense input and settlement view.
const expenses = [];

function roundCurrency(value) {
  return Math.round((value + Number.EPSILON) * 100) / 100;
}

function parseParticipants(value) {
  return value.split(',').map((name) => name.trim()).filter(Boolean);
}

function validateExpense(payerName, amount, description, participants) {
  if (!payerName.trim()) return '支払った人を入力してください。';
  if (!Number.isFinite(amount) || amount <= 0) return '金額は正の数で入力してください。';
  if (!description.trim()) return '内容を入力してください。';
  if (participants.length === 0) return '参加者を1人以上入力してください。';
  return null;
}

function createExpense(payerName, amount, description, participants) {
  return {
    id: `expense-${Date.now()}-${expenses.length}`,
    groupId: 'local-trip',
    paidBy: payerName.trim(),
    description: description.trim(),
    amount: roundCurrency(amount),
    participants: [...new Set(participants)],
    occurredOn: new Date().toISOString().slice(0, 10)
  };
}

// Positive balance means the person should receive money; negative means they owe.
function calculateNetBalances(expenseList) {
  const balances = {};
  expenseList.forEach((expense) => {
    const participants = [...new Set(expense.participants || [])];
    if (!expense.paidBy || !Number.isFinite(expense.amount) || expense.amount <= 0 || participants.length === 0) return;
    const share = expense.amount / participants.length;
    balances[expense.paidBy] = (balances[expense.paidBy] || 0) + expense.amount;
    participants.forEach((participant) => {
      balances[participant] = (balances[participant] || 0) - share;
    });
  });
  Object.keys(balances).forEach((name) => { balances[name] = roundCurrency(balances[name]); });
  return balances;
}

function calculateOptimalPairwiseTransfers(balances) {
  const creditors = Object.keys(balances).filter((name) => balances[name] > 0.005)
    .map((name) => ({ name, amount: roundCurrency(balances[name]) }));
  const debtors = Object.keys(balances).filter((name) => balances[name] < -0.005)
    .map((name) => ({ name, amount: roundCurrency(-balances[name]) }));
  const transfers = [];
  let creditorIndex = 0;
  let debtorIndex = 0;

  while (creditorIndex < creditors.length && debtorIndex < debtors.length && transfers.length < 100) {
    const creditor = creditors[creditorIndex];
    const debtor = debtors[debtorIndex];
    const amount = roundCurrency(Math.min(creditor.amount, debtor.amount));
    if (amount > 0) transfers.push({ fromUser: debtor.name, toUser: creditor.name, amount, currency: 'JPY' });
    creditor.amount = roundCurrency(creditor.amount - amount);
    debtor.amount = roundCurrency(debtor.amount - amount);
    if (creditor.amount <= 0.005) creditorIndex += 1;
    if (debtor.amount <= 0.005) debtorIndex += 1;
  }
  return transfers;
}

function renderExpenses(expenseList) {
  const list = document.getElementById('expense-list');
  const emptyMessage = document.getElementById('empty-message');
  list.replaceChildren();
  emptyMessage.hidden = expenseList.length > 0;
  expenseList.forEach((expense) => {
    const item = document.createElement('li');
    item.className = 'expense';
    const details = document.createElement('span');
    details.innerHTML = `<span class="expense-description"></span><span class="expense-meta"></span>`;
    details.querySelector('.expense-description').textContent = expense.description;
    details.querySelector('.expense-meta').textContent = ` ${expense.paidBy} ・ ${expense.participants.join('、')}`;
    const amount = document.createElement('strong');
    amount.textContent = `${expense.amount.toLocaleString('ja-JP')} 円`;
    item.append(details, amount);
    list.append(item);
  });
}

function renderSettlements(transfers) {
  const list = document.getElementById('settlement-list');
  const emptyMessage = document.getElementById('settlement-empty');
  list.replaceChildren();
  emptyMessage.hidden = transfers.length > 0;
  transfers.forEach((transfer) => {
    const item = document.createElement('li');
    item.className = 'settlement';
    item.textContent = `${transfer.fromUser} → ${transfer.toUser}：${transfer.amount.toLocaleString('ja-JP')} ${transfer.currency}`;
    list.append(item);
  });
}

function renderAll() {
  renderExpenses(expenses);
  renderSettlements(calculateOptimalPairwiseTransfers(calculateNetBalances(expenses)));
}

const form = document.getElementById('expense-form');
form.addEventListener('submit', (event) => {
  event.preventDefault();
  const payerName = document.getElementById('payer-name').value;
  const amount = Number(document.getElementById('expense-amount').value);
  const description = document.getElementById('expense-description').value;
  const participants = parseParticipants(document.getElementById('expense-participants').value);
  const error = validateExpense(payerName, amount, description, participants);
  if (error) { window.alert(error); return; }
  expenses.push(createExpense(payerName, amount, description, participants));
  renderAll();
  form.reset();
  document.getElementById('payer-name').focus();
});

if (typeof window !== 'undefined') {
  window.Warikan = { calculateNetBalances, calculateOptimalPairwiseTransfers, parseParticipants, validateExpense };
}
if (typeof module !== 'undefined' && module.exports) {
  module.exports = { calculateNetBalances, calculateOptimalPairwiseTransfers, parseParticipants, validateExpense };
}
renderAll();
