import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/input/input.js';

/** An account a statement can be imported into. */
export interface ImportAccountOption {
  id: number;
  name: string;
  accountType: string;
}

/** A built-in importer, as `GET /api/imports/formats` publishes it. */
export interface ImportFormatOption {
  key: string;
  name: string;
  accountTypes: string[];
}

/**
 * Column positions for a CSV no built-in importer can read.
 *
 * Field-for-field the api layer's `GenericCsvConfig`, restated here because
 * `@nigel/ui` depends on `lit` alone and may not import api types. A test in
 * the app asserts the two shapes stay identical.
 */
export interface GenericCsvMapping {
  dateCol: number;
  descCol: number;
  amountCol: number;
  dateFormat: string;
}

export interface ImportFormValue {
  /** Account name, empty when none is chosen yet. */
  account: string;
  /** Empty to auto-detect, a built-in key, a profile name, or `GENERIC_FORMAT_CHOICE`. */
  format: string;
  mapping: GenericCsvMapping;
  /** Name to save the mapping under; empty saves nothing. */
  saveProfile: string;
}

export interface NcImportChangeDetail {
  value: ImportFormValue;
}

/** The format value that opens the column-mapping fields. */
export const GENERIC_FORMAT_CHOICE = '__generic__';

/** What `nigel import` defaults to when given columns but no `--date-format`. */
export const DEFAULT_CSV_MAPPING: GenericCsvMapping = {
  dateCol: 0,
  descCol: 1,
  amountCol: 2,
  dateFormat: '%m/%d/%Y',
};

export const EMPTY_IMPORT_FORM: ImportFormValue = {
  account: '',
  format: '',
  mapping: DEFAULT_CSV_MAPPING,
  saveProfile: '',
};

/**
 * Everything about an import except the file: which account, which reader, and
 * — when nothing built in can read it — where the columns are.
 *
 * One component rather than three loose selects on the screen, following
 * `wc-register-toolbar`: the fields are interdependent (choosing "Generic CSV"
 * reveals four more, and the save-profile name only means anything alongside
 * them), and the component-first rule keeps `wa-*` primitives out of screens.
 *
 * Controlled: it never edits `value`, it only reports what the value would
 * become. That is what lets the screen keep format and mapping mutually
 * exclusive — the two can never both be set, because the choice is one field.
 */
@customElement('wc-import-form')
export class WcImportForm extends LitElement {
  static styles = css`
    :host {
      display: grid;
      gap: var(--wa-space-m, 12px);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .row {
      display: flex;
      flex-wrap: wrap;
      gap: var(--wa-space-m, 12px);
    }

    .row > * {
      flex: 1 1 16rem;
      min-width: 0;
    }

    .mapping {
      display: grid;
      gap: var(--wa-space-s, 8px);
      padding: var(--wa-space-m, 12px);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-m, 8px);
      background: var(--wa-color-surface);
    }

    .mapping-heading {
      margin: 0;
      font-size: var(--wa-font-size-s, 13px);
      font-weight: var(--wa-font-weight-semibold, 600);
    }

    .mapping-hint {
      margin: 0;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
    }

    .columns {
      display: flex;
      flex-wrap: wrap;
      gap: var(--wa-space-s, 8px);
    }

    .columns > * {
      flex: 1 1 7rem;
      min-width: 0;
    }

    .hint {
      margin: 0;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
    }

    .error {
      margin: 0;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-danger);
    }
  `;

  @property({ attribute: false })
  accounts: ImportAccountOption[] = [];

  @property({ attribute: false })
  formats: ImportFormatOption[] = [];

  /** Saved CSV profile names, from `GET /api/csv-profiles`. */
  @property({ attribute: false })
  profiles: string[] = [];

  @property({ attribute: false })
  value: ImportFormValue = EMPTY_IMPORT_FORM;

  @property({ type: String, attribute: 'account-error' })
  accountError = '';

  @property({ type: String, attribute: 'format-error' })
  formatError = '';

  @property({ type: String, attribute: 'mapping-error' })
  mappingError = '';

  @property({ type: Boolean, reflect: true })
  disabled = false;

  private get generic(): boolean {
    return this.value.format === GENERIC_FORMAT_CHOICE;
  }

  private get selectedAccount(): ImportAccountOption | undefined {
    return this.accounts.find((account) => account.name === this.value.account);
  }

  private emit(patch: Partial<ImportFormValue>): void {
    this.dispatchEvent(
      new CustomEvent<NcImportChangeDetail>('nc-import-change', {
        detail: { value: { ...this.value, ...patch } },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private emitMapping(patch: Partial<GenericCsvMapping>): void {
    this.emit({ mapping: { ...this.value.mapping, ...patch } });
  }

  private static valueOf(event: Event): string {
    return (event.target as HTMLElement & { value: string }).value ?? '';
  }

  private handleAccount = (event: Event): void => {
    this.emit({ account: WcImportForm.valueOf(event) });
  };

  private handleFormat = (event: Event): void => {
    this.emit({ format: WcImportForm.valueOf(event) });
  };

  private handleColumn(key: keyof GenericCsvMapping) {
    return (event: Event): void => {
      const parsed = Number.parseInt(WcImportForm.valueOf(event), 10);
      // A half-typed or cleared field must not become NaN in the payload; the
      // last good value stands until a real number replaces it.
      if (!Number.isInteger(parsed) || parsed < 0) return;
      this.emitMapping({ [key]: parsed } as Partial<GenericCsvMapping>);
    };
  }

  private handleDateFormat = (event: Event): void => {
    this.emitMapping({ dateFormat: WcImportForm.valueOf(event) });
  };

  private handleSaveProfile = (event: Event): void => {
    this.emit({ saveProfile: WcImportForm.valueOf(event) });
  };

  render() {
    return html`
      <div class="row">${this.renderAccount()} ${this.renderFormat()}</div>
      ${this.generic ? this.renderMapping() : nothing}
    `;
  }

  private renderAccount() {
    const account = this.selectedAccount;

    return html`
      <div>
        <wa-select
          label="Account"
          size="s"
          value=${this.value.account}
          ?disabled=${this.disabled || this.accounts.length === 0}
          @change=${this.handleAccount}
        >
          ${this.accounts.map(
            (option) =>
              html`<wa-option value=${option.name}>${option.name}</wa-option>`,
          )}
        </wa-select>
        ${this.accounts.length === 0
          ? html`<p class="hint">
              No accounts yet — add one before importing a statement.
            </p>`
          : account
            ? html`<p class="hint">${account.accountType.replace(/_/g, ' ')}</p>`
            : nothing}
        ${this.accountError
          ? html`<p class="error" role="alert">${this.accountError}</p>`
          : nothing}
      </div>
    `;
  }

  /**
   * One flat list. Web Awesome's select has no option-group element, so the
   * saved profiles say so in their labels rather than sitting under a heading
   * that would have to be faked with a disabled option.
   */
  private renderFormat() {
    return html`
      <div>
        <wa-select
          label="Format"
          size="s"
          value=${this.value.format}
          ?disabled=${this.disabled}
          @change=${this.handleFormat}
        >
          <wa-option value="">Detect automatically</wa-option>
          ${this.formats.map(
            (format) => html`<wa-option value=${format.key}>${format.name}</wa-option>`,
          )}
          ${this.profiles.map(
            (name) => html`<wa-option value=${name}>Saved: ${name}</wa-option>`,
          )}
          <wa-option value=${GENERIC_FORMAT_CHOICE}>Generic CSV…</wa-option>
        </wa-select>
        ${this.formatError
          ? html`<p class="error" role="alert">${this.formatError}</p>`
          : nothing}
      </div>
    `;
  }

  private renderMapping() {
    const { mapping, saveProfile } = this.value;

    return html`
      <div class="mapping">
        <p class="mapping-heading">Column mapping</p>
        <p class="mapping-hint">
          Counting from zero, as <code>nigel import</code> does.
        </p>
        <div class="columns">
          <wa-input
            class="date-col"
            type="number"
            min="0"
            label="Date column"
            size="s"
            value=${String(mapping.dateCol)}
            ?disabled=${this.disabled}
            @input=${this.handleColumn('dateCol')}
          ></wa-input>
          <wa-input
            class="desc-col"
            type="number"
            min="0"
            label="Description column"
            size="s"
            value=${String(mapping.descCol)}
            ?disabled=${this.disabled}
            @input=${this.handleColumn('descCol')}
          ></wa-input>
          <wa-input
            class="amount-col"
            type="number"
            min="0"
            label="Amount column"
            size="s"
            value=${String(mapping.amountCol)}
            ?disabled=${this.disabled}
            @input=${this.handleColumn('amountCol')}
          ></wa-input>
          <wa-input
            class="date-format"
            label="Date format"
            size="s"
            value=${mapping.dateFormat}
            ?disabled=${this.disabled}
            @input=${this.handleDateFormat}
          ></wa-input>
        </div>
        <wa-input
          class="save-profile"
          label="Save as profile (optional)"
          size="s"
          placeholder="chase"
          value=${saveProfile}
          ?disabled=${this.disabled}
          @input=${this.handleSaveProfile}
        ></wa-input>
        ${this.mappingError
          ? html`<p class="error" role="alert">${this.mappingError}</p>`
          : nothing}
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-import-form': WcImportForm;
  }

  interface HTMLElementEventMap {
    'nc-import-change': CustomEvent<NcImportChangeDetail>;
  }
}
