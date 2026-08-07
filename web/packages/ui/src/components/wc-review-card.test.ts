import { describe, it, expect, afterEach } from 'vitest';
import './wc-review-card.js';
import type { WcReviewCard } from './wc-review-card.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-review-card.preview.js';

async function mount(props: Partial<WcReviewCard> = {}): Promise<WcReviewCard> {
  const el = document.createElement('wc-review-card');
  Object.assign(el, {
    date: '2025-03-04',
    description: 'ADOBE CREATIVE CLOUD',
    amount: -54.99,
    accountName: 'BofA Credit Card',
    ...props,
  });
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-review-card', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('leads with the description and the amount', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('.description')?.textContent?.trim()).toBe(
      'ADOBE CREATIVE CLOUD',
    );
    const money = el.shadowRoot?.querySelector('wc-money');
    expect(money?.amount).toBe(-54.99);
    expect(money?.getAttribute('variant')).toBe('signed');
  });

  it('pairs date and account as labelled values', async () => {
    const el = await mount();
    const terms = [...(el.shadowRoot?.querySelectorAll('dt') ?? [])].map((n) =>
      n.textContent?.trim(),
    );
    const values = [...(el.shadowRoot?.querySelectorAll('dd') ?? [])].map((n) =>
      n.textContent?.trim(),
    );
    expect(terms).toEqual(['Date', 'Account']);
    expect(values).toEqual(['2025-03-04', 'BofA Credit Card']);
  });

  it('says nothing about a current category on a freshly flagged transaction', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('.current')).toBeNull();
  });

  it('shows what a re-reviewed transaction already carries', async () => {
    const el = await mount({
      currentCategory: 'Software / Subscriptions',
      currentVendor: 'Adobe Inc',
    });
    const current = el.shadowRoot?.querySelector('.current')?.textContent;
    expect(current).toContain('Software / Subscriptions');
    expect(current).toContain('Adobe Inc');
  });

  it('handles a vendor with no category', async () => {
    const el = await mount({ currentCategory: null, currentVendor: 'Adobe Inc' });
    expect(el.shadowRoot?.querySelector('.current')?.textContent).toContain(
      'uncategorized',
    );
  });
});

describePreviewA11y(preview);
