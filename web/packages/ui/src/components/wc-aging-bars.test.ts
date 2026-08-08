import { describe, it, expect, afterEach } from 'vitest';
import './wc-aging-bars.js';
import {
  agingBarHeights,
  type AgingBucketView,
  type WcAgingBars,
} from './wc-aging-bars.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-aging-bars.preview.js';

const BUCKETS: AgingBucketView[] = [
  { label: 'current', count: 2, total: 4200 },
  { label: '1-30', count: 1, total: 1850 },
  { label: '31-60', count: 0, total: 0 },
  { label: '61-90', count: 1, total: 960 },
  { label: '90+', count: 0, total: 0 },
];

async function mount(props: Partial<WcAgingBars> = {}): Promise<WcAgingBars> {
  const el = document.createElement('wc-aging-bars');
  Object.assign(el, { buckets: BUCKETS, total: 7010, asOf: '2026-08-07' }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('agingBarHeights', () => {
  it('scales against the largest bucket, not the total', () => {
    expect(agingBarHeights(BUCKETS)).toEqual([100, 44, 0, 23, 0]);
  });

  it('draws nothing when every bucket is zero', () => {
    expect(agingBarHeights(BUCKETS.map((b) => ({ ...b, total: 0 })))).toEqual([
      0, 0, 0, 0, 0,
    ]);
  });
});

describe('wc-aging-bars', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('puts every figure in a table, with the total last', async () => {
    const el = await mount();
    const headers = [...(el.shadowRoot?.querySelectorAll('th') ?? [])].map((th) =>
      th.textContent?.trim(),
    );
    expect(headers).toEqual(['current', '1-30', '31-60', '61-90', '90+', 'Total']);

    const amounts = [...(el.shadowRoot?.querySelectorAll('wc-money') ?? [])].map(
      (money) => (money as HTMLElement & { amount: number }).amount,
    );
    expect(amounts).toEqual([4200, 1850, 0, 960, 0, 7010]);
  });

  it('hides the bars from assistive tech — the table carries the values', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('.bars')?.getAttribute('aria-hidden')).toBe(
      'true',
    );
    expect(el.shadowRoot?.querySelectorAll('.bar')).toHaveLength(5);
  });

  it('links to the full report only when given somewhere to go', async () => {
    const bare = await mount();
    expect(bare.shadowRoot?.querySelector('a')).toBeNull();

    const linked = await mount({ href: '#/reports?report=aging' });
    expect(linked.shadowRoot?.querySelector('a')?.getAttribute('href')).toBe(
      '#/reports?report=aging',
    );
  });

  it('says so rather than drawing an empty frame when there are no buckets', async () => {
    const el = await mount({ buckets: [] });
    expect(el.shadowRoot?.querySelector('.empty')?.textContent).toContain(
      'No open receivables',
    );
  });
});

describePreviewA11y(preview);
