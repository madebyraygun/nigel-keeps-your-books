import { LitElement, html, css, nothing, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import '@nigel/ui';
import {
  dispatchNcToast,
  EMPTY_IMPORT_FORM,
  type ImportAccountOption,
  type ImportFormValue,
  type NcFileErrorDetail,
  type NcFileSelectDetail,
  type NcImportChangeDetail,
} from '@nigel/ui';

import { ApiError, type ApiClient } from '../api/index.js';
import type {
  CsvProfile,
  ImportConfirmation,
  ImporterFormat,
  ImportPreview,
} from '../api/types.js';
import {
  confirmRequestBody,
  formatLabel,
  importRequestBody,
  previewCounts,
  resultCounts,
  routeImportError,
} from './import-data.js';
import type { ScreenContext } from './context.js';
import type { ScreenId } from './registry.js';

type Busy = 'upload' | 'preview' | 'confirm' | null;

/**
 * The browser's import flow: choose, preview, confirm — one screen, three
 * panels that appear as the decision is made.
 *
 * The upload is lazy. Picking a file sends nothing; Preview uploads and then
 * dry-runs in one action. Two reasons: a file chosen and thought better of
 * never reaches the server's spool at all, and the account has to be known
 * before an upload is worth anything. The `uploadId` is then cached against
 * the chosen file, so correcting a column mapping and previewing again re-reads
 * the file the server already has rather than sending it a second time.
 *
 * A duplicate file blocks the confirm rather than warning about it. The server
 * would answer 200 with zero counts — the checksum is checked before anything
 * is parsed — so the button would offer a no-op, and offering a no-op is worse
 * than not offering it.
 */
@customElement('nigel-import-screen')
export class NigelImportScreen extends LitElement {
  static styles = css`
    :host {
      display: grid;
      gap: var(--wa-space-m, 12px);
      align-content: start;
      padding: var(--wa-space-l, 16px);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
      max-width: 56rem;
    }

    .stack {
      display: grid;
      gap: var(--wa-space-m, 12px);
    }

    .format-line {
      margin: 0;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
    }

    .format-line strong {
      color: var(--wa-color-text);
      font-weight: var(--wa-font-weight-medium, 500);
    }

    .snapshot {
      overflow-wrap: anywhere;
      font-family: var(--wa-font-family-mono);
      font-size: var(--wa-font-size-s, 13px);
    }

    button.action {
      font: inherit;
      padding: var(--wa-space-xs, 6px) var(--wa-space-m, 12px);
      border-radius: var(--wa-radius-m, 8px);
      border: 1px solid var(--wa-color-border);
      background: var(--wa-color-surface);
      color: inherit;
      cursor: pointer;
    }

    button.action.primary {
      border-color: var(--wa-color-brand);
      color: var(--wa-color-brand);
      font-weight: var(--wa-font-weight-medium, 500);
    }

    button.action:disabled {
      opacity: 0.5;
      cursor: default;
    }

    a {
      color: var(--wa-color-brand);
    }

    .load-error {
      margin: 0;
      color: var(--wa-color-danger);
      font-size: var(--wa-font-size-s, 13px);
    }
  `;

  /** Supplied by the registry from the screen context. */
  @property({ attribute: false })
  client!: ApiClient;

  @property({ attribute: false })
  params: URLSearchParams = new URLSearchParams();

  @property({ attribute: false })
  navigate?: (screen: ScreenId, params?: URLSearchParams) => void;

  @state() private accounts: ImportAccountOption[] = [];
  @state() private formats: ImporterFormat[] = [];
  @state() private profiles: CsvProfile[] = [];
  @state() private loadError: string | null = null;

  @state() private form: ImportFormValue = EMPTY_IMPORT_FORM;
  @state() private filename = '';
  @state() private filesize = 0;

  @state() private preview: ImportPreview | null = null;
  @state() private result: ImportConfirmation | null = null;
  @state() private busy: Busy = null;

  @state() private dropzoneError = '';
  @state() private accountError = '';
  @state() private formatError = '';
  @state() private mappingError = '';

  /** The chosen file. Not reactive: nothing renders from it but its name. */
  private file: File | null = null;

  /** The server's copy of `file`, reused until the file changes. */
  private uploadId: string | null = null;

  /** Toast text already shown, so a repeated failure does not stack up. */
  private toasted = new Set<string>();

  firstUpdated(): void {
    void this.load();
  }

  // -- loading --------------------------------------------------------------

  /**
   * The three lists the form is built from.
   *
   * `allSettled` rather than `all`: a missing profile list is no reason to
   * withhold the accounts, and the form degrades to whatever did arrive.
   */
  private async load(): Promise<void> {
    this.loadError = null;

    const [accounts, formats, profiles] = await Promise.allSettled([
      this.client.getAccounts(),
      this.client.getImportFormats(),
      this.client.getCsvProfiles(),
    ]);

    if (accounts.status === 'fulfilled') {
      this.accounts = accounts.value.map((account) => ({
        id: account.id,
        name: account.name,
        accountType: account.accountType,
      }));
      // One account is not a choice; preselecting it saves a click that has
      // only one possible outcome.
      if (this.accounts.length === 1 && this.form.account === '') {
        this.form = { ...this.form, account: this.accounts[0].name };
      }
    } else {
      this.reportLoadFailure(accounts.reason, 'Could not load your accounts.');
    }

    if (formats.status === 'fulfilled') {
      this.formats = formats.value;
    } else {
      this.reportLoadFailure(formats.reason, 'Could not load the importer list.');
    }

    if (profiles.status === 'fulfilled') {
      this.profiles = profiles.value;
    } else {
      this.reportLoadFailure(profiles.reason, 'Could not load your saved CSV profiles.');
    }
  }

  private reportLoadFailure(reason: unknown, fallback: string): void {
    const message = reason instanceof ApiError ? reason.message : fallback;
    this.loadError = message;
    if (reason instanceof ApiError && (reason.isLocked || reason.isUnauthorized)) return;
    this.toast(message);
  }

  private toast(message: string): void {
    if (this.toasted.has(message)) return;
    this.toasted.add(message);
    dispatchNcToast(this, { message, variant: 'danger' });
  }

  // -- choosing -------------------------------------------------------------

  private handleFileSelect = (event: Event): void => {
    const { file } = (event as CustomEvent<NcFileSelectDetail>).detail;
    this.file = file;
    this.filename = file.name;
    this.filesize = file.size;
    // A new file invalidates everything downstream of it, including the copy
    // the server is holding.
    this.uploadId = null;
    this.preview = null;
    this.result = null;
    this.clearErrors();
  };

  private handleFileError = (event: Event): void => {
    this.dropzoneError = (event as CustomEvent<NcFileErrorDetail>).detail.message;
  };

  private handleFileClear = (): void => {
    this.file = null;
    this.filename = '';
    this.filesize = 0;
    this.uploadId = null;
    this.preview = null;
    this.clearErrors();
  };

  private handleFormChange = (event: Event): void => {
    this.form = (event as CustomEvent<NcImportChangeDetail>).detail.value;
    // The previous preview described a different reading of the file.
    this.preview = null;
    this.clearErrors();
  };

  private clearErrors(): void {
    this.dropzoneError = '';
    this.accountError = '';
    this.formatError = '';
    this.mappingError = '';
  }

  private get ready(): boolean {
    return this.file !== null && this.form.account !== '' && this.busy === null;
  }

  /**
   * Whether confirming would actually import something.
   *
   * A preview with nothing to import is the same trap a duplicate file is: the
   * confirm succeeds, adds no rows, and records the file's checksum, after
   * which the file can never be imported — so a corrected format or column
   * mapping arrives too late.
   */
  private get importable(): boolean {
    return (
      this.preview !== null && !this.preview.duplicateFile && this.preview.imported > 0
    );
  }

  // -- the two calls --------------------------------------------------------

  /** The server's copy of the chosen file, uploading it if it has none. */
  private async ensureUpload(): Promise<string> {
    if (this.uploadId !== null) return this.uploadId;
    if (this.file === null) throw new Error('no file chosen');

    this.busy = 'upload';
    const upload = await this.client.uploadImport(this.file);
    this.uploadId = upload.uploadId;
    return upload.uploadId;
  }

  /**
   * Run one of the two calls, re-uploading once if the spool has forgotten us.
   *
   * Uploads expire after an hour, and a preview left open over lunch is the
   * ordinary way to meet that. The file is still in the browser, so the honest
   * response is to send it again rather than to make someone re-choose it.
   */
  private async withUpload<T>(
    call: (uploadId: string) => Promise<T>,
    mayRetry = true,
  ): Promise<T> {
    const uploadId = await this.ensureUpload();
    try {
      return await call(uploadId);
    } catch (error) {
      if (!mayRetry || !(error instanceof ApiError) || !error.isUploadExpired) throw error;
      this.uploadId = null;
      return this.withUpload(call, false);
    }
  }

  private handlePreview = async (): Promise<void> => {
    if (!this.ready) return;
    this.clearErrors();
    this.result = null;

    try {
      const body = importRequestBody(this.form);
      this.preview = await this.withUpload((uploadId) => {
        this.busy = 'preview';
        return this.client.previewImport({ uploadId, ...body });
      });
    } catch (error) {
      this.preview = null;
      this.surface(error);
    } finally {
      this.busy = null;
    }
  };

  private handleConfirm = async (): Promise<void> => {
    if (this.busy !== null || !this.importable) return;
    this.clearErrors();

    try {
      const body = confirmRequestBody(this.form);
      const result = await this.withUpload((uploadId) => {
        this.busy = 'confirm';
        return this.client.confirmImport({ uploadId, ...body });
      });

      this.result = result;
      this.preview = null;
      // A confirmed upload is gone server-side, and a saved profile should
      // show up in the list the next import offers.
      this.uploadId = null;
      if (body.saveProfile !== undefined) void this.refreshProfiles();
    } catch (error) {
      this.surface(error);
    } finally {
      this.busy = null;
    }
  };

  private async refreshProfiles(): Promise<void> {
    try {
      this.profiles = await this.client.getCsvProfiles();
    } catch {
      // The profile is saved either way; a stale list is not worth a message.
    }
  }

  /** Put a failure where it belongs, per `routeImportError`. */
  private surface(error: unknown): void {
    const routed = routeImportError(error, this.form);

    switch (routed.field) {
      case 'dropzone':
        this.dropzoneError = routed.message;
        break;
      case 'account':
        this.accountError = routed.message;
        break;
      case 'format':
        this.formatError = routed.message;
        break;
      case 'mapping':
        this.mappingError = routed.message;
        break;
      case 'none':
        break;
    }

    if (routed.toast) this.toast(routed.message);
  }

  private handleReset = (): void => {
    this.file = null;
    this.filename = '';
    this.filesize = 0;
    this.uploadId = null;
    this.preview = null;
    this.result = null;
    this.clearErrors();
    this.toasted.clear();
    // The account and the format stay: a second statement for the same account
    // is the ordinary next thing to do.
  };

  // -- rendering ------------------------------------------------------------

  render() {
    if (this.result !== null) return this.renderResult(this.result);

    return html`
      ${this.renderChoose()} ${this.preview ? this.renderPreview(this.preview) : nothing}
    `;
  }

  private renderChoose(): TemplateResult {
    const busyNow = this.busy !== null;

    return html`
      <wc-panel
        heading="Import a statement"
        description="Nothing is written until you confirm. A preview reads the file and reports what would happen."
      >
        <div class="stack">
          ${this.loadError
            ? html`<p class="load-error" role="alert">${this.loadError}</p>`
            : nothing}
          <wc-dropzone
            filename=${this.filename}
            .size=${this.filesize}
            error=${this.dropzoneError}
            ?busy=${busyNow}
            @nc-file-select=${this.handleFileSelect}
            @nc-file-error=${this.handleFileError}
            @nc-file-clear=${this.handleFileClear}
          ></wc-dropzone>

          <wc-import-form
            .accounts=${this.accounts}
            .formats=${this.formats}
            .profiles=${this.profiles.map((profile) => profile.name)}
            .value=${this.form}
            account-error=${this.accountError}
            format-error=${this.formatError}
            mapping-error=${this.mappingError}
            ?disabled=${busyNow}
            @nc-import-change=${this.handleFormChange}
          ></wc-import-form>

          ${busyNow
            ? html`<wc-spinner
                show-label
                label=${this.busy === 'confirm' ? 'Importing' : 'Reading the file'}
              ></wc-spinner>`
            : nothing}
        </div>

        <button
          class="action primary"
          slot="actions"
          type="button"
          ?disabled=${!this.ready}
          @click=${this.handlePreview}
        >
          Preview
        </button>
      </wc-panel>
    `;
  }

  private renderPreview(preview: ImportPreview): TemplateResult {
    if (preview.duplicateFile) {
      return html`
        <wc-panel heading="Already imported">
          <wc-notice-bar
            variant="warning"
            message="nigel has seen this exact file before, so importing it again would add nothing. Choose a different statement."
          ></wc-notice-bar>
        </wc-panel>
      `;
    }

    return html`
      <wc-panel heading="Preview">
        <div class="stack">
          <p class="format-line">
            Read as
            <strong>${formatLabel(preview.format, this.formats, this.profiles)}</strong>
          </p>
          <wc-count-grid .items=${previewCounts(preview)}></wc-count-grid>
          ${preview.imported === 0
            ? html`<wc-notice-bar
                variant="warning"
                message="There is nothing here to import. Check the format and, for a generic CSV, the column mapping above, then preview again."
              ></wc-notice-bar>`
            : nothing}
          <wc-sample-table
            .rows=${preview.sample}
            caption=${`First ${preview.sample.length} rows of ${this.filename}`}
            empty-message="No rows in this file could be parsed."
          ></wc-sample-table>
        </div>

        <button
          class="action primary"
          slot="actions"
          type="button"
          ?disabled=${this.busy !== null || !this.importable}
          @click=${this.handleConfirm}
        >
          Import ${preview.imported} transactions
        </button>
      </wc-panel>
    `;
  }

  private renderResult(result: ImportConfirmation): TemplateResult {
    if (result.duplicateFile) {
      return html`
        <wc-panel heading="Already imported">
          <wc-notice-bar
            variant="warning"
            message="That file had already been imported, so nothing was added."
          ></wc-notice-bar>
          <button class="action" slot="actions" type="button" @click=${this.handleReset}>
            Import another
          </button>
        </wc-panel>
      `;
    }

    return html`
      <wc-panel
        heading="Import complete"
        description=${`${this.filename} was read as ${formatLabel(result.format, this.formats, this.profiles)}.`}
      >
        <div class="stack">
          <wc-count-grid .items=${resultCounts(result)}></wc-count-grid>
          <wc-notice-bar variant="info">
            A snapshot was taken first, at
            <span class="snapshot">${result.snapshot}</span>
          </wc-notice-bar>
        </div>

        ${result.stillFlagged > 0
          ? html`<a slot="actions" href="#/review"
              >Review ${result.stillFlagged} flagged</a
            >`
          : nothing}
        <button class="action" slot="actions" type="button" @click=${this.handleReset}>
          Import another
        </button>
      </wc-panel>
    `;
  }
}

export function renderImport(ctx: ScreenContext): TemplateResult {
  return html`
    <nigel-import-screen
      .client=${ctx.client}
      .params=${ctx.params}
      .navigate=${ctx.navigate}
    ></nigel-import-screen>
  `;
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-import-screen': NigelImportScreen;
  }
}
