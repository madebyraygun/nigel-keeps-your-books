import { describe, it, expect, afterEach } from 'vitest';
import './wc-period-nav.js';
import {
  paramsToPeriod,
  periodLabel,
  periodToParams,
  stepPeriod,
  type NcPeriod,
  type WcPeriodNav,
} from './wc-period-nav.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-period-nav.preview.js';

async function mount(props: Partial<WcPeriodNav> = {}): Promise<WcPeriodNav> {
  const el = document.createElement('wc-period-nav');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function buttons(el: WcPeriodNav, selector: string): HTMLButtonElement[] {
  return [...(el.shadowRoot?.querySelectorAll<HTMLButtonElement>(selector) ?? [])];
}

function kindButton(el: WcPeriodNav, label: string): HTMLButtonElement {
  const found = buttons(el, '.kinds button').find(
    (b) => b.textContent?.trim() === label,
  );
  if (!found) throw new Error(`no "${label}" granularity button`);
  return found;
}

async function periodFrom(
  el: WcPeriodNav,
  act: () => void,
): Promise<NcPeriod | null> {
  let seen: NcPeriod | null = null;
  el.addEventListener('nc-period-change', (event) => {
    seen = event.detail.period;
  });
  act();
  await el.updateComplete;
  return seen;
}

describe('stepPeriod', () => {
  it('pages a year', () => {
    expect(stepPeriod({ kind: 'year', year: 2025 }, 1)).toEqual({
      kind: 'year',
      year: 2026,
    });
    expect(stepPeriod({ kind: 'year', year: 2025 }, -1)).toEqual({
      kind: 'year',
      year: 2024,
    });
  });

  it('rolls a month over the year boundary in both directions', () => {
    expect(stepPeriod({ kind: 'month', year: 2025, month: 12 }, 1)).toEqual({
      kind: 'month',
      year: 2026,
      month: 1,
    });
    expect(stepPeriod({ kind: 'month', year: 2025, month: 1 }, -1)).toEqual({
      kind: 'month',
      year: 2024,
      month: 12,
    });
  });

  it('leaves the unfiltered period alone', () => {
    expect(stepPeriod({ kind: 'all' }, 1)).toEqual({ kind: 'all' });
  });
});

describe('periodLabel', () => {
  it('names each kind', () => {
    expect(periodLabel({ kind: 'all' })).toBe('All transactions');
    expect(periodLabel({ kind: 'year', year: 2025 })).toBe('2025');
    expect(periodLabel({ kind: 'month', year: 2025, month: 3 }, 'en-US')).toBe(
      'March 2025',
    );
  });
});

describe('periodToParams', () => {
  it('sends nothing for the unfiltered period', () => {
    expect(periodToParams({ kind: 'all' })).toEqual({});
  });

  it('sends year as a number', () => {
    expect(periodToParams({ kind: 'year', year: 2025 })).toEqual({ year: 2025 });
  });

  it('sends a month alone, zero padded, because month fixes its own year', () => {
    expect(periodToParams({ kind: 'month', year: 2025, month: 3 })).toEqual({
      month: '2025-03',
    });
  });
});

describe('paramsToPeriod', () => {
  it('reads a month', () => {
    expect(paramsToPeriod(new URLSearchParams('month=2025-03'))).toEqual({
      kind: 'month',
      year: 2025,
      month: 3,
    });
  });

  it('reads a year', () => {
    expect(paramsToPeriod(new URLSearchParams('year=2025'))).toEqual({
      kind: 'year',
      year: 2025,
    });
  });

  it('prefers a valid month over a year, matching the wire form it emits', () => {
    expect(paramsToPeriod(new URLSearchParams('year=2024&month=2025-03'))).toEqual({
      kind: 'month',
      year: 2025,
      month: 3,
    });
  });

  it.each(['month=2025-3', 'month=2025-13', 'year=nope', 'account=BofA'])(
    'falls back to unfiltered for %s',
    (query) => {
      expect(paramsToPeriod(new URLSearchParams(query))).toEqual({ kind: 'all' });
    },
  );
});

describe('wc-period-nav', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('labels the current period', async () => {
    const el = await mount({ period: { kind: 'month', year: 2025, month: 3 }, locale: 'en-US' });
    expect(el.shadowRoot?.querySelector('.label')?.textContent?.trim()).toBe(
      'March 2025',
    );
  });

  it('pages forward and back', async () => {
    const el = await mount({ period: { kind: 'year', year: 2025 } });
    const [prev, next] = buttons(el, '.pager button');

    expect(await periodFrom(el, () => next?.click())).toEqual({
      kind: 'year',
      year: 2026,
    });
    expect(await periodFrom(el, () => prev?.click())).toEqual({
      kind: 'year',
      year: 2024,
    });
  });

  it('disables paging while unfiltered, because there is nothing to page', async () => {
    const el = await mount({ period: { kind: 'all' }, allowAll: true });
    expect(buttons(el, '.pager button').every((b) => b.disabled)).toBe(true);
  });

  it('offers all three kinds only when allow-all is set', async () => {
    const withAll = await mount({ allowAll: true, period: { kind: 'all' } });
    expect(buttons(withAll, '.kinds button').map((b) => b.textContent?.trim())).toEqual(
      ['All', 'Year', 'Month'],
    );

    const withoutAll = await mount({ period: { kind: 'year', year: 2025 } });
    expect(
      buttons(withoutAll, '.kinds button').map((b) => b.textContent?.trim()),
    ).toEqual(['Year', 'Month']);
  });

  it('drops the month control and the whole switch when only years are supported', async () => {
    const el = await mount({
      granularity: 'yearOnly',
      period: { kind: 'year', year: 2025 },
    });
    expect(el.shadowRoot?.querySelector('.kinds')).toBeNull();
    expect(el.shadowRoot?.querySelector('.pager')).not.toBeNull();
  });

  it('renders nothing at all when the route takes no date', async () => {
    const el = await mount({ granularity: 'none' });
    expect(el.shadowRoot?.querySelector('.pager')).toBeNull();
  });

  it('keeps the year when switching a month to a year', async () => {
    const el = await mount({ period: { kind: 'month', year: 2019, month: 7 } });
    expect(await periodFrom(el, () => kindButton(el, 'Year').click())).toEqual({
      kind: 'year',
      year: 2019,
    });
  });

  it('starts a past year in January rather than in a month it never showed', async () => {
    const el = await mount({ period: { kind: 'year', year: 2019 } });
    expect(await periodFrom(el, () => kindButton(el, 'Month').click())).toEqual({
      kind: 'month',
      year: 2019,
      month: 1,
    });
  });

  it('seeds the current month when switching the current year to months', async () => {
    const now = new Date();
    const el = await mount({ period: { kind: 'year', year: now.getFullYear() } });
    expect(await periodFrom(el, () => kindButton(el, 'Month').click())).toEqual({
      kind: 'month',
      year: now.getFullYear(),
      month: now.getMonth() + 1,
    });
  });

  it('clears to unfiltered', async () => {
    const el = await mount({
      allowAll: true,
      period: { kind: 'month', year: 2025, month: 3 },
    });
    expect(await periodFrom(el, () => kindButton(el, 'All').click())).toEqual({
      kind: 'all',
    });
  });

  it('moves between kinds with the arrow keys, as a radiogroup should', async () => {
    const el = await mount({ period: { kind: 'year', year: 2025 } });
    const group = el.shadowRoot?.querySelector('.kinds');

    const seen = await periodFrom(el, () =>
      group?.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }),
      ),
    );
    expect(seen).toEqual({ kind: 'month', year: 2025, month: 1 });
  });

  it('marks exactly one kind checked and keeps the rest out of the tab order', async () => {
    const el = await mount({
      allowAll: true,
      period: { kind: 'year', year: 2025 },
    });
    const kinds = buttons(el, '.kinds button');
    expect(kinds.filter((b) => b.getAttribute('aria-checked') === 'true').length).toBe(1);
    expect(kinds.filter((b) => b.tabIndex === 0).length).toBe(1);
  });

  it('emits nothing when the current kind is clicked again', async () => {
    const el = await mount({ period: { kind: 'year', year: 2025 } });
    expect(await periodFrom(el, () => kindButton(el, 'Year').click())).toBeNull();
  });
});

describePreviewA11y(preview);
