import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import './settings.js';
import type { NigelSettingsScreen } from './settings.js';
import { ApiError, appLocked } from '../api/index.js';
import { initializeAppStore, resetAppStore } from '../state/app-store.js';
import { FakeApiClient } from '../__mocks__/fake-api-client.js';

/**
 * The settings screen driven entirely by FakeApiClient — no network, no server,
 * and every assertion is about which api method the screen chose to call.
 */
let reloads = 0;

async function mount(client = new FakeApiClient()): Promise<{
  el: NigelSettingsScreen;
  client: FakeApiClient;
}> {
  reloads = 0;
  const store = initializeAppStore(client, { reload: () => (reloads += 1) });
  await store.refreshStatus();
  // The store's own bookkeeping call is not what these tests are about.
  client.calls.length = 0;

  const el = document.createElement('nigel-settings-screen');
  el.client = client;
  document.body.appendChild(el);
  await el.updateComplete;
  await new Promise((r) => setTimeout(r, 0));
  await el.updateComplete;
  return { el, client };
}

const panel = (el: NigelSettingsScreen, index: number) =>
  el.shadowRoot?.querySelectorAll('wc-panel')[index];

const input = (el: NigelSettingsScreen, index: number) =>
  el.shadowRoot?.querySelectorAll('wa-input')[index] as HTMLElement & { value: string };

function button(el: NigelSettingsScreen, panelIndex: number) {
  return panel(el, panelIndex)?.querySelector('wa-button') as HTMLElement | undefined;
}

async function settle(el: NigelSettingsScreen): Promise<void> {
  await new Promise((r) => setTimeout(r, 0));
  await el.updateComplete;
}

describe('settings screen', () => {
  beforeEach(() => {
    resetAppStore();
    appLocked.set(false);
  });

  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it('loads the application settings when it mounts', async () => {
    const { client } = await mount();
    expect(client.calls).toContain('getAppSettings');
  });

  it('shows the active data directory', async () => {
    const { el } = await mount();
    expect(el.shadowRoot?.textContent).toContain('/tmp/nigel');
  });

  it('labels the name field from the books profile', async () => {
    const { el } = await mount();
    expect(panel(el, 0)?.getAttribute('heading')).toBe('Business name');

    const client = new FakeApiClient();
    client.status = { ...client.status, profile: 'personal' };
    const { el: personal } = await mount(client);
    expect(panel(personal, 0)?.getAttribute('heading')).toBe('Household name');
  });

  describe('business name', () => {
    it('saves the name and re-reads status so the sidebar follows', async () => {
      const { el, client } = await mount();
      input(el, 0).value = 'Raygun LLC';
      input(el, 0).dispatchEvent(new Event('input'));
      await el.updateComplete;

      button(el, 0)?.click();
      await settle(el);

      expect(client.calls).toContain('setCompanyName');
      // The name is shown from status, so a save that skipped the refresh would
      // leave a stale sidebar behind it.
      expect(client.calls.lastIndexOf('getStatus')).toBeGreaterThan(
        client.calls.indexOf('setCompanyName'),
      );
      expect(client.status.companyName).toBe('Raygun LLC');
    });

    it('reports a failed save without changing what is shown', async () => {
      const client = new FakeApiClient();
      const { el } = await mount(client);
      client.settingsError = new ApiError({
        code: 'internal',
        rawCode: 'internal',
        message: 'disk is full',
        status: 500,
      });

      button(el, 0)?.click();
      await settle(el);

      expect(client.status.companyName).toBe('Test Consultancy');
    });
  });

  describe('update check', () => {
    it('writes the new value', async () => {
      const { el, client } = await mount();
      const toggle = el.shadowRoot?.querySelector('wa-switch') as HTMLElement & {
        checked: boolean;
      };
      toggle.checked = false;
      toggle.dispatchEvent(new Event('change'));
      await settle(el);

      expect(client.calls).toContain('updateAppSettings');
      expect(client.appSettings.updateCheck).toBe(false);
    });

    it('puts the switch back when the write fails', async () => {
      const client = new FakeApiClient();
      const { el } = await mount(client);
      expect(client.appSettings.updateCheck).toBe(true);
      client.settingsError = new ApiError({
        code: 'internal',
        rawCode: 'internal',
        message: 'nope',
        status: 500,
      });

      const toggle = el.shadowRoot?.querySelector('wa-switch') as HTMLElement & {
        checked: boolean;
      };
      toggle.checked = false;
      toggle.dispatchEvent(new Event('change'));
      await settle(el);

      // A control whose write failed must not keep claiming it succeeded.
      // Asserted on the property, not the attribute: the property is what the
      // user moved, and re-rendering the old value does not move it back.
      expect(toggle.checked).toBe(true);
      expect(client.appSettings.updateCheck).toBe(true);
    });
  });

  describe('data directory', () => {
    it('asks before switching, and does nothing when refused', async () => {
      const ui = await import('@nigel/ui');
      vi.spyOn(ui, 'confirmDialog').mockResolvedValue(false);

      const { el, client } = await mount();
      const field = input(el, 1);
      field.value = '/tmp/other';
      field.dispatchEvent(new Event('input'));
      await el.updateComplete;

      button(el, 2)?.click();
      await settle(el);

      expect(client.calls).not.toContain('setDataDir');
    });

    it('switches once confirmed', async () => {
      const ui = await import('@nigel/ui');
      vi.spyOn(ui, 'confirmDialog').mockResolvedValue(true);

      const { el, client } = await mount();
      const field = input(el, 1);
      field.value = '/tmp/other';
      field.dispatchEvent(new Event('input'));
      await el.updateComplete;

      button(el, 2)?.click();
      await settle(el);

      expect(client.calls).toContain('setDataDir');
      expect(reloads).toBe(1);
    });

    it('does nothing when the path is blank', async () => {
      const { el, client } = await mount();
      button(el, 2)?.click();
      await settle(el);
      expect(client.calls).not.toContain('setDataDir');
    });
  });

  describe('password', () => {
    function submitPassword(
      el: NigelSettingsScreen,
      detail: Record<string, unknown>,
      formIndex = 0,
    ) {
      const forms = el.shadowRoot?.querySelectorAll('wc-password-form');
      forms?.[formIndex]?.dispatchEvent(
        new CustomEvent('nc-password-submit', {
          detail,
          bubbles: true,
          composed: true,
        }),
      );
    }

    it('offers set on a plaintext database', async () => {
      const { el } = await mount();
      const form = el.shadowRoot?.querySelector('wc-password-form');
      expect(form?.getAttribute('mode')).toBe('set');
    });

    it('encrypts and then re-reads the encryption state', async () => {
      const { el, client } = await mount();
      submitPassword(el, { mode: 'set', newPassword: 'hunter2' });
      await settle(el);

      expect(client.calls).toContain('setPassword');
      expect(client.calls.lastIndexOf('getStatus')).toBeGreaterThan(
        client.calls.indexOf('setPassword'),
      );
    });

    it('offers change and remove on an encrypted database', async () => {
      const client = new FakeApiClient();
      client.status = { ...client.status, encrypted: true };
      const { el } = await mount(client);

      const modes = [...(el.shadowRoot?.querySelectorAll('wc-password-form') ?? [])].map(
        (f) => f.getAttribute('mode'),
      );
      expect(modes).toEqual(['change', 'remove']);
    });

    it('confirms before removing the password', async () => {
      const ui = await import('@nigel/ui');
      const confirm = vi.spyOn(ui, 'confirmDialog').mockResolvedValue(false);

      const client = new FakeApiClient();
      client.status = { ...client.status, encrypted: true };
      const { el } = await mount(client);

      submitPassword(el, { mode: 'remove', currentPassword: 'hunter2' }, 1);
      await settle(el);

      expect(confirm).toHaveBeenCalled();
      expect(client.calls).not.toContain('removePassword');
    });

    it('surfaces a wrong current password on the form', async () => {
      const client = new FakeApiClient();
      client.status = { ...client.status, encrypted: true };
      const { el } = await mount(client);
      client.settingsError = new ApiError({
        code: 'invalid_password',
        rawCode: 'invalid_password',
        message: 'Wrong password.',
        status: 401,
        details: { attemptsRemaining: 2, retryAfterMs: 0 },
      });

      submitPassword(el, {
        mode: 'change',
        currentPassword: 'nope',
        newPassword: 'new one',
      });
      await settle(el);

      expect(
        el.shadowRoot?.querySelector('wc-password-form')?.getAttribute('error'),
      ).toBe('Wrong password.');
    });
  });
});
