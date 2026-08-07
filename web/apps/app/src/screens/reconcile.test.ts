import { describe, it, expect, afterEach } from 'vitest';
import './reconcile.js';
import type { NigelReconcileScreen } from './reconcile.js';
import type {
  NcToastDetail,
  WcReconcileForm,
  WcReconcileResult,
  WcReconciliationHistory,
} from '@nigel/ui';

import { ApiError } from '../api/index.js';
import { FakeApiClient } from '../__mocks__/fake-api-client.js';
import type { Account, ReconciliationRecord } from '../api/types.js';
import type { ScreenId } from './registry.js';

const ACCOUNTS: Account[] = [
  {
    id: 1,
    name: 'BofA Checking',
    accountType: 'checking',
    institution: 'Bank of America',
    lastFour: '4821',
  },
  {
    id: 2,
    name: 'BofA Credit Card',
    accountType: 'credit_card',
    institution: 'Bank of America',
    lastFour: '9902',
  },
];

const HISTORY: ReconciliationRecord[] = [
  {
    id: 3,
    accountId: 1,
    accountName: 'BofA Checking',
    month: '2025-01',
    statementBalance: 100,
    calculatedBalance: 100,
    isReconciled: true,
    reconciledAt: '2025-02-01 09:00:00',
    notes: null,
  },
];

function client(): FakeApiClient {
  const fake = new FakeApiClient();
  fake.accounts = ACCOUNTS.map((account) => ({ ...account }));
  fake.reconciliations = HISTORY.map((record) => ({ ...record }));
  fake.calculatedBalances = { 'BofA Checking': 4928.01, 'BofA Credit Card': -250 };
  return fake;
}

async function settle(el: NigelReconcileScreen): Promise<void> {
  await el.updateComplete;
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
}

interface Mounted {
  el: NigelReconcileScreen;
  fake: FakeApiClient;
  navigated: { screen: ScreenId; params?: URLSearchParams }[];
}

async function mount(
  fake: FakeApiClient = client(),
  params = new URLSearchParams(),
): Promise<Mounted> {
  const navigated: { screen: ScreenId; params?: URLSearchParams }[] = [];
  const el = document.createElement('nigel-reconcile-screen');
  el.client = fake;
  el.params = params;
  el.navigate = (screen, next) => navigated.push({ screen, params: next });
  document.body.appendChild(el);
  await settle(el);
  return { el, fake, navigated };
}

function form(el: NigelReconcileScreen): WcReconcileForm {
  const found = el.shadowRoot?.querySelector('wc-reconcile-form');
  if (!found) throw new Error('no wc-reconcile-form rendered');
  return found as WcReconcileForm;
}

function result(el: NigelReconcileScreen): WcReconcileResult | null {
  return (el.shadowRoot?.querySelector('wc-reconcile-result') as WcReconcileResult) ?? null;
}

function history(el: NigelReconcileScreen): WcReconciliationHistory {
  const found = el.shadowRoot?.querySelector('wc-reconciliation-history');
  if (!found) throw new Error('no wc-reconciliation-history rendered');
  return found as WcReconciliationHistory;
}

async function submit(
  el: NigelReconcileScreen,
  detail: { account: string; month: string; statementBalance: number },
): Promise<void> {
  form(el).dispatchEvent(
    new CustomEvent('nc-reconcile-submit', { detail, bubbles: true }),
  );
  await settle(el);
}

function toasts(): NcToastDetail[] {
  const seen: NcToastDetail[] = [];
  window.addEventListener('nc-toast', (event) => {
    seen.push((event as CustomEvent<NcToastDetail>).detail);
  });
  return seen;
}

describe('nigel-reconcile-screen', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('loads the accounts and the selected account’s history on enter', async () => {
    const { el, fake } = await mount();

    expect(fake.calls).toContain('getAccounts');
    expect(fake.calls).toContain('getReconciliations:account=BofA+Checking');
    expect(form(el).accounts).toEqual(['BofA Checking', 'BofA Credit Card']);
    expect(form(el).value.account).toBe('BofA Checking');
    expect(history(el).rows.map((row) => row.month)).toEqual(['2025-01']);
  });

  it('opens on a deep-linked account', async () => {
    const { el, fake } = await mount(client(), new URLSearchParams('account=BofA Credit Card'));

    expect(form(el).value.account).toBe('BofA Credit Card');
    expect(fake.calls).toContain('getReconciliations:account=BofA+Credit+Card');
  });

  it('shows the reconciled verdict and refreshes the history', async () => {
    const { el, fake } = await mount();

    await submit(el, {
      account: 'BofA Checking',
      month: '2025-02',
      statementBalance: 4928.01,
    });

    expect(fake.calls).toContain(
      'reconcile:{"account":"BofA Checking","month":"2025-02","statementBalance":4928.01}',
    );
    expect(result(el)?.isReconciled).toBe(true);
    expect(result(el)?.discrepancy).toBe(0);
    expect(result(el)?.month).toBe('2025-02');

    // The server records the attempt, so the new month must appear below.
    expect(history(el).rows.map((row) => row.month)).toEqual(['2025-02', '2025-01']);
  });

  it('keeps the verdict labelled with the month it actually checked', async () => {
    const { el } = await mount();

    await submit(el, {
      account: 'BofA Checking',
      month: '2025-02',
      statementBalance: 4928.01,
    });

    form(el).dispatchEvent(
      new CustomEvent('nc-reconcile-change', {
        detail: {
          value: { account: 'BofA Checking', month: '2025-03', balance: '4928.01' },
        },
        bubbles: true,
      }),
    );
    await settle(el);

    // Editing the month must not relabel February's figures as March's.
    expect(result(el)?.month).toBe('2025-02');
  });

  it('shows a discrepancy with the difference the server calculated', async () => {
    const { el } = await mount();

    await submit(el, {
      account: 'BofA Checking',
      month: '2025-03',
      statementBalance: 5000,
    });

    expect(result(el)?.isReconciled).toBe(false);
    expect(result(el)?.discrepancy).toBeCloseTo(71.99, 2);
    // A mismatch is recorded too — that is the point of the history.
    expect(history(el).rows[0].isReconciled).toBe(false);
  });

  it('puts an empty month under the month field and keeps the typed figures', async () => {
    const fake = client();
    fake.emptyMonths.add('BofA Checking|2025-07');
    const { el } = await mount(fake);

    form(el).dispatchEvent(
      new CustomEvent('nc-reconcile-change', {
        detail: {
          value: { account: 'BofA Checking', month: '2025-07', balance: '4,928.01' },
        },
        bubbles: true,
      }),
    );
    await settle(el);

    await submit(el, {
      account: 'BofA Checking',
      month: '2025-07',
      statementBalance: 4928.01,
    });

    expect(form(el).errors.month).toBe(
      'No transactions for that account in that month.',
    );
    expect(result(el)).toBeNull();
    // Retyping a figure copied off a paper statement is the worst outcome.
    expect(form(el).value.balance).toBe('4,928.01');
  });

  it('puts an unknown account under the account field', async () => {
    const { el } = await mount();

    await submit(el, { account: 'Nope', month: '2025-02', statementBalance: 1 });

    expect(form(el).errors.account).toBe(
      'That account no longer exists. Reload to see the current list.',
    );
    expect(result(el)).toBeNull();
  });

  it('raises a toast for a failed reconcile as well as the inline message', async () => {
    const seen = toasts();
    const fake = client();
    fake.emptyMonths.add('BofA Checking|2025-07');
    const { el } = await mount(fake);

    await submit(el, {
      account: 'BofA Checking',
      month: '2025-07',
      statementBalance: 1,
    });

    expect(seen.at(-1)?.variant).toBe('danger');
    expect(seen.at(-1)?.message).toBe(
      'No transactions for that account in that month.',
    );
  });

  it('clears a stale verdict and reloads history when the account changes', async () => {
    const { el, fake, navigated } = await mount();

    await submit(el, {
      account: 'BofA Checking',
      month: '2025-02',
      statementBalance: 4928.01,
    });
    expect(result(el)).not.toBeNull();

    form(el).dispatchEvent(
      new CustomEvent('nc-reconcile-change', {
        detail: {
          value: { account: 'BofA Credit Card', month: '2025-02', balance: '' },
        },
        bubbles: true,
      }),
    );
    await settle(el);

    expect(result(el)).toBeNull();
    expect(fake.calls).toContain('getReconciliations:account=BofA+Credit+Card');
    expect(navigated.at(-1)?.screen).toBe('reconcile');
    expect(navigated.at(-1)?.params?.get('account')).toBe('BofA Credit Card');
  });

  it('keeps the form when the history fails, and retries just that call', async () => {
    const fake = client();
    fake.reconciliationsError = new ApiError({
      code: 'internal',
      rawCode: 'internal',
      message: 'Database is busy.',
      status: 500,
    });
    const { el } = await mount(fake);

    expect(history(el).error).toBe('Database is busy.');
    // The half that does work is still usable.
    expect(form(el).accounts).toHaveLength(2);

    fake.reconciliationsError = null;
    history(el).dispatchEvent(new CustomEvent('nc-retry', { bubbles: true }));
    await settle(el);

    expect(history(el).error).toBeNull();
    expect(history(el).rows).toHaveLength(1);
  });

  it('offers a retry when the accounts will not load', async () => {
    const fake = client();
    fake.accountsError = new ApiError({
      code: 'internal',
      rawCode: 'internal',
      message: 'Database is busy.',
      status: 500,
    });
    const { el } = await mount(fake);

    const notice = el.shadowRoot?.querySelector('wc-notice-bar');
    expect(notice?.getAttribute('message')).toBe('Database is busy.');

    fake.accountsError = null;
    notice?.dispatchEvent(new CustomEvent('nc-notice-action', { bubbles: true }));
    await settle(el);

    expect(form(el).accounts).toHaveLength(2);
  });

  it('points at the accounts screen when there is nothing to reconcile', async () => {
    const fake = new FakeApiClient();
    const { el } = await mount(fake);

    const empty = el.shadowRoot?.querySelector('wc-empty-state');
    expect(empty?.getAttribute('message')).toBe('No accounts found. Add one first.');
    expect(el.shadowRoot?.querySelector('a[href="#/accounts"]')).not.toBeNull();
  });
});
