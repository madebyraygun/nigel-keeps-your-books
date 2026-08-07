import { signal, computed, Signal } from '../mixins/signal-watcher.js';
import { ApiError, appLocked, type ApiClient } from '../api/index.js';
import type { StatusResponse } from '../api/types.js';

/**
 * Read-only view handed to consumers. Mutation goes through the store's
 * actions, which keep the derived flags consistent.
 */
export interface ReadonlySignal<T> {
  get(): T;
}

/** What an unlock attempt told us, in the shape the unlock screen needs. */
export type UnlockOutcome =
  | { ok: true }
  | { ok: false; message: string; attemptsRemaining?: number; retryAfterMs?: number };

/**
 * Where the app is in its boot sequence.
 *
 * `starting` — the first `/api/status` has not answered yet.
 * `locked` — the database is encrypted and this process has no key, so the
 * unlock gate is the only thing rendered and no screen exists to fetch data.
 * `failed` — status could not be read at all.
 * `ready` — the app proper.
 */
export type BootPhase = 'starting' | 'locked' | 'failed' | 'ready';

/** What a data-directory switch reported. */
export type SwitchOutcome = { ok: true } | { ok: false; message: string };

export interface AppStore {
  status: ReadonlySignal<StatusResponse | null>;
  statusLoading: ReadonlySignal<boolean>;
  statusError: ReadonlySignal<ApiError | null>;

  boot: Signal.Computed<BootPhase>;
  locked: Signal.Computed<boolean>;
  initialized: Signal.Computed<boolean>;
  companyName: Signal.Computed<string>;
  version: Signal.Computed<string>;

  refreshStatus(): Promise<void>;
  unlock(password: string): Promise<UnlockOutcome>;
  switchDataDir(path: string): Promise<SwitchOutcome>;
}

export interface AppStoreOptions {
  /**
   * How the app restarts itself after a data-directory switch.
   *
   * Injected because jsdom cannot implement `location.reload`, and because the
   * point of the reload is that every screen re-derives from a fresh status —
   * which is exactly what a test needs to observe.
   */
  reload?: () => void;
}

let _status: Signal.State<StatusResponse | null>;
let _statusLoading: Signal.State<boolean>;
let _statusError: Signal.State<ApiError | null>;
let _store: AppStore | null = null;

export function initializeAppStore(
  client: ApiClient,
  options: AppStoreOptions = {},
): AppStore {
  const reload = options.reload ?? (() => window.location.reload());
  _status = signal<StatusResponse | null>(null);
  _statusLoading = signal(false);
  _statusError = signal<ApiError | null>(null);

  const refreshStatus = async (): Promise<void> => {
    _statusLoading.set(true);
    try {
      _status.set(await client.getStatus());
      _statusError.set(null);
    } catch (error) {
      _statusError.set(
        error instanceof ApiError
          ? error
          : new ApiError({
              code: 'unknown',
              rawCode: 'unknown',
              message: String(error),
              status: 0,
            }),
      );
    } finally {
      _statusLoading.set(false);
    }
  };

  const unlock = async (password: string): Promise<UnlockOutcome> => {
    try {
      await client.unlock(password);
      await refreshStatus();
      return { ok: true };
    } catch (error) {
      if (!(error instanceof ApiError)) {
        return { ok: false, message: String(error) };
      }
      const details = error.invalidPasswordDetails();
      return {
        ok: false,
        message: error.message,
        ...(details ?? {}),
      };
    }
  };

  const switchDataDir = async (path: string): Promise<SwitchOutcome> => {
    try {
      await client.setDataDir(path);
    } catch (error) {
      return {
        ok: false,
        message: error instanceof ApiError ? error.message : String(error),
      };
    }
    // Everything on screen describes the database we just stopped serving, so
    // the honest move is to start over rather than patch state item by item.
    reload();
    return { ok: true };
  };

  _store = {
    status: _status,
    statusLoading: _statusLoading,
    statusError: _statusError,

    // Either source can be authoritative first: status reports it at load,
    // and a 423 on any later call raises it without a status round trip.
    locked: computed(() => (_status.get()?.locked ?? false) || appLocked.get()),
    // Derived rather than stored, so a 423 from any later call sends the whole
    // app back to the gate without a bespoke code path.
    boot: computed((): BootPhase => {
      const locked = (_status.get()?.locked ?? false) || appLocked.get();
      if (locked) return 'locked';
      if (_statusError.get()) return 'failed';
      if (!_status.get()) return 'starting';
      return 'ready';
    }),
    initialized: computed(() => _status.get()?.initialized ?? false),
    companyName: computed(() => _status.get()?.companyName || 'Nigel'),
    version: computed(() => _status.get()?.version ?? ''),

    refreshStatus,
    unlock,
    switchDataDir,
  };

  return _store;
}

export function getAppStore(): AppStore {
  if (!_store) {
    throw new Error('app store used before initializeAppStore()');
  }
  return _store;
}

/** Test seam: drop the singleton so each test starts clean. */
export function resetAppStore(): void {
  _store = null;
}
