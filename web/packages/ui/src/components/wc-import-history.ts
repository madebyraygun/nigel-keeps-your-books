import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import './wc-empty-state.js';
import './wc-notice-bar.js';
import './wc-spinner.js';

/**
 * One import, as `GET /api/imports` lists it.
 *
 * Restated here rather than imported: `@nigel/ui` depends on `lit` alone and
 * may not reach for api types. A test in the app asserts the two agree.
 */
export interface ImportHistoryRow {
  id: number;
  filename: string;
  accountName: string;
  importDate: string;
  transactionCount: number;
}

export interface NcImportUndoDetail {
  id: number;
}

/** "42 transactions", but "1 transaction". */
export function transactionCountLabel(count: number): string {
  return `${count} ${count === 1 ? 'transaction' : 'transactions'}`;
}

/**
 * Import history with an Undo button on every row.
 *
 * The web supersets `undo_manager.rs`, which can only offer the most recent
 * import because a terminal has nothing to point at — `delete_import` has
 * always taken an id. An import whose rows are gone still lists, at zero: the
 * count is what is attached to it, not a reason to hide it.
 */
@customElement('wc-import-history')
export class WcImportHistory extends LitElement {
  static styles = css`
    :host {
      display: block;
      overflow-x: auto;
    }

    table {
      width: 100%;
      border-collapse: collapse;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-text);
      font-family: var(--wa-font-family-sans);
    }

    caption {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }

    th,
    td {
      text-align: start;
      padding: var(--wa-space-xs, 6px) var(--wa-space-s, 8px);
      border-bottom: 1px solid var(--wa-color-border);
      vertical-align: top;
    }

    th {
      font-weight: var(--wa-font-weight-medium, 500);
      color: var(--wa-color-muted);
      white-space: nowrap;
    }

    td.file {
      font-family: var(--wa-font-family-mono, ui-monospace, monospace);
      overflow-wrap: anywhere;
    }

    td.count,
    th.count {
      text-align: end;
      font-variant-numeric: tabular-nums;
    }

    td.actions {
      text-align: end;
      white-space: nowrap;
    }

    th.actions {
      text-align: end;
    }

    tr[aria-busy='true'] {
      opacity: 0.6;
    }

    .loading {
      display: grid;
      place-items: center;
      padding: var(--wa-space-l, 16px);
    }
  `;

  @property({ attribute: false })
  imports: ImportHistoryRow[] = [];

  @property({ type: Boolean, reflect: true })
  loading = false;

  /** A load failure. Renders in place of the table, with a retry. */
  @property({ attribute: false })
  error: string | null = null;

  /** The row being undone: its button goes inert while the request is out. */
  @property({ attribute: false })
  busyId: number | null = null;

  private handleUndo(id: number): void {
    this.dispatchEvent(
      new CustomEvent<NcImportUndoDetail>('nc-import-undo', {
        detail: { id },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private handleRetry = (): void => {
    this.dispatchEvent(new CustomEvent('nc-retry', { bubbles: true, composed: true }));
  };

  render() {
    if (this.error) {
      return html`
        <wc-notice-bar
          variant="danger"
          message=${this.error}
          action-label="Try again"
          @nc-notice-action=${this.handleRetry}
        ></wc-notice-bar>
      `;
    }

    if (this.loading) {
      return html`
        <div class="loading">
          <wc-spinner show-label label="Loading imports"></wc-spinner>
        </div>
      `;
    }

    if (this.imports.length === 0) {
      return html`
        <wc-empty-state
          icon="wc-icon-undo"
          heading="Nothing to undo"
          message="No imports to undo."
        ></wc-empty-state>
      `;
    }

    return html`
      <table>
        <caption>
          Import history
        </caption>
        <thead>
          <tr>
            <th scope="col">File</th>
            <th scope="col">Account</th>
            <th scope="col">Imported</th>
            <th scope="col" class="count">Transactions</th>
            <th scope="col" class="actions">Actions</th>
          </tr>
        </thead>
        <tbody>
          ${this.imports.map((item) => this.renderRow(item))}
        </tbody>
      </table>
    `;
  }

  private renderRow(item: ImportHistoryRow) {
    const busy = this.busyId === item.id;

    return html`
      <tr data-row=${item.id} aria-busy=${busy ? 'true' : 'false'}>
        <td class="file">${item.filename}</td>
        <td>${item.accountName}</td>
        <td>${item.importDate}</td>
        <td class="count">${item.transactionCount}</td>
        <td class="actions">
          <wa-button
            data-undo
            size="s"
            appearance="outlined"
            variant="danger"
            ?disabled=${busy}
            aria-label=${`Undo import of ${item.filename}`}
            @click=${() => this.handleUndo(item.id)}
          >
            Undo
          </wa-button>
        </td>
      </tr>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-import-history': WcImportHistory;
  }

  interface HTMLElementEventMap {
    'nc-import-undo': CustomEvent<NcImportUndoDetail>;
  }
}
