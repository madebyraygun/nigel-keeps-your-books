import { describe, it, expect, afterEach } from 'vitest';
import './wc-stat-card.js';
import type { WcStatCard } from './wc-stat-card.js';
import type { WcMoney } from './wc-money.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-stat-card.preview.js';

async function mount(props: Partial<WcStatCard> = {}): Promise<WcStatCard> {
  const el = document.createElement('wc-stat-card');
  Object.assign(el, { label: 'YTD Income', ...props });
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function query<T extends Element>(el: WcStatCard, selector: string): T | null {
  return el.shadowRoot?.querySelector<T>(selector) ?? null;
}

describe('wc-stat-card', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders its label', async () => {
    const el = await mount({ label: 'YTD Expenses' });
    expect(query(el, '.label')?.textContent?.trim()).toBe('YTD Expenses');
  });

  it('hands the amount to wc-money rather than formatting it itself', async () => {
    const el = await mount({ amount: -96540.18 });
    expect(query<WcMoney>(el, 'wc-money')?.amount).toBe(-96540.18);
  });

  it('passes the variant through to wc-money', async () => {
    const el = await mount({ amount: 10, variant: 'plain' });
    expect(query<WcMoney>(el, 'wc-money')?.variant).toBe('plain');
  });

  it('renders a hint only when given one', async () => {
    expect(query(await mount(), '.hint')).toBeNull();
    const el = await mount({ hint: 'Year to date' });
    expect(query(el, '.hint')?.textContent?.trim()).toBe('Year to date');
  });

  it('shows a spinner instead of a value while loading', async () => {
    const el = await mount({ loading: true, amount: 5 });
    expect(query(el, 'wc-spinner')).not.toBeNull();
    expect(query(el, 'wc-money')).toBeNull();
  });

  it('shows the error instead of a stale value', async () => {
    const el = await mount({ amount: 5, error: 'Could not reach the server.' });
    expect(query(el, '.error')?.textContent?.trim()).toBe(
      'Could not reach the server.',
    );
    expect(query(el, 'wc-money')).toBeNull();
  });

  it('fires nc-retry from the error state', async () => {
    const el = await mount({ error: 'boom' });
    let fired = 0;
    el.addEventListener('nc-retry', () => (fired += 1));
    query<HTMLButtonElement>(el, '.retry')?.click();
    expect(fired).toBe(1);
  });

  it('lets nc-retry cross the shadow boundary', async () => {
    const el = await mount({ error: 'boom' });
    let fired = 0;
    document.body.addEventListener('nc-retry', () => (fired += 1));
    query<HTMLButtonElement>(el, '.retry')?.click();
    expect(fired).toBe(1);
  });
});

describePreviewA11y(preview);
