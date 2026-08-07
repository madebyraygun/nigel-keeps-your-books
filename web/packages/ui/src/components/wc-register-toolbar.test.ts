import { describe, it, expect, afterEach } from 'vitest';
import './wc-register-toolbar.js';
import type {
  AccountOption,
  NcAccountChangeDetail,
  NcSearchChangeDetail,
  WcRegisterToolbar,
} from './wc-register-toolbar.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-register-toolbar.preview.js';

const accounts: AccountOption[] = [
  { id: 1, name: 'BofA Checking' },
  { id: 2, name: 'BofA Credit Card' },
];

async function mount(
  props: Partial<WcRegisterToolbar> = {},
): Promise<WcRegisterToolbar> {
  const el = document.createElement('wc-register-toolbar');
  Object.assign(el, { accounts, ...props });
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function statusText(el: WcRegisterToolbar): string {
  return el.shadowRoot?.querySelector('.status')?.textContent?.trim() ?? '';
}

describe('wc-register-toolbar', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('offers every account plus an all-accounts choice', async () => {
    const el = await mount();
    const options = [...(el.shadowRoot?.querySelectorAll('wa-option') ?? [])];
    expect(options.map((o) => o.textContent?.trim())).toEqual([
      'All accounts',
      'BofA Checking',
      'BofA Credit Card',
    ]);
  });

  it('reports a chosen account by name', async () => {
    const el = await mount();
    const seen: NcAccountChangeDetail[] = [];
    el.addEventListener('nc-account-change', (event) => seen.push(event.detail));

    const select = el.shadowRoot?.querySelector<HTMLElement & { value: string }>(
      'wa-select',
    );
    if (!select) throw new Error('no account select');
    select.value = 'BofA Checking';
    select.dispatchEvent(new Event('change', { bubbles: true }));

    expect(seen).toEqual([{ account: 'BofA Checking' }]);
  });

  it('reports all-accounts as null rather than as a sentinel', async () => {
    const el = await mount({ account: 'BofA Checking' });
    const seen: NcAccountChangeDetail[] = [];
    el.addEventListener('nc-account-change', (event) => seen.push(event.detail));

    const select = el.shadowRoot?.querySelector<HTMLElement & { value: string }>(
      'wa-select',
    );
    if (!select) throw new Error('no account select');
    select.value = '__all__';
    select.dispatchEvent(new Event('change', { bubbles: true }));

    expect(seen).toEqual([{ account: null }]);
  });

  it('emits a search on every keystroke, with no debounce', async () => {
    const el = await mount();
    const seen: NcSearchChangeDetail[] = [];
    el.addEventListener('nc-search-change', (event) => seen.push(event.detail));

    const input = el.shadowRoot?.querySelector<HTMLElement & { value: string }>(
      '.search',
    );
    if (!input) throw new Error('no search input');

    for (const value of ['a', 'ad', 'ado']) {
      input.value = value;
      input.dispatchEvent(new Event('input', { bubbles: true }));
    }

    expect(seen.map((d) => d.query)).toEqual(['a', 'ad', 'ado']);
  });

  it('counts rows, matches and the empty result differently', async () => {
    expect(statusText(await mount({ totalCount: 480 }))).toBe('480 rows');
    expect(statusText(await mount({ totalCount: 1 }))).toBe('1 row');
    expect(statusText(await mount({ totalCount: 480, matchCount: 7 }))).toBe(
      '7 of 480 rows',
    );
    expect(statusText(await mount({ totalCount: 480, matchCount: 0 }))).toBe(
      'No matches',
    );
  });

  it('announces the count politely, so a search result is not silent', async () => {
    const el = await mount({ totalCount: 3 });
    const status = el.shadowRoot?.querySelector('.status');
    expect(status?.getAttribute('role')).toBe('status');
    expect(status?.getAttribute('aria-live')).toBe('polite');
  });

  it('hands focus to the search box on request', async () => {
    const el = await mount();
    let focused = 0;
    const input = el.shadowRoot?.querySelector('.search');
    input?.addEventListener('focus', () => (focused += 1));
    el.focusSearch();
    expect(focused).toBe(1);
  });

  it('lets the register be unfiltered by date and hides the pager for a dateless route', async () => {
    const withDates = await mount();
    expect(withDates.shadowRoot?.querySelector('wc-period-nav')).not.toBeNull();

    const withoutDates = await mount({ granularity: 'none' });
    expect(withoutDates.shadowRoot?.querySelector('wc-period-nav')).toBeNull();
  });
});

describePreviewA11y(preview);
