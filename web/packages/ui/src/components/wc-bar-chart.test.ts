import { describe, it, expect, afterEach } from 'vitest';
import './wc-bar-chart.js';
import { barHeights, type BarBucket, type WcBarChart } from './wc-bar-chart.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-bar-chart.preview.js';

const buckets: BarBucket[] = [
  { label: 'Jan', income: 1000, expense: 500 },
  { label: 'Feb', income: 2000, expense: 250 },
];

async function mount(props: Partial<WcBarChart> = {}): Promise<WcBarChart> {
  const el = document.createElement('wc-bar-chart');
  Object.assign(el, { locale: 'en-US', ...props });
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function bars(el: WcBarChart, kind: 'income' | 'expense'): HTMLElement[] {
  return [...(el.shadowRoot?.querySelectorAll<HTMLElement>(`.bar.${kind}`) ?? [])];
}

describe('barHeights', () => {
  it('scales every bar against the tallest figure in the set', () => {
    expect(barHeights(buckets)).toEqual([
      { income: 50, expense: 25 },
      { income: 100, expense: 12.5 },
    ]);
  });

  it('scales against an expense when the expense is the peak', () => {
    const heights = barHeights([{ label: 'Jan', income: 250, expense: 1000 }]);
    expect(heights).toEqual([{ income: 25, expense: 100 }]);
  });

  it('draws a flat baseline rather than dividing by zero', () => {
    const heights = barHeights([
      { label: 'Jan', income: 0, expense: 0 },
      { label: 'Feb', income: 0, expense: 0 },
    ]);
    expect(heights).toEqual([
      { income: 0, expense: 0 },
      { income: 0, expense: 0 },
    ]);
  });

  it('has nothing to scale for an empty set', () => {
    expect(barHeights([])).toEqual([]);
  });

  it('handles a single bucket', () => {
    const [only] = barHeights([{ label: 'Jan', income: 900, expense: 300 }]);
    expect(only?.income).toBe(100);
    expect(only?.expense).toBeCloseTo(100 / 3, 10);
  });
});

describe('wc-bar-chart', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('draws two bars per bucket', async () => {
    const twelve = Array.from({ length: 12 }, (_, i) => ({
      label: `M${i}`,
      income: 100 + i,
      expense: 50 + i,
    }));
    const el = await mount({ buckets: twelve });
    expect(bars(el, 'income').length).toBe(12);
    expect(bars(el, 'expense').length).toBe(12);
  });

  it('gives the tallest figure the full height', async () => {
    const el = await mount({ buckets });
    expect(bars(el, 'income')[1]?.style.height).toBe('100%');
    expect(bars(el, 'income')[0]?.style.height).toBe('50%');
  });

  it('puts the formatted amount in each bar tooltip', async () => {
    const el = await mount({ buckets });
    expect(bars(el, 'income')[0]?.title).toBe('Jan income $1,000.00');
    expect(bars(el, 'expense')[0]?.title).toBe('Jan expenses $500.00');
  });

  it('labels each bucket under the plot', async () => {
    const el = await mount({ buckets });
    const ticks = [...(el.shadowRoot?.querySelectorAll('.tick') ?? [])];
    expect(ticks.map((t) => t.textContent?.trim())).toEqual(['Jan', 'Feb']);
  });

  it('names itself for assistive tech rather than leaving bars unexplained', async () => {
    const el = await mount({ buckets, caption: '2025 - 26' });
    const label = el.shadowRoot
      ?.querySelector('[role="img"]')
      ?.getAttribute('aria-label');
    expect(label).toContain('2025 - 26');
    expect(label).toContain('$3,000.00');
    expect(label).toContain('$750.00');
  });

  it('repeats the figures as a table only assistive tech reads', async () => {
    const el = await mount({ buckets });
    const rows = [...(el.shadowRoot?.querySelectorAll('table.sr-only tbody tr') ?? [])];
    expect(rows.length).toBe(2);
    expect(rows[0]?.textContent).toContain('$1,000.00');
    expect(rows[0]?.textContent).toContain('$500.00');
  });

  it('renders the caption', async () => {
    const el = await mount({ buckets, caption: '2025 - 26' });
    expect(el.shadowRoot?.querySelector('.caption')?.textContent?.trim()).toBe(
      '2025 - 26',
    );
  });

  it('offers the empty slot rather than an empty plot', async () => {
    const el = await mount({ buckets: [] });
    expect(el.shadowRoot?.querySelector('.plot')).toBeNull();
    expect(el.shadowRoot?.querySelector('slot[name="empty"]')).not.toBeNull();
  });

  it('shows a spinner while loading', async () => {
    const el = await mount({ buckets, loading: true });
    expect(el.shadowRoot?.querySelector('wc-spinner')).not.toBeNull();
    expect(el.shadowRoot?.querySelector('.plot')).toBeNull();
  });

  it('shows the error instead of a stale chart and retries', async () => {
    const el = await mount({ buckets, error: 'boom' });
    expect(el.shadowRoot?.querySelector('.plot')).toBeNull();
    let fired = 0;
    el.addEventListener('nc-retry', () => (fired += 1));
    el.shadowRoot?.querySelector<HTMLButtonElement>('.retry')?.click();
    expect(fired).toBe(1);
  });

  it('hides the legend from assistive tech, which has the table instead', async () => {
    const el = await mount({ buckets });
    expect(el.shadowRoot?.querySelector('.legend')?.getAttribute('aria-hidden')).toBe(
      'true',
    );
  });
});

describePreviewA11y(preview);
