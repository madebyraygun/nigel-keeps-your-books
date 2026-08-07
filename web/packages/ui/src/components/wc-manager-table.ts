import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '../icons/icons.js';

/** A column in a manager list. `key` is for keying the cells, not for lookup. */
export interface ManagerColumn {
  key: string;
  label: string;
  align?: 'start' | 'end';
  /** Tabular figures — patterns, form lines and anything else read character by character. */
  mono?: boolean;
}

/** A per-row button. `name` is what comes back on the event. */
export interface ManagerAction {
  name: string;
  label: string;
  /** Tag name of a `wc-icon-*`. */
  icon?: string;
  variant?: 'default' | 'danger';
}

export interface ManagerRow {
  id: number;
  /** One per column, in column order. `null` renders as an em dash. */
  cells: (string | number | null)[];
  /**
   * What this row is called, for the action buttons' labels. A column of
   * buttons that all read "Delete" is unusable with a screen reader.
   */
  label: string;
}

export interface NcManagerActionDetail {
  action: string;
  id: number;
}

/**
 * The list every manager screen is built on: columns, rows, per-row actions.
 *
 * Not `wc-report-table`, which renders section and total rows for read-only
 * financial output. These rows carry an id, a busy state and buttons, and never
 * a total; sharing one component would put write semantics into a report
 * renderer for the sake of a shared `<table>` tag.
 */
@customElement('wc-manager-table')
export class WcManagerTable extends LitElement {
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

    td.end,
    th.end {
      text-align: end;
      font-variant-numeric: tabular-nums;
    }

    td.mono {
      font-family: var(--wa-font-family-mono, ui-monospace, monospace);
      overflow-wrap: anywhere;
    }

    .muted {
      color: var(--wa-color-muted);
    }

    tr[aria-busy='true'] {
      opacity: 0.6;
    }

    .actions {
      display: flex;
      justify-content: flex-end;
      gap: var(--wa-space-2xs, 4px);
      white-space: nowrap;
    }

    th.actions-header {
      text-align: end;
    }
  `;

  @property({ attribute: false })
  columns: ManagerColumn[] = [];

  @property({ attribute: false })
  rows: ManagerRow[] = [];

  @property({ attribute: false })
  actions: ManagerAction[] = [];

  /** The row with a request in flight: its buttons go inert. */
  @property({ type: Number, attribute: false })
  busyId: number | null = null;

  /** Visually hidden table caption — what this list is. */
  @property({ type: String })
  caption = '';

  private handleAction(action: string, id: number): void {
    this.dispatchEvent(
      new CustomEvent<NcManagerActionDetail>('nc-manager-action', {
        detail: { action, id },
        bubbles: true,
        composed: true,
      }),
    );
  }

  render() {
    return html`
      <table>
        ${this.caption ? html`<caption>${this.caption}</caption>` : nothing}
        <thead>
          <tr>
            ${this.columns.map(
              (column) => html`
                <th scope="col" class=${column.align === 'end' ? 'end' : ''}>
                  ${column.label}
                </th>
              `,
            )}
            ${this.actions.length > 0
              ? html`<th scope="col" class="actions-header">Actions</th>`
              : nothing}
          </tr>
        </thead>
        <tbody>
          ${this.rows.map((row) => this.renderRow(row))}
        </tbody>
      </table>
    `;
  }

  private renderRow(row: ManagerRow) {
    const busy = this.busyId === row.id;

    return html`
      <tr data-row=${row.id} aria-busy=${busy ? 'true' : 'false'}>
        ${this.columns.map((column, index) => {
          const value = row.cells[index] ?? null;
          const classes = [
            column.align === 'end' ? 'end' : '',
            column.mono ? 'mono' : '',
            value === null ? 'muted' : '',
          ]
            .filter(Boolean)
            .join(' ');
          return html`<td class=${classes}>${value === null ? '—' : value}</td>`;
        })}
        ${this.actions.length > 0
          ? html`
              <td>
                <div class="actions">
                  ${this.actions.map(
                    (action) => html`
                      <wa-button
                        data-action=${action.name}
                        size="s"
                        appearance="outlined"
                        variant=${action.variant === 'danger' ? 'danger' : 'neutral'}
                        ?disabled=${busy}
                        aria-label=${`${action.label} ${row.label}`}
                        @click=${() => this.handleAction(action.name, row.id)}
                      >
                        ${action.icon
                          ? html`<span slot="start">${iconFor(action.icon)}</span>`
                          : nothing}
                        ${action.label}
                      </wa-button>
                    `,
                  )}
                </div>
              </td>
            `
          : nothing}
      </tr>
    `;
  }
}

/**
 * The three glyphs a manager row can carry.
 *
 * A lookup rather than `unsafeStatic`, because the tag name arrives from a
 * caller and building an element out of caller-supplied text is how a template
 * injection starts. The set is small and closed on purpose.
 */
function iconFor(tag: string) {
  switch (tag) {
    case 'wc-icon-edit':
      return html`<wc-icon-edit></wc-icon-edit>`;
    case 'wc-icon-trash':
      return html`<wc-icon-trash></wc-icon-trash>`;
    case 'wc-icon-plus':
      return html`<wc-icon-plus></wc-icon-plus>`;
    default:
      return nothing;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-manager-table': WcManagerTable;
  }
}
