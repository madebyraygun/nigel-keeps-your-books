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

describe('app store boot phase', () => {
  beforeEach(() => {
    resetAppStore();
    appLocked.set(false);
  });

  it('starts before status has answered', () => {
    const store = initializeAppStore(new FakeApiClient());
    expect(store.boot.get()).toBe('starting');
  });

  it('is ready once an unlocked status arrives', async () => {
    const store = initializeAppStore(new FakeApiClient());
    await store.refreshStatus();
    expect(store.boot.get()).toBe('ready');
  });

  it('is locked when the database is encrypted', async () => {
    const client = new FakeApiClient();
    client.status = LOCKED_STATUS;
    const store = initializeAppStore(client);
    await store.refreshStatus();
    expect(store.boot.get()).toBe('locked');
  });

  it('is locked when any later call reports a locked database', async () => {
    // A 423 from anywhere sends the app back to the gate; no bespoke path.
    const store = initializeAppStore(new FakeApiClient());
    await store.refreshStatus();
    appLocked.set(true);
    expect(store.boot.get()).toBe('locked');
  });

  it('fails when status could not be read', async () => {
    const client = new FakeApiClient();
    client.statusError = new ApiError({
      code: 'internal',
      rawCode: 'internal',
      message: 'boom',
      status: 500,
    });
    const store = initializeAppStore(client);
    await store.refreshStatus();
    expect(store.boot.get()).toBe('failed');
  });

  it('becomes ready after a successful unlock', async () => {
    const client = new FakeApiClient();
    client.status = LOCKED_STATUS;
    const store = initializeAppStore(client);
    await store.refreshStatus();

    const outcome = await store.unlock('hunter2');

    expect(outcome).toEqual({ ok: true });
    expect(store.boot.get()).toBe('ready');
  });
});

describe('switching data directory', () => {
  beforeEach(() => {
    resetAppStore();
    appLocked.set(false);
  });

  it('reloads the app once the server has switched', async () => {
    let reloads = 0;
    const client = new FakeApiClient();
    const store = initializeAppStore(client, { reload: () => (reloads += 1) });

    const outcome = await store.switchDataDir('/tmp/other');

    expect(outcome).toEqual({ ok: true });
    expect(client.calls).toContain('setDataDir');
    expect(reloads).toBe(1);
  });

  it('does not reload when the switch was refused', async () => {
    // A bad path leaves the current books on screen, which is the honest state.
    let reloads = 0;
    const client = new FakeApiClient();
    client.settingsError = new ApiError({
      code: 'bad_request',
      rawCode: 'bad_request',
      message: 'No database found at /nope/nigel.db',
      status: 400,
    });
    const store = initializeAppStore(client, { reload: () => (reloads += 1) });

    const outcome = await store.switchDataDir('/nope');

    expect(outcome).toEqual({
      ok: false,
      message: 'No database found at /nope/nigel.db',
    });
    expect(reloads).toBe(0);
  });
});
