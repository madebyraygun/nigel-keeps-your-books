import { describe, it, expect, beforeEach } from 'vitest';
import { getAppStore, initializeAppStore, resetAppStore } from './app-store.js';
import { ApiError, appLocked } from '../api/index.js';
import { FakeApiClient, LOCKED_STATUS } from '../__mocks__/fake-api-client.js';

describe('app store', () => {
  beforeEach(() => {
    resetAppStore();
    appLocked.set(false);
  });

  it('throws when used before initialization', () => {
    expect(() => getAppStore()).toThrow(/before initializeAppStore/);
  });

  it('returns the initialized store afterwards', () => {
    const store = initializeAppStore(new FakeApiClient());
    expect(getAppStore()).toBe(store);
  });

  it('starts empty', () => {
    const store = initializeAppStore(new FakeApiClient());
    expect(store.status.get()).toBeNull();
    expect(store.statusError.get()).toBeNull();
  });

  it('populates the signals from status', async () => {
    const store = initializeAppStore(new FakeApiClient());
    await store.refreshStatus();

    expect(store.status.get()?.dataDir).toBe('/tmp/nigel');
    expect(store.companyName.get()).toBe('Test Consultancy');
    expect(store.initialized.get()).toBe(true);
    expect(store.version.get()).toBe('0.0.0-test');
    expect(store.statusLoading.get()).toBe(false);
  });

  it('falls back to the product name when no company is set', async () => {
    const client = new FakeApiClient();
    client.status = { ...client.status, companyName: null };
    const store = initializeAppStore(client);
    await store.refreshStatus();
    expect(store.companyName.get()).toBe('Nigel');
  });

  it('records a status failure without throwing', async () => {
    const client = new FakeApiClient();
    client.statusError = new ApiError({
      code: 'internal',
      rawCode: 'internal',
      message: 'boom',
      status: 500,
    });
    const store = initializeAppStore(client);

    await expect(store.refreshStatus()).resolves.toBeUndefined();
    expect(store.statusError.get()?.message).toBe('boom');
    expect(store.statusLoading.get()).toBe(false);
  });

  it('clears a previous error on a later success', async () => {
    const client = new FakeApiClient();
    client.statusError = new ApiError({
      code: 'internal',
      rawCode: 'internal',
      message: 'boom',
      status: 500,
    });
    const store = initializeAppStore(client);
    await store.refreshStatus();

    client.statusError = null;
    await store.refreshStatus();

    expect(store.statusError.get()).toBeNull();
  });

  describe('locked', () => {
    it('follows the status body', async () => {
      const client = new FakeApiClient();
      client.status = LOCKED_STATUS;
      const store = initializeAppStore(client);
      await store.refreshStatus();
      expect(store.locked.get()).toBe(true);
    });

    it('also follows a 423 seen on any other call', async () => {
      // Either source can be first: status at load, or a 423 mid-session.
      const store = initializeAppStore(new FakeApiClient());
      await store.refreshStatus();
      expect(store.locked.get()).toBe(false);

      appLocked.set(true);
      expect(store.locked.get()).toBe(true);
    });
  });

  describe('unlock', () => {
    it('reports success and refreshes status', async () => {
      const client = new FakeApiClient();
      client.status = LOCKED_STATUS;
      const store = initializeAppStore(client);

      await expect(store.unlock('hunter2')).resolves.toEqual({ ok: true });
      expect(client.calls).toContain('unlock:hunter2');
      expect(store.status.get()?.locked).toBe(false);
    });

    it('surfaces the attempts remaining on a wrong password', async () => {
      const client = new FakeApiClient();
      client.unlockError = new ApiError({
        code: 'invalid_password',
        rawCode: 'invalid_password',
        message: 'Wrong password.',
        status: 401,
        details: { attemptsRemaining: 2, retryAfterMs: 1000 },
      });
      const store = initializeAppStore(client);

      expect(await store.unlock('nope')).toEqual({
        ok: false,
        message: 'Wrong password.',
        attemptsRemaining: 2,
        retryAfterMs: 1000,
      });
    });

    it('reports other failures without attempt counts', async () => {
      const client = new FakeApiClient();
      client.unlockError = new ApiError({
        code: 'bad_request',
        rawCode: 'bad_request',
        message: 'not encrypted',
        status: 400,
      });
      const store = initializeAppStore(client);

      expect(await store.unlock('x')).toEqual({ ok: false, message: 'not encrypted' });
    });
  });
});
