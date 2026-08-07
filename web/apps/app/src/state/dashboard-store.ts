import { signal, computed, Signal } from '../mixins/signal-watcher.js';
import { ApiError, type ApiClient } from '../api/index.js';
import type {
  BalanceReport,
  CashflowReport,
  FlaggedTransaction,
  PnlReport,
} from '../api/types.js';
import type { ReadonlySignal } from './app-store.js';

/**
 * One fetch's worth of state.
 *
 * Each of the dashboard's four figures carries its own, because they are
 * fetched in parallel and fail independently: a balance query that times out
 * should cost the reader the balances panel and nothing else.
 */
export interface Fetched<T> {
  data: ReadonlySignal<T | null>;
  loading: ReadonlySignal<boolean>;
  error: ReadonlySignal<ApiError | null>;
}

export interface DashboardStore {
  pnl: Fetched<PnlReport>;
  balance: Fetched<BalanceReport>;
  cashflow: Fetched<CashflowReport>;
  flagged: Fetched<FlaggedTransaction[]>;

  flaggedCount: Signal.Computed<number>;
  /** True once everything has answered and there is nothing to show. */
  isEmpty: Signal.Computed<boolean>;
  /** True while any of the four is in flight. */
  busy: Signal.Computed<boolean>;

  /** Fetch everything. Safe to call repeatedly; this is also what refresh does. */
  load(): Promise<void>;
  reloadPnl(): Promise<void>;
  reloadBalance(): Promise<void>;
  reloadCashflow(): Promise<void>;
  reloadFlagged(): Promise<void>;
}

/** The signals behind one `Fetched`, which only the store writes. */
interface Slot<T> {
  data: Signal.State<T | null>;
  loading: Signal.State<boolean>;
  error: Signal.State<ApiError | null>;
}

function slot<T>(): Slot<T> {
  return {
    data: signal<T | null>(null),
    loading: signal(false),
    error: signal<ApiError | null>(null),
  };
}

function toApiError(error: unknown): ApiError {
  return error instanceof ApiError
    ? error
    : new ApiError({
        code: 'unknown',
        rawCode: 'unknown',
        message: String(error),
        status: 0,
      });
}

/**
 * Run one fetch into its slot.
 *
 * Never throws: a rejected call belongs in that slot's `error` signal, where
 * the card that needs it can render a message and a retry. Letting it escape
 * would take the other three fetches down with it.
 */
async function fill<T>(target: Slot<T>, request: () => Promise<T>): Promise<void> {
  target.loading.set(true);
  try {
    target.data.set(await request());
    target.error.set(null);
  } catch (error) {
    target.error.set(toApiError(error));
  } finally {
    target.loading.set(false);
  }
}

let _store: DashboardStore | null = null;

export function initializeDashboardStore(client: ApiClient): DashboardStore {
  const pnl = slot<PnlReport>();
  const balance = slot<BalanceReport>();
  const cashflow = slot<CashflowReport>();
  const flagged = slot<FlaggedTransaction[]>();

  const reloadPnl = () =>
    fill(pnl, async () => {
      // Year to date, matching the TUI's home screen.
      const { report } = await client.getPnl({ year: new Date().getFullYear() });
      return report;
    });

  const reloadBalance = () =>
    fill(balance, async () => (await client.getBalance()).report);

  const reloadCashflow = () =>
    // Deliberately unfiltered: the window is the last twelve months of data,
    // which the TUI takes client-side and which crosses year boundaries.
    fill(cashflow, async () => (await client.getCashflow()).report);

  const reloadFlagged = () =>
    fill(flagged, async () => (await client.getFlagged()).report);

  const load = async (): Promise<void> => {
    // In parallel, and none of them rejects, so this settles when the slowest
    // does rather than when the first one fails.
    await Promise.all([
      reloadPnl(),
      reloadBalance(),
      reloadCashflow(),
      reloadFlagged(),
    ]);
  };

  const expose = <T>(s: Slot<T>): Fetched<T> => ({
    data: s.data,
    loading: s.loading,
    error: s.error,
  });

  _store = {
    pnl: expose(pnl),
    balance: expose(balance),
    cashflow: expose(cashflow),
    flagged: expose(flagged),

    flaggedCount: computed(() => flagged.data.get()?.length ?? 0),

    // Only empty once something has actually answered — an empty database and
    // a database nobody has asked about yet look identical otherwise, and
    // showing "import your first statement" during the first load would be a
    // lie that corrects itself a moment later.
    isEmpty: computed(() => {
      const balanceReport = balance.data.get();
      const cashflowReport = cashflow.data.get();
      if (!balanceReport || !cashflowReport) return false;
      return (
        balanceReport.accounts.length === 0 && cashflowReport.months.length === 0
      );
    }),

    busy: computed(
      () =>
        pnl.loading.get() ||
        balance.loading.get() ||
        cashflow.loading.get() ||
        flagged.loading.get(),
    ),

    load,
    reloadPnl,
    reloadBalance,
    reloadCashflow,
    reloadFlagged,
  };

  return _store;
}

export function getDashboardStore(): DashboardStore {
  if (!_store) {
    throw new Error('dashboard store used before initializeDashboardStore()');
  }
  return _store;
}

/** Test seam: drop the singleton so each test starts clean. */
export function resetDashboardStore(): void {
  _store = null;
}
