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
    it('forces the unlock screen', async () => {
      const client = new FakeApiClient();
      client.status = LOCKED_STATUS;
      const el = await mount(client);
      expect(shell(el)?.getAttribute('screen-title')).toBe('Unlock');
    });

    it('leaves the hash alone so unlocking returns the user where they were', async () => {
      window.location.hash = '#/reports';
      const client = new FakeApiClient();
      client.status = LOCKED_STATUS;
      await mount(client);
      expect(window.location.hash).toBe('#/reports');
    });

    it('disables every nav item', async () => {
      const client = new FakeApiClient();
      client.status = LOCKED_STATUS;
      const el = await mount(client);
      const items = (sidebar(el) as HTMLElement & { items: { disabled?: boolean }[] })
        .items;
      expect(items.every((i) => i.disabled)).toBe(true);
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
