import { describe, it, expect, afterEach, beforeEach } from 'vitest';
import './nigel-app.js';
import type { NigelApp } from './nigel-app.js';
import { appLocked, appUnauthorized, ApiError } from '../api/index.js';
import { resetAppStore } from '../state/app-store.js';
import { FakeApiClient, LOCKED_STATUS } from '../__mocks__/fake-api-client.js';

/**
 * The whole app driven by FakeApiClient — which is itself the point: if the
 * root container can run on an ApiClient that never opens a socket, the seam
 * really is swappable for Tauri or a remote server.
 */
async function mount(client = new FakeApiClient()): Promise<NigelApp> {
  const el = document.createElement('nigel-app');
  el.client = client;
  document.body.appendChild(el);
  await el.updateComplete;
  // One more turn for the status promise to settle and re-render.
  await new Promise((r) => setTimeout(r, 0));
  await el.updateComplete;
  return el;
}

const shell = (el: NigelApp) => el.shadowRoot?.querySelector('wc-app-shell');
const sidebar = (el: NigelApp) => el.shadowRoot?.querySelector('wc-nav-sidebar');

describe('nigel-app', () => {
  beforeEach(() => {
    resetAppStore();
    appLocked.set(false);
    appUnauthorized.set(false);
    window.location.hash = '';
  });

  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('shows a spinner until status arrives', async () => {
    const el = document.createElement('nigel-app');
    el.client = new FakeApiClient();
    document.body.appendChild(el);
    await el.updateComplete;

    expect(el.shadowRoot?.querySelector('wc-spinner')).toBeTruthy();
    expect(shell(el)).toBeNull();
  });

  it('renders the shell and sidebar once status arrives', async () => {
    const el = await mount();
    expect(shell(el)).toBeTruthy();
    expect(sidebar(el)).toBeTruthy();
  });

  it('seeds the hash so a reload lands on the same screen', async () => {
    await mount();
    expect(window.location.hash).toBe('#/dashboard');
  });

  it('renders the screen named by the hash', async () => {
    window.location.hash = '#/register';
    const el = await mount();
    expect(shell(el)?.getAttribute('screen-title')).toBe('Register');
  });

  it('swaps screens on hashchange', async () => {
    const el = await mount();
    expect(shell(el)?.getAttribute('screen-title')).toBe('Dashboard');

    window.location.hash = '#/reports';
    window.dispatchEvent(new HashChangeEvent('hashchange'));
    await el.updateComplete;

    expect(shell(el)?.getAttribute('screen-title')).toBe('Reports');
  });

  it('navigates by writing the hash, so back works', async () => {
    // The hash is the single writer of route state; a nav click only sets it.
    const el = await mount();
    sidebar(el)?.dispatchEvent(
      new CustomEvent('nc-navigate', {
        detail: { id: 'review' },
        bubbles: true,
        composed: true,
      }),
    );
    expect(window.location.hash).toBe('#/review');
  });

  it('falls back to the dashboard for an unknown hash', async () => {
    window.location.hash = '#/nonsense';
    const el = await mount();
    expect(shell(el)?.getAttribute('screen-title')).toBe('Dashboard');
  });

  it('titles the document with the screen and company', async () => {
    const el = await mount();
    expect(document.title).toBe('Dashboard · Test Consultancy');
    expect(el).toBeTruthy();
  });

  describe('locked', () => {
    const lockedClient = () => {
      const client = new FakeApiClient();
      client.status = LOCKED_STATUS;
      return client;
    };

    it('shows the unlock gate instead of the app', async () => {
      const el = await mount(lockedClient());
      expect(el.shadowRoot?.querySelector('nigel-unlock-screen')).toBeTruthy();
    });

    it('renders no shell and no sidebar while locked', async () => {
      // Structural, not cosmetic: with no shell there is no screen element, so
      // there is nothing that could fetch data before the password arrives.
      const el = await mount(lockedClient());
      expect(shell(el)).toBeNull();
      expect(sidebar(el)).toBeNull();
    });

    it('leaves the hash alone so unlocking returns the user where they were', async () => {
      window.location.hash = '#/reports';
      await mount(lockedClient());
      expect(window.location.hash).toBe('#/reports');
    });

    it('fetches nothing but status before the database is unlocked', async () => {
      // The whole acceptance criterion in one assertion: the only call the app
      // makes while locked is the one that told it it was locked.
      const client = lockedClient();
      await mount(client);
      expect(client.calls).toEqual(['getStatus']);
    });

    it('enters the app once the password lands', async () => {
      const client = lockedClient();
      const el = await mount(client);

      el.shadowRoot
        ?.querySelector('nigel-unlock-screen')
        ?.shadowRoot?.querySelector('wc-unlock-card')
        ?.dispatchEvent(
          new CustomEvent('nc-unlock', {
            detail: { password: 'hunter2' },
            bubbles: true,
            composed: true,
          }),
        );
      await new Promise((r) => setTimeout(r, 0));
      await el.updateComplete;

      // A prefix rather than the whole log: once the shell exists the screen
      // behind it starts loading its own data, and pinning the exact tail here
      // would fail for every screen that ever fetches anything. What this test
      // is about is the order of the first three — nothing before the unlock.
      expect(client.calls.slice(0, 3)).toEqual([
        'getStatus',
        'unlock:hunter2',
        'getStatus',
      ]);
      expect(shell(el)).toBeTruthy();
    });

    it('returns to the gate when any later call reports a locked database', async () => {
      const el = await mount();
      expect(shell(el)).toBeTruthy();

      appLocked.set(true);
      await el.updateComplete;

      expect(shell(el)).toBeNull();
      expect(el.shadowRoot?.querySelector('nigel-unlock-screen')).toBeTruthy();
    });
  });

  describe('banners', () => {
    it('explains a dead session, since the SPA cannot mint a token', async () => {
      const el = await mount();
      appUnauthorized.set(true);
      await el.updateComplete;

      const banner = el.shadowRoot?.querySelector('[slot="banner"]');
      expect(banner?.textContent).toContain('Session expired');
      expect(banner?.textContent).toContain('nigel serve');
    });

    it('offers a retry when status could not be loaded', async () => {
      const client = new FakeApiClient();
      client.statusError = new ApiError({
        code: 'internal',
        rawCode: 'internal',
        message: 'database is on fire',
        status: 500,
      });
      const el = await mount(client);

      const banner = el.shadowRoot?.querySelector('[slot="banner"]');
      expect(banner?.textContent).toContain('database is on fire');
      expect(banner?.querySelector('button')).toBeTruthy();
    });

    it('retries the status call when asked', async () => {
      const client = new FakeApiClient();
      client.statusError = new ApiError({
        code: 'internal',
        rawCode: 'internal',
        message: 'boom',
        status: 500,
      });
      const el = await mount(client);

      client.statusError = null;
      el.shadowRoot?.querySelector<HTMLButtonElement>('[slot="banner"] button')?.click();
      await new Promise((r) => setTimeout(r, 0));
      await el.updateComplete;

      expect(shell(el)?.getAttribute('screen-title')).toBe('Dashboard');
    });

    it('shows no banner when everything is fine', async () => {
      const el = await mount();
      expect(el.shadowRoot?.querySelector('[slot="banner"]')).toBeNull();
    });
  });
});
