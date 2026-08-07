import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import {
  initializeDashboardStore,
  getDashboardStore,
  resetDashboardStore,
  type DashboardStore,
} from './dashboard-store.js';
import { FakeApiClient } from '../__mocks__/fake-api-client.js';
import { ApiError } from '../api/index.js';
import type { BalanceReport, CashflowReport, PnlReport } from '../api/types.js';

const PNL: PnlReport = {
  income: [{ name: 'Consulting', total: 184320.5 }],
  expenses: [{ name: 'Rent', total: -24000 }],
  totalIncome: 184320.5,
  totalExpenses: -96540.18,
  net: 87780.32,
};

const BALANCE: BalanceReport = {
  accounts: [{ name: 'BofA Checking', accountType: 'checking', balance: 4928.01 }],
  total: 4928.01,
  ytdNetIncome: 87780.32,
};

const CASHFLOW: CashflowReport = {
  months: [
    { month: '2026-01', inflows: 5000, outflows: -3200, net: 1800, runningBalance: 1800 },
  ],
};

let client: FakeApiClient;
let store: DashboardStore;

beforeEach(() => {
  resetDashboardStore();
  client = new FakeApiClient();
  client.pnl = PNL;
  client.balance = BALANCE;
  client.cashflow = CASHFLOW;
  client.flagged = [
    { id: 1, date: '2026-01-04', description: 'UNKNOWN', amount: -12, accountName: 'BofA Checking' },
    { id: 2, date: '2026-01-05', description: 'ALSO UNKNOWN', amount: -30, accountName: 'BofA Checking' },
  ];
  store = initializeDashboardStore(client);
});

afterEach(() => {
  resetDashboardStore();
});

describe('dashboard store', () => {
  it('fetches all four figures exactly once per load', async () => {
    await store.load();

    expect(client.calls.filter((c) => c.startsWith('getPnl')).length).toBe(1);
    expect(client.calls.filter((c) => c === 'getBalance').length).toBe(1);
    expect(client.calls.filter((c) => c.startsWith('getCashflow')).length).toBe(1);
    expect(client.calls.filter((c) => c === 'getFlagged').length).toBe(1);
  });

  it('populates every slot', async () => {
    await store.load();

    expect(store.pnl.data.get()).toEqual(PNL);
    expect(store.balance.data.get()).toEqual(BALANCE);
    expect(store.cashflow.data.get()).toEqual(CASHFLOW);
    expect(store.flagged.data.get()?.length).toBe(2);
  });

  it('asks for the current year of profit and loss', async () => {
    await store.load();
    expect(client.calls).toContain(`getPnl:${new Date().getFullYear()}`);
  });

  it('asks for cash flow unfiltered, so the window can cross a year', async () => {
    await store.load();
    // No year: the last twelve months of data, windowed client-side.
    expect(client.calls).toContain('getCashflow:');
  });

  it('re-fetches everything on a second load', async () => {
    await store.load();
    await store.load();
    expect(client.calls.filter((c) => c === 'getBalance').length).toBe(2);
  });

  it('keeps one failure from blanking the other three', async () => {
    client.balanceError = new ApiError({
      code: 'internal',
      rawCode: 'internal',
      message: 'Balance blew up.',
      status: 500,
    });

    await store.load();

    expect(store.balance.error.get()?.message).toBe('Balance blew up.');
    expect(store.balance.data.get()).toBeNull();
    expect(store.pnl.data.get()).toEqual(PNL);
    expect(store.cashflow.data.get()).toEqual(CASHFLOW);
    expect(store.flagged.data.get()?.length).toBe(2);
    expect(store.pnl.error.get()).toBeNull();
  });

  it('never rejects, whatever the transport does', async () => {
    client.pnlError = new Error('network gone');
    client.balanceError = new Error('network gone');
    client.cashflowError = new Error('network gone');
    client.flaggedError = new Error('network gone');

    await expect(store.load()).resolves.toBeUndefined();
    expect(store.pnl.error.get()?.message).toBe('Error: network gone');
  });

  it('clears a previous error once the retry succeeds', async () => {
    client.balanceError = new Error('down');
    await store.load();
    expect(store.balance.error.get()).not.toBeNull();

    client.balanceError = null;
    await store.reloadBalance();

    expect(store.balance.error.get()).toBeNull();
    expect(store.balance.data.get()).toEqual(BALANCE);
  });

  it('retries only the endpoint that failed', async () => {
    await store.load();
    const before = client.calls.length;

    await store.reloadBalance();

    expect(client.calls.length).toBe(before + 1);
    expect(client.calls[client.calls.length - 1]).toBe('getBalance');
  });

  it('ignores a fetch that is overtaken by a newer one', async () => {
    const pending: Array<() => void> = [];
    const stale: BalanceReport = { accounts: [], total: 1, ytdNetIncome: 1 };
    let answer = stale;
    client.getBalance = () => {
      const report = answer;
      return new Promise((resolve) => {
        pending.push(() => resolve({ granularity: 'none', report }));
      });
    };

    const first = store.reloadBalance();
    answer = BALANCE;
    const second = store.reloadBalance();

    // The second fetch answers first, and the first one lands afterwards.
    pending[1]?.();
    pending[0]?.();
    await Promise.all([first, second]);

    expect(store.balance.data.get()).toEqual(BALANCE);
    expect(store.balance.loading.get()).toBe(false);
  });

  it('counts the flagged transactions', async () => {
    await store.load();
    expect(store.flaggedCount.get()).toBe(2);
  });

  it('counts zero before anything has loaded', () => {
    expect(store.flaggedCount.get()).toBe(0);
  });

  it('reports an empty database once both answers are in', async () => {
    client.balance = { accounts: [], total: 0, ytdNetIncome: 0 };
    client.cashflow = { months: [] };

    await store.load();

    expect(store.isEmpty.get()).toBe(true);
  });

  it('does not call a database empty before it has answered', () => {
    // Mid-boot, nothing has replied — which is not the same as nothing existing.
    expect(store.isEmpty.get()).toBe(false);
  });

  it('is not empty when there are accounts', async () => {
    await store.load();
    expect(store.isEmpty.get()).toBe(false);
  });

  it('reports busy while a fetch is in flight', async () => {
    const pending = store.load();
    expect(store.busy.get()).toBe(true);
    await pending;
    expect(store.busy.get()).toBe(false);
  });

  it('hands back the same store from getDashboardStore', () => {
    expect(getDashboardStore()).toBe(store);
  });

  it('refuses to hand back a store that was never initialized', () => {
    resetDashboardStore();
    expect(() => getDashboardStore()).toThrow(/before initializeDashboardStore/);
  });
});
