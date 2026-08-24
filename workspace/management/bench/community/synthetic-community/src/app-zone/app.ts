let count = 0;
const label = document.querySelector('[data-testid="count"]') as HTMLElement;
const render = () => { label.textContent = String(count); };
document.querySelector('[data-action="increment"]')?.addEventListener('click', () => { count += 1; render(); });
document.querySelector('[data-action="reset"]')?.addEventListener('click', () => { count = 0; render(); });
render();
