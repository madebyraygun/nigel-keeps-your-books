import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest';
import '../components/nigel-app.js';
import type { NigelApp } from '../components/nigel-app.js';
import { appLocked, appUnauthorized } from '../api/index.js';
import { resetAppStore } from '../state/app-store.js';
import { FakeApiClient } from '../__mocks__/fake-api-client.js';
import type { ImportListItem, RegisterRow } from '../api/types.js';

const IMPORTS: ImportListItem[] = [
  {
    id: 12,
    filename: 'march-checking.csv',
    accountName: 'BofA Checking',
    importDate: '2025-04-02 09:14:11',
    transactionCount: 42,
  },
];

const ROWS: RegisterRow[] = [
  {
    id: 501,
    date: '2025-03-04',
    description: 'ADOBE CREATIVE CLOUD',
    amount: -59.99,
    category: 'Software / Subscriptions',
    categoryId: 12,
    vendor: 'Adobe',
    accountName: 'BofA Checking',
    isFlagged: false,
  },
];

async function settle(el: NigelApp): Promise<void> {
  await el.updateComplete;
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
}

async function mount(client: FakeApiClient): Promise<NigelApp> {
  const el = document.createElement('nigel-app');
  el.client = client;
  document.body.appendChild(el);
  await settle(el);
  return el;
}

async function goTo(el: NigelApp, hash: string): Promise<void> {
  window.location.hash = hash;
  window.dispatchEvent(new HashChangeEvent('hashchange'));
  await settle(el);
}

function client(): FakeApiClient {
  const fake = new FakeApiClient();
  fake.imports = IMPORTS.map((item) => ({ ...item }));
  fake.register = { rows: ROWS.map((row) => ({ ...row })), total: -59.99 };
  return fake;
}

/**
 * The freshness property the whole app leans on.
 *
 * There is no global cache and each screen fetches in `firstUpdated`, so
 * arriving at a screen is what makes it current. That only holds while every
 * screen is a distinct element that Lit tears down on a route change — put two
 * screens behind one tag and Lit would reuse the instance, `firstUpdated` would
 * not run again, and a register would quietly show transactions that an undo
 * had already deleted. This test is the guard on that.
 */
describe('screens are refetched on arrival', () => {
  beforeEach(() => {
    resetAppStore();
    appLocked.set(false);
    appUnauthorized.set(false);
    window.location.hash = '';
  });

  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it('reloads the register after an import is undone', async () => {
    const ui = await import('@nigel/ui');
    vi.spyOn(ui, 'confirmDialog').mockResolvedValue(true);

    const fake = client();
    const el = await mount(fake);

    // Look at the register first, so a stale cache would have something to
    // serve on the way back.
    await goTo(el, '#/register');
    expect(fake.calls.filter((call) => call.startsWith('getRegister'))).toHaveLength(1);

    await goTo(el, '#/undo');
    const screen = el.shadowRoot?.querySelector('nigel-undo-screen');
    screen?.shadowRoot
      ?.querySelector('wc-import-history')
      ?.dispatchEvent(
        new CustomEvent('nc-import-undo', { detail: { id: 12 }, bubbles: true }),
      );
    await settle(el);

    expect(fake.calls).toContain('deleteImport:12');

    await goTo(el, '#/register');

    const undoAt = fake.calls.indexOf('deleteImport:12');
    const registerCalls = fake.calls
      .map((call, index) => ({ call, index }))
      .filter((entry) => entry.call.startsWith('getRegister'));

    expect(registerCalls).toHaveLength(2);
    expect(registerCalls[1].index).toBeGreaterThan(undoAt);
  });

  it('reloads the dashboard after an import is undone', async () => {
    const ui = await import('@nigel/ui');
    vi.spyOn(ui, 'confirmDialog').mockResolvedValue(true);

    const fake = client();
    const el = await mount(fake);
    await goTo(el, '#/dashboard');
    const before = fake.calls.filter((call) => call.startsWith('getPnl')).length;

    await goTo(el, '#/undo');
    el.shadowRoot
      ?.querySelector('nigel-undo-screen')
      ?.shadowRoot?.querySelector('wc-import-history')
      ?.dispatchEvent(
        new CustomEvent('nc-import-undo', { detail: { id: 12 }, bubbles: true }),
      );
    await settle(el);

    await goTo(el, '#/dashboard');

    expect(fake.calls.filter((call) => call.startsWith('getPnl')).length).toBe(
      before + 1,
    );
  });
});
