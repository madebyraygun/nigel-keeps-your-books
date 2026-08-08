import { LitElement, html, css, nothing, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/switch/switch.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '@nigel/ui';
import { confirmDialog, dispatchNcToast, type NcPasswordSubmitDetail } from '@nigel/ui';

import { SignalWatcher } from '../mixins/signal-watcher.js';
import { ApiError, type ApiClient } from '../api/index.js';
import { getAppStore, type AppStore } from '../state/app-store.js';
import type { AppSettings } from '../api/types.js';
import type { ScreenContext } from './context.js';

/**
 * Settings: business name, auto-update check, data directory, and the database
 * password — what `settings_manager.rs` covers, plus `nigel load`.
 *
 * A screen with state is a custom element under `screens/`; visual primitives
 * still live in `@nigel/ui`. This is the first of them, and the convention the
 * remaining screen tasks follow.
 */
@customElement('nigel-settings-screen')
export class NigelSettingsScreen extends SignalWatcher(LitElement) {
  static styles = css`
    :host {
      display: grid;
      gap: var(--wa-space-l, 16px);
      max-width: 48rem;
      padding: var(--wa-space-l, 16px);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .row {
      display: flex;
      gap: var(--wa-space-s, 8px);
      align-items: flex-end;
      flex-wrap: wrap;
    }

    .row wa-input {
      flex: 1 1 18rem;
    }

    .path {
      font-family: var(--wa-font-family-mono, monospace);
      font-size: var(--wa-font-size-s, 13px);
      background: var(--wa-color-surface-alt);
      border-radius: var(--wa-radius-s, 6px);
      padding: var(--wa-space-2xs, 4px) var(--wa-space-xs, 6px);
      overflow-wrap: anywhere;
    }

    .note {
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
      margin: var(--wa-space-xs, 6px) 0 0;
    }

    .error {
      color: var(--wa-color-danger);
      font-size: var(--wa-font-size-s, 13px);
      margin: var(--wa-space-xs, 6px) 0 0;
    }
  `;

  /** Supplied by the registry from the screen context. */
  @property({ attribute: false })
  client!: ApiClient;

  @state() private appSettings: AppSettings | null = null;
  @state() private companyDraft = '';
  @state() private dataDirDraft = '';
  @state() private busy: string | null = null;
  @state() private passwordError = '';
  @state() private dataDirError = '';

  private store: AppStore = getAppStore();
  private seededCompany = false;

  connectedCallback(): void {
    super.connectedCallback();
    void this.loadAppSettings();
  }

  private async loadAppSettings(): Promise<void> {
    try {
      this.appSettings = await this.client.getAppSettings();
    } catch (error) {
      this.toastError(error, 'Could not load settings.');
    }
  }

  private toastError(error: unknown, fallback: string): void {
    const message = error instanceof ApiError ? error.message : fallback;
    dispatchNcToast(this, { message, variant: 'danger' });
  }

  private toastOk(message: string): void {
    dispatchNcToast(this, { message, variant: 'success' });
  }

  // -- business name --------------------------------------------------------

  private get companyName(): string {
    // Seeded once: re-reading the store on every render would overwrite what
    // the user is in the middle of typing.
    if (!this.seededCompany) {
      this.seededCompany = true;
      this.companyDraft = this.store.status.get()?.companyName ?? '';
    }
    return this.companyDraft;
  }

  private handleCompanyInput = (event: Event) => {
    this.companyDraft = (event.target as HTMLInputElement).value;
  };

  private saveCompanyName = async () => {
    this.busy = 'company';
    try {
      await this.client.setCompanyName(this.companyDraft);
      // The sidebar and the document title read this from status.
      await this.store.refreshStatus();
      this.toastOk('Name saved.');
    } catch (error) {
      this.toastError(error, 'Could not save the name.');
    } finally {
      this.busy = null;
    }
  };

  // -- update check ---------------------------------------------------------

  private toggleUpdateCheck = async (event: Event) => {
    const control = event.target as HTMLElement & { checked: boolean };
    const wanted = control.checked;
    const previous = this.appSettings;
    this.busy = 'update';
    try {
      this.appSettings = await this.client.updateAppSettings({ updateCheck: wanted });
    } catch (error) {
      // Put the switch back where the server still has it: a control whose
      // write failed must not keep claiming it succeeded.
      //
      // The control is reset directly rather than by re-rendering the old
      // value. The user moved the DOM property out from under lit, so lit's
      // dirty check sees the value it already committed and skips the update,
      // leaving the switch showing a state nobody saved.
      this.appSettings = previous;
      control.checked = previous?.updateCheck ?? true;
      this.toastError(error, 'Could not save the update setting.');
    } finally {
      this.busy = null;
    }
  };

  // -- data directory -------------------------------------------------------

  private handleDataDirInput = (event: Event) => {
    this.dataDirDraft = (event.target as HTMLInputElement).value;
  };

  private switchDataDir = async () => {
    const path = this.dataDirDraft.trim();
    if (path.length === 0) return;

    this.dataDirError = '';
    const confirmed = await confirmDialog({
      heading: 'Switch data directory?',
      message: `Nigel will reload and open the books in ${path}.`,
      confirmLabel: 'Switch',
    });
    if (!confirmed) return;

    this.busy = 'data-dir';
    const outcome = await this.store.switchDataDir(path);
    this.busy = null;
    if (!outcome.ok) this.dataDirError = outcome.message;
  };

  // -- password -------------------------------------------------------------

  private handlePasswordSubmit = async (event: CustomEvent<NcPasswordSubmitDetail>) => {
    const detail = event.detail;
    if (detail.mode === 'remove') {
      const confirmed = await confirmDialog({
        heading: 'Remove the database password?',
        message:
          'The database will be decrypted and readable by anyone with access to the file.',
        confirmLabel: 'Remove password',
        variant: 'danger',
      });
      if (!confirmed) return;
    }

    this.passwordError = '';
    this.busy = 'password';
    try {
      if (detail.mode === 'set') {
        await this.client.setPassword({ newPassword: detail.newPassword ?? '' });
        this.toastOk('Database encrypted.');
      } else if (detail.mode === 'change') {
        await this.client.changePassword({
          currentPassword: detail.currentPassword ?? '',
          newPassword: detail.newPassword ?? '',
        });
        this.toastOk('Password changed.');
      } else {
        await this.client.removePassword({
          currentPassword: detail.currentPassword ?? '',
        });
        this.toastOk('Password removed.');
      }
      // Re-read rather than assume: the encryption state the next render draws
      // comes from the server, never from an optimistic local flag.
      await this.store.refreshStatus();
    } catch (error) {
      this.passwordError =
        error instanceof ApiError ? error.message : 'Could not change the password.';
    } finally {
      this.busy = null;
    }
  };

  render() {
    const status = this.store.status.get();
    const encrypted = status?.encrypted ?? false;
    // Same metadata field either way; only the label follows the books profile.
    const nameLabel = status?.profile === 'personal' ? 'Household name' : 'Business name';

    return html`
      <wc-panel
        heading=${nameLabel}
        description="Shown in the sidebar, on reports, and in the browser tab."
      >
        <div class="row">
          <wa-input
            label=${nameLabel}
            .value=${this.companyName}
            ?disabled=${this.busy === 'company'}
            @input=${this.handleCompanyInput}
          ></wa-input>
        </div>
        <wa-button
          slot="actions"
          variant="brand"
          ?disabled=${this.busy === 'company'}
          @click=${this.saveCompanyName}
          >Save</wa-button
        >
      </wc-panel>

      <wc-panel
        heading="Updates"
        description="Check GitHub for a newer version once a day when nigel starts."
      >
        ${this.appSettings
          ? html`<wa-switch
              ?checked=${this.appSettings.updateCheck}
              ?disabled=${this.busy === 'update'}
              @change=${this.toggleUpdateCheck}
              >Check for updates automatically</wa-switch
            >`
          : html`<wc-spinner label="Loading settings"></wc-spinner>`}
      </wc-panel>

      <wc-panel
        heading="Data directory"
        description="Where this set of books lives. Switching reloads nigel onto the other database."
      >
        <p class="path">${status?.dataDir ?? ''}</p>
        <div class="row">
          <wa-input
            label="Switch to"
            placeholder="~/Documents/other-books"
            .value=${this.dataDirDraft}
            ?disabled=${this.busy === 'data-dir'}
            @input=${this.handleDataDirInput}
          ></wa-input>
        </div>
        ${this.dataDirError
          ? html`<p class="error">${this.dataDirError}</p>`
          : nothing}
        <wa-button
          slot="actions"
          ?disabled=${this.busy === 'data-dir'}
          @click=${this.switchDataDir}
          >Switch</wa-button
        >
      </wc-panel>

      <wc-panel
        heading="Database password"
        description=${encrypted
          ? 'This database is encrypted.'
          : 'This database is not encrypted. Anyone with the file can read it.'}
      >
        <wc-password-form
          mode=${encrypted ? 'change' : 'set'}
          ?busy=${this.busy === 'password'}
          error=${this.passwordError}
          @nc-password-submit=${this.handlePasswordSubmit}
        ></wc-password-form>
        ${encrypted
          ? html`<wc-password-form
              mode="remove"
              ?busy=${this.busy === 'password'}
              @nc-password-submit=${this.handlePasswordSubmit}
            ></wc-password-form>`
          : nothing}
      </wc-panel>
    `;
  }
}

export function renderSettings(ctx: ScreenContext): TemplateResult {
  return html`<nigel-settings-screen .client=${ctx.client}></nigel-settings-screen>`;
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-settings-screen': NigelSettingsScreen;
  }
}
