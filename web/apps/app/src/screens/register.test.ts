import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import './register.js';
import type { NigelRegisterScreen } from './register.js';
import type { WcRegisterTable } from '@nigel/ui';
import { ApiError, appLocked } from '../api/index.js';
import { resetAppStore } from '../state/app-store.js';
import { FakeApiClient } from '../__mocks__/fake-api-client.js';
import type { Account, CategoryRow, RegisterRow } from '../api/types.js';
import { todayIso } from './register-data.js';

const accounts: Account[] = [
  { id: 1, name: 'BofA Checking', accountType: 'checking', institution: null, lastFour: null },
  {
    id: 2,
    name: 'BofA Credit Card',
    accountType: 'credit_card',
    institution: null,
    lastFour: null,
  },
];

const categories: CategoryRow[] = [
  { id: 3, name: 'Consulting income', categoryType: 'income', taxLine: null, formLine: null },
  {
    id: 12,
    name: 'Software / Subscriptions',
    categoryType: 'expense',
    taxLine: null,
    formLine: null,
  },
];

function row(overrides: Partial<RegisterRow> = {}): RegisterRow {
  return {
    id: 1,
    date: '2025-03-04',
    description: 'ADOBE CREATIVE CLOUD',
    amount: -54.99,
    category: 'Software / Subscriptions',
    categoryId: 12,
    vendor: 'Adobe Inc',
    accountName: 'BofA Credit Card',
    isFlagged: false,
    ...overrides,
  };
}

/** Two in the past, one today, one in the future — spans `scroll_to_today`. */
function spanningToday(): RegisterRow[] {
  const today = todayIso();
  return [
    row({ id: 1, date: '2020-01-01', description: 'OLDEST' }),
    row({ id: 2, date: '2020-06-01', description: 'OLDER' }),
    row({ id: 3, date: today, description: 'TODAY EARLIER' }),
    row({ id: 4, date: today, description: 'TODAY LATER' }),
    row({ id: 5, date: '2099-01-01', description: 'FUTURE' }),
  ];
}

function client(rows: RegisterRow[] = spanningToday()): FakeApiClient {
  const fake = new FakeApiClient();
  fake.register = { rows, total: rows.reduce((sum, r) => sum + r.amount, 0) };
  fake.accounts = accounts;
  fake.categories = categories;
  return fake;
}

const navigations: { screen: string; params: string }[] = [];

async function mount(
  fake: FakeApiClient = client(),
  query = '',
): Promise<{ el: NigelRegisterScreen; fake: FakeApiClient }> {
  navigations.length = 0;

  const el = document.createElement('nigel-register-screen');
  el.client = fake;
  el.params = new URLSearchParams(query);
  el.navigate = (screen, params) =>
    navigations.push({ screen, params: params?.toString() ?? '' });
  document.body.appendChild(el);
  await settle(el);
  return { el, fake };
}

async function settle(el: NigelRegisterScreen): Promise<void> {
  await el.updateComplete;
  await new Promise((r) => setTimeout(r, 0));
  await el.updateComplete;
  await new Promise((r) => setTimeout(r, 0));
  await el.updateComplete;
}

function table(el: NigelRegisterScreen): WcRegisterTable {
  const found = el.shadowRoot?.querySelector('wc-register-table');
  if (!found) throw new Error('no register table rendered');
  return found;
}

function search(el: NigelRegisterScreen, query: string): void {
  el.shadowRoot
    ?.querySelector('wc-register-toolbar')
    ?.dispatchEvent(
      new CustomEvent('nc-search-change', {
        detail: { query },
        bubbles: true,
        composed: true,
      }),
    );
}

function emitOnTable(el: NigelRegisterScreen, type: string, detail: unknown): void {
  table(el).dispatchEvent(
    new CustomEvent(type, { detail, bubbles: true, composed: true }),
  );
}

function registerCalls(fake: FakeApiClient): string[] {
  return fake.calls.filter((call) => call.startsWith('getRegister'));
}

describe('register screen', () => {
  beforeEach(() => {
    resetAppStore();
    appLocked.set(false);
  });

  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  // -- loading ---------------------------------------------------------------

  it('loads the register, the accounts and the chart of accounts together', async () => {
    const { fake } = await mount();
    expect(fake.calls).toEqual(['getRegister:', 'getAccounts', 'getCategories']);
  });

  it('renders every row with the report total', async () => {
    const { el } = await mount();
    expect(table(el).rows.length).toBe(5);
    expect(table(el).total).toBeCloseTo(-274.95, 2);
  });

  it('shows the failure and offers a retry rather than an empty table', async () => {
    const fake = client();
    fake.registerError = new ApiError({
      code: 'internal',
      rawCode: 'internal',
      message: 'database is busy',
      status: 500,
    });
    const el = document.createElement('nigel-register-screen');
    el.client = fake;
    el.params = new URLSearchParams();
    document.body.appendChild(el);
    await settle(el);

    expect(el.shadowRoot?.querySelector('wc-register-table')).toBeNull();
    expect(el.shadowRoot?.querySelector('wc-empty-state')?.getAttribute('message')).toBe(
      'database is busy',
    );
  });

  // -- scroll to today -------------------------------------------------------

  it('opens on the last row dated today, as the TUI does', async () => {
    const { el } = await mount();
    expect(table(el).selectedId).toBe(4);
  });

  it('opens on the last row when every transaction is in the past', async () => {
    const rows = [row({ id: 1, date: '2020-01-01' }), row({ id: 2, date: '2020-02-01' })];
    const { el } = await mount(client(rows));
    expect(table(el).selectedId).toBe(2);
  });

  it('opens at the top when every transaction is in the future', async () => {
    const rows = [row({ id: 1, date: '2099-01-01' }), row({ id: 2, date: '2099-02-01' })];
    const { el } = await mount(client(rows));
    expect(table(el).selectedId).toBe(1);
  });

  it('stays at the top of a dated register, which has no today to find', async () => {
    const { el } = await mount(client(), 'month=2025-03');
    expect(table(el).selectedId).toBeNull();
  });

  it('opens on a linked transaction instead, so review and the dashboard can link in', async () => {
    const { el } = await mount(client(), 'id=2');
    expect(table(el).selectedId).toBe(2);
  });

  // -- search ----------------------------------------------------------------

  it('filters the table live and counts the matches', async () => {
    const { el } = await mount();
    search(el, 'today');
    await settle(el);

    expect(table(el).rows.map((r) => r.id)).toEqual([3, 4]);
    expect(table(el).getAttribute('footer-note')).toBe('2 of 5 rows shown');
  });

  it('keeps the footer total on the whole result set, not the filtered rows', async () => {
    const { el } = await mount();
    const before = table(el).total;
    search(el, 'today');
    await settle(el);
    expect(table(el).total).toBe(before);
  });

  it('searches without going back to the server', async () => {
    const { el, fake } = await mount();
    search(el, 'adobe');
    await settle(el);
    expect(registerCalls(fake).length).toBe(1);
  });

  it('reads a search out of the route, so a search is a link', async () => {
    const { el } = await mount(client(), 'q=future');
    expect(table(el).rows.map((r) => r.id)).toEqual([5]);
  });

  // -- routing ---------------------------------------------------------------

  it('navigates rather than refetching when the account changes', async () => {
    const { el, fake } = await mount();
    el.shadowRoot
      ?.querySelector('wc-register-toolbar')
      ?.dispatchEvent(
        new CustomEvent('nc-account-change', {
          detail: { account: 'BofA Checking' },
          bubbles: true,
          composed: true,
        }),
      );
    await settle(el);

    expect(navigations).toEqual([
      { screen: 'register', params: 'account=BofA+Checking' },
    ]);
    expect(registerCalls(fake).length).toBe(1);
  });

  it('replaces the date filter rather than stacking two of them', async () => {
    const { el } = await mount(client(), 'year=2024&account=BofA Checking');
    el.shadowRoot
      ?.querySelector('wc-register-toolbar')
      ?.dispatchEvent(
        new CustomEvent('nc-period-change', {
          detail: { period: { kind: 'month', year: 2025, month: 3 } },
          bubbles: true,
          composed: true,
        }),
      );
    await settle(el);

    expect(navigations[0]?.params).toBe('account=BofA+Checking&month=2025-03');
  });

  it('fetches again only when the request itself changed', async () => {
    const { el, fake } = await mount();

    el.params = new URLSearchParams('q=adobe');
    await settle(el);
    expect(registerCalls(fake).length).toBe(1);

    el.params = new URLSearchParams('account=BofA Checking');
    await settle(el);
    expect(registerCalls(fake)).toEqual([
      'getRegister:',
      'getRegister:account=BofA+Checking',
    ]);
  });

  it('sends a deep link straight through to the request', async () => {
    const { fake } = await mount(client(), 'month=2025-03&account=BofA Checking');
    expect(registerCalls(fake)).toEqual([
      'getRegister:month=2025-03&account=BofA+Checking',
    ]);
  });

  // -- editing ---------------------------------------------------------------

  it('sends only the fields an edit changed', async () => {
    const { el, fake } = await mount();
    emitOnTable(el, 'nc-edit-commit', { id: 1, categoryId: 3, vendor: 'Adobe Inc' });
    await settle(el);

    expect(fake.calls).toContain('patchTransaction:1:{"categoryId":3}');
  });

  it('clears a vendor with an explicit null', async () => {
    const { el, fake } = await mount();
    emitOnTable(el, 'nc-edit-commit', { id: 1, categoryId: 12, vendor: null });
    await settle(el);

    expect(fake.calls).toContain('patchTransaction:1:{"vendor":null}');
  });

  it('sends nothing when an edit changed nothing', async () => {
    const { el, fake } = await mount();
    emitOnTable(el, 'nc-edit-commit', { id: 1, categoryId: 12, vendor: 'Adobe Inc' });
    await settle(el);

    expect(fake.calls.some((call) => call.startsWith('patchTransaction'))).toBe(false);
  });

  it('swaps in the row the server answered with', async () => {
    const { el } = await mount();
    emitOnTable(el, 'nc-edit-commit', { id: 1, categoryId: 3, vendor: 'Adobe Inc' });
    await settle(el);

    const updated = table(el).rows.find((r) => r.id === 1);
    expect(updated?.categoryId).toBe(3);
    expect(updated?.category).toBe('Consulting income');
  });

  it('sends the flag as a state, both ways', async () => {
    const { el, fake } = await mount();
    emitOnTable(el, 'nc-flag-toggle', { id: 1, flag: true });
    await settle(el);
    emitOnTable(el, 'nc-flag-toggle', { id: 1, flag: false });
    await settle(el);

    expect(fake.calls.filter((c) => c.startsWith('patchTransaction'))).toEqual([
      'patchTransaction:1:{"flag":true}',
      'patchTransaction:1:{"flag":false}',
    ]);
  });

  it('puts the row back and says so when the write fails', async () => {
    const { el, fake } = await mount();
    const toasts: string[] = [];
    window.addEventListener('nc-toast', (event) =>
      toasts.push((event as CustomEvent<{ message: string }>).detail.message),
    );

    fake.patchError = new ApiError({
      code: 'not_found',
      rawCode: 'not_found',
      message: 'no category 999',
      status: 404,
    });
    emitOnTable(el, 'nc-edit-commit', { id: 1, categoryId: 3, vendor: 'Adobe Inc' });
    await settle(el);

    const unchanged = table(el).rows.find((r) => r.id === 1);
    expect(unchanged?.categoryId).toBe(12);
    expect(unchanged?.category).toBe('Software / Subscriptions');
    expect(toasts).toEqual(['no category 999']);
  });

  it('opens an edit only when the table asks for one', async () => {
    const { el } = await mount();
    expect(table(el).editingId).toBeNull();

    emitOnTable(el, 'nc-row-activate', { id: 3 });
    await settle(el);
    expect(table(el).editingId).toBe(3);

    emitOnTable(el, 'nc-edit-cancel', { id: 3 });
    await settle(el);
    expect(table(el).editingId).toBeNull();
  });

  it('hides the account column when the register is filtered to one account', async () => {
    const { el } = await mount(client(), 'account=BofA Checking');
    expect(table(el).showAccount).toBe(false);
  });
});
