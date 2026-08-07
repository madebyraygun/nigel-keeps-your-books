import { LitElement, html, css, nothing, type PropertyValues } from 'lit';
import { customElement, property, state, query } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '../icons/icons.js';
import './wc-money.js';
import './wc-empty-state.js';
import { categoryLabel, type CategoryOption } from './category-option.js';

export type { CategoryOption };

/** One transaction, in the shape the register table draws. */
export interface RegisterTableRow {
  id: number;
  /** `YYYY-MM-DD`. */
  date: string;
  description: string;
  amount: number;
  category: string | null;
  categoryId: number | null;
  vendor: string | null;
  accountName: string;
  isFlagged: boolean;
}

export interface NcRowEventDetail {
  id: number;
}

export interface NcEditCommitDetail {
  id: number;
  /** The chosen category, or null when the row is left uncategorized. */
  categoryId: number | null;
  /** The typed vendor, or null when the field was left empty (a clear). */
  vendor: string | null;
}

export interface NcFlagToggleDetail {
  id: number;
  /** The desired state, not a toggle — a retry must be safe. */
  flag: boolean;
}

/**
 * Rows to move by on PgUp/PgDn when the viewport cannot be measured — jsdom
 * reports every height as zero. Matches the TUI's `PAGE_SIZE`.
 */
const DEFAULT_PAGE_ROWS = 20;

/**
 * The transaction register — the web counterpart of `browser.rs`.
 *
 * Two constraints shape it. Rows stay cheap: a row that is not being edited
 * renders text, one `wc-money` and one icon button, and no `wa-*` component at
 * all, because an unfiltered register is thousands of rows and every custom
 * element in a row is paid for thousands of times. And selection follows DOM
 * focus through a roving tabindex, so the keyboard model the TUI has is the
 * same one a screen reader announces.
 *
 * The category editor is a hand-built combobox rather than `wa-select`: this
 * Web Awesome build has no searchable select, and the combobox ARIA wiring
 * (`aria-activedescendant` onto the option list) needs an input this component
 * owns rather than one inside another component's shadow root.
 */
@customElement('wc-register-table')
export class WcRegisterTable extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
      min-height: 0;
    }

    .scroller {
      overflow: auto;
      max-height: var(--nc-register-height, 60vh);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-md, 8px);
    }

    table {
      width: 100%;
      border-collapse: collapse;
    }

    caption {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }

    th {
      position: sticky;
      top: 0;
      z-index: 1;
      text-align: left;
      font-size: var(--wa-font-size-s, 13px);
      font-weight: var(--wa-font-weight-medium, 500);
      color: var(--wa-color-muted);
      background: var(--wa-color-surface);
      padding: var(--wa-space-xs, 6px) var(--wa-space-s, 8px);
      border-bottom: 1px solid var(--wa-color-border);
      white-space: nowrap;
    }

    td {
      padding: var(--wa-space-xs, 6px) var(--wa-space-s, 8px);
      border-bottom: 1px solid var(--wa-color-border-soft, var(--wa-color-border));
      vertical-align: top;
    }

    :host([dense]) td {
      padding: var(--wa-space-2xs, 4px) var(--wa-space-s, 8px);
    }

    th.amount,
    td.amount {
      text-align: right;
      width: 12ch;
    }

    td.date {
      white-space: nowrap;
      font-variant-numeric: tabular-nums;
    }

    th.flag,
    td.flag {
      width: 2.5rem;
      text-align: center;
    }

    tbody tr[aria-selected='true'] {
      background: var(--wa-color-surface-alt, rgba(120, 120, 160, 0.18));
    }

    tbody tr[data-flagged='true'] td.date {
      box-shadow: inset 3px 0 0 var(--nc-color-flagged, #e0a13a);
    }

    tbody tr:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: -2px;
    }

    tbody tr[aria-busy='true'] {
      opacity: 0.6;
    }

    .muted {
      color: var(--wa-color-muted);
    }

    .icon-button {
      font: inherit;
      color: var(--wa-color-muted);
      background: none;
      border: none;
      border-radius: var(--wa-radius-sm, 6px);
      padding: 2px;
      cursor: pointer;
      line-height: 0;
    }

    .icon-button[aria-pressed='true'] {
      color: var(--nc-color-flagged, #e0a13a);
    }

    .icon-button:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 1px;
    }

    tfoot td {
      border-bottom: none;
      border-top: 2px solid var(--wa-color-border);
      font-weight: var(--wa-font-weight-bold, 700);
      position: sticky;
      bottom: 0;
      background: var(--wa-color-surface);
    }

    .note {
      font-weight: var(--wa-font-weight-normal, 400);
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
    }

    /* -- the inline editors -- */

    .combobox {
      position: relative;
    }

    .combobox input {
      font: inherit;
      width: 100%;
      min-width: 12ch;
      color: var(--wa-color-text);
      background: var(--wa-color-bg);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-sm, 6px);
      padding: var(--wa-space-2xs, 4px) var(--wa-space-xs, 6px);
    }

    .combobox input:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 1px;
    }

    .options {
      position: absolute;
      z-index: 2;
      margin: 2px 0 0;
      padding: 0;
      list-style: none;
      max-height: 12rem;
      overflow: auto;
      min-width: 100%;
      background: var(--wa-color-surface);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-sm, 6px);
      box-shadow: var(--wa-shadow-m, 0 4px 12px rgba(0, 0, 0, 0.25));
    }

    .options li {
      padding: var(--wa-space-2xs, 4px) var(--wa-space-xs, 6px);
      cursor: pointer;
      white-space: nowrap;
    }

    .options li[aria-selected='true'] {
      background: var(--wa-color-brand, #4a6cf7);
      color: var(--wa-color-on-brand, #fff);
    }

    /* The label names the field for a screen reader; the column header shows
       a sighted user the same thing, so it does not need the space. */
    .vendor-input::part(form-control-label) {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }

    .sr-only {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }

    .edit-actions {
      display: flex;
      gap: var(--wa-space-2xs, 4px);
      justify-content: flex-end;
    }

    .edit-actions button {
      font: inherit;
      font-size: var(--wa-font-size-s, 13px);
      color: inherit;
      background: none;
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-sm, 6px);
      padding: 2px var(--wa-space-xs, 6px);
      cursor: pointer;
    }

    .edit-actions button:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 1px;
    }
  `;

  @property({ attribute: false })
  rows: RegisterTableRow[] = [];

  @property({ attribute: false })
  categories: CategoryOption[] = [];

  @property({ type: Number, attribute: 'selected-id' })
  selectedId: number | null = null;

  /** The row in edit mode. The host owns this: activation only asks for it. */
  @property({ type: Number, attribute: 'editing-id' })
  editingId: number | null = null;

  /** A row with a write in flight. */
  @property({ type: Number, attribute: 'busy-id' })
  busyId: number | null = null;

  @property({ type: Boolean, reflect: true })
  dense = false;

  /**
   * Drop every affordance that writes: no flag button, no activation, no edit
   * mode. Selection and scrolling stay, because reading a long register still
   * wants a cursor.
   *
   * This is what the reports screen's register view uses. Editing lives at
   * `#/register`; offering it from two screens would mean two places to keep
   * honest about the same row.
   */
  @property({ type: Boolean, reflect: true })
  readonly = false;

  @property({ type: Boolean, attribute: 'show-account' })
  showAccount = true;

  /** The table's accessible name. */
  @property({ type: String })
  caption = 'Transaction register';

  /** Rendered as the footer's running total. Omit and no total is shown. */
  @property({ type: Number })
  total?: number;

  /** Free text beside the total, e.g. a match count. */
  @property({ type: String, attribute: 'footer-note' })
  footerNote = '';

  @property({ type: String, attribute: 'empty-message' })
  emptyMessage = 'No transactions match these filters.';

  /**
   * Selection, held internally so the component still works uncontrolled (in
   * the preview harness, say). A host that sets `selectedId` overrides it.
   */
  @state() private activeId: number | null = null;

  @state() private editCategoryId: number | null = null;
  @state() private editCategoryQuery = '';
  @state() private editOptionIndex = 0;
  @state() private editListOpen = false;

  @query('.scroller') private scroller?: HTMLElement;
  @query('.vendor-input') private vendorInput?: HTMLElement & { value: string };

  private pendingFocusId: number | null = null;

  willUpdate(changed: PropertyValues<this>): void {
    if (changed.has('selectedId')) this.activeId = this.selectedId;

    if (changed.has('rows') && this.activeId !== null) {
      // A row that filtering removed cannot stay selected.
      if (!this.rows.some((row) => row.id === this.activeId)) {
        this.activeId = this.rows[0]?.id ?? null;
      }
    }

    if (changed.has('editingId')) this.resetEditState();
  }

  updated(): void {
    if (this.pendingFocusId === null) return;
    const id = this.pendingFocusId;
    this.pendingFocusId = null;
    this.rowElement(id)?.focus();
  }

  // -- public API -----------------------------------------------------------

  /** Select a row by index and bring it into view. Used for scroll-to-today. */
  scrollToIndex(index: number): boolean {
    const row = this.rows[index];
    if (!row) return false;

    this.setActive(row.id);
    void this.updateComplete.then(() => this.bringIntoView(row.id));
    return true;
  }

  /** Select a row by transaction id and bring it into view. */
  scrollToRow(id: number): boolean {
    const index = this.rows.findIndex((row) => row.id === id);
    return index === -1 ? false : this.scrollToIndex(index);
  }

  // -- selection ------------------------------------------------------------

  private rowElement(id: number): HTMLElement | null {
    return this.shadowRoot?.querySelector<HTMLElement>(`tr[data-id="${id}"]`) ?? null;
  }

  private bringIntoView(id: number): void {
    const element = this.rowElement(id);
    // jsdom has no layout and no scrollIntoView.
    if (element && typeof element.scrollIntoView === 'function') {
      element.scrollIntoView({ block: 'center' });
    }
  }

  private setActive(id: number): void {
    if (this.activeId === id) return;
    this.activeId = id;
    this.dispatchEvent(
      new CustomEvent<NcRowEventDetail>('nc-row-select', {
        detail: { id },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private get activeIndex(): number {
    return this.rows.findIndex((row) => row.id === this.activeId);
  }

  private focusIndex(index: number): void {
    const clamped = Math.min(Math.max(index, 0), this.rows.length - 1);
    const row = this.rows[clamped];
    if (!row) return;
    this.setActive(row.id);
    this.pendingFocusId = row.id;
    this.requestUpdate();
  }

  private pageRows(): number {
    const rowElement = this.shadowRoot?.querySelector('tbody tr');
    const height = rowElement?.getBoundingClientRect().height ?? 0;
    const viewport = this.scroller?.clientHeight ?? 0;
    const rows = height > 0 ? Math.floor(viewport / height) : 0;
    return rows > 1 ? rows : DEFAULT_PAGE_ROWS;
  }

  // -- keyboard -------------------------------------------------------------

  private handleKeydown(event: KeyboardEvent): void {
    if (this.editingId !== null) return;
    if (this.rows.length === 0) return;

    const current = this.activeIndex;
    const from = current === -1 ? 0 : current;

    switch (event.key) {
      case 'ArrowDown':
        this.focusIndex(current === -1 ? 0 : from + 1);
        break;
      case 'ArrowUp':
        this.focusIndex(current === -1 ? 0 : from - 1);
        break;
      case 'PageDown':
        this.focusIndex(from + this.pageRows());
        break;
      case 'PageUp':
        this.focusIndex(from - this.pageRows());
        break;
      case 'Home':
        this.focusIndex(0);
        break;
      case 'End':
        this.focusIndex(this.rows.length - 1);
        break;
      case 'Enter':
        this.activateRow(this.rows[from]);
        break;
      case 'Escape':
        this.activeId = null;
        break;
      case 'f':
      case 'F':
        this.requestFlag(this.rows[from]);
        break;
      case '/':
        this.dispatchEvent(
          new CustomEvent('nc-search-focus', { bubbles: true, composed: true }),
        );
        break;
      default:
        return;
    }

    event.preventDefault();
  }

  private activateRow(row: RegisterTableRow | undefined): void {
    if (!row) return;
    this.setActive(row.id);
    // A read-only table still moves its cursor; it just never asks to edit.
    if (this.readonly) return;
    this.dispatchEvent(
      new CustomEvent<NcRowEventDetail>('nc-row-activate', {
        detail: { id: row.id },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private requestFlag(row: RegisterTableRow | undefined): void {
    if (!row || this.readonly) return;
    this.dispatchEvent(
      new CustomEvent<NcFlagToggleDetail>('nc-flag-toggle', {
        detail: { id: row.id, flag: !row.isFlagged },
        bubbles: true,
        composed: true,
      }),
    );
  }

  // -- inline editing -------------------------------------------------------

  private get editingRow(): RegisterTableRow | undefined {
    return this.rows.find((row) => row.id === this.editingId);
  }

  private resetEditState(): void {
    const row = this.editingRow;
    this.editCategoryId = row?.categoryId ?? null;
    this.editCategoryQuery = row?.category ?? '';
    this.editOptionIndex = 0;
    this.editListOpen = false;
  }

  /** Case-insensitive substring over the "Name (inc)" label, as the TUI does. */
  private get filteredCategories(): CategoryOption[] {
    const query = this.editCategoryQuery.trim().toLowerCase();
    if (query === '') return this.categories;
    return this.categories.filter((category) =>
      categoryLabel(category).toLowerCase().includes(query),
    );
  }

  private chooseCategory(option: CategoryOption | undefined): void {
    if (!option) return;
    this.editCategoryId = option.id;
    this.editCategoryQuery = option.name;
    this.editListOpen = false;
    this.editOptionIndex = 0;
  }

  private handleCategoryInput(event: Event): void {
    this.editCategoryQuery = (event.target as HTMLInputElement).value;
    this.editListOpen = true;
    this.editOptionIndex = 0;
  }

  private handleCategoryKeydown(event: KeyboardEvent): void {
    const options = this.filteredCategories;

    switch (event.key) {
      case 'ArrowDown':
        this.editListOpen = true;
        this.editOptionIndex = Math.min(this.editOptionIndex + 1, options.length - 1);
        break;
      case 'ArrowUp':
        this.editListOpen = true;
        this.editOptionIndex = Math.max(this.editOptionIndex - 1, 0);
        break;
      case 'Enter':
        // Category then vendor, the order the TUI's two-stage editor uses.
        this.chooseCategory(options[this.editOptionIndex]);
        void this.updateComplete.then(() => this.vendorInput?.focus());
        break;
      case 'Escape':
        this.cancelEdit();
        break;
      default:
        return;
    }

    event.preventDefault();
    event.stopPropagation();
  }

  private handleVendorKeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter') {
      this.commitEdit();
    } else if (event.key === 'Escape') {
      this.cancelEdit();
    } else {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
  }

  private commitEdit(): void {
    const row = this.editingRow;
    if (!row) return;

    const vendor = (this.vendorInput?.value ?? '').trim();
    this.dispatchEvent(
      new CustomEvent<NcEditCommitDetail>('nc-edit-commit', {
        detail: {
          id: row.id,
          categoryId: this.editCategoryId,
          vendor: vendor === '' ? null : vendor,
        },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private cancelEdit(): void {
    const row = this.editingRow;
    if (!row) return;
    this.dispatchEvent(
      new CustomEvent<NcRowEventDetail>('nc-edit-cancel', {
        detail: { id: row.id },
        bubbles: true,
        composed: true,
      }),
    );
  }

  // -- render ---------------------------------------------------------------

  render() {
    if (this.rows.length === 0) {
      return html`
        <wc-empty-state
          icon="wc-icon-register"
          heading="Nothing to show"
          message=${this.emptyMessage}
        ></wc-empty-state>
      `;
    }

    const columns = this.showAccount ? 7 : 6;

    return html`
      <div class="scroller" @keydown=${this.handleKeydown}>
        <table role="grid">
          <caption>
            ${this.caption}
          </caption>
          <thead>
            <tr role="row">
              <th scope="col" class="flag"><span class="sr-only">Flag</span></th>
              <th scope="col">Date</th>
              <th scope="col">Description</th>
              <th scope="col">Category</th>
              <th scope="col">Vendor</th>
              <th scope="col" class="amount">Amount</th>
              ${this.showAccount ? html`<th scope="col">Account</th>` : nothing}
            </tr>
          </thead>
          <tbody>
            ${this.rows.map((row) => this.renderRow(row))}
          </tbody>
          ${this.total === undefined && this.footerNote === ''
            ? nothing
            : html`
                <tfoot>
                  <tr role="row">
                    <td role="gridcell" colspan=${columns - 1}>
                      Net
                      ${this.footerNote
                        ? html`<span class="note">· ${this.footerNote}</span>`
                        : nothing}
                    </td>
                    <td role="gridcell" class="amount">
                      ${this.total === undefined
                        ? nothing
                        : html`<wc-money .amount=${this.total} align="end"></wc-money>`}
                    </td>
                    ${this.showAccount ? html`<td role="gridcell"></td>` : nothing}
                  </tr>
                </tfoot>
              `}
        </table>
      </div>
    `;
  }

  private renderFlagButton(row: RegisterTableRow) {
    return html`
      <button
        class="icon-button"
        type="button"
        aria-pressed=${row.isFlagged ? 'true' : 'false'}
        aria-label=${`Flag transaction ${row.id}`}
        ?disabled=${row.id === this.busyId}
        @click=${() => this.requestFlag(row)}
      >
        <wc-icon-flag></wc-icon-flag>
      </button>
    `;
  }

  /**
   * Read-only rows still have to show which transactions are flagged, and the
   * row's colour cannot be the only channel that says so.
   */
  private renderFlagMark(row: RegisterTableRow) {
    if (!row.isFlagged) return nothing;
    return html`<wc-icon-flag role="img" aria-label="Flagged"></wc-icon-flag>`;
  }

  private renderRow(row: RegisterTableRow) {
    const selected = row.id === this.activeId;
    const editing = row.id === this.editingId;

    return html`
      <tr
        role="row"
        data-id=${row.id}
        data-flagged=${row.isFlagged ? 'true' : 'false'}
        aria-selected=${selected ? 'true' : 'false'}
        aria-busy=${row.id === this.busyId ? 'true' : 'false'}
        tabindex=${selected ? '0' : '-1'}
        @focusin=${() => this.setActive(row.id)}
        @dblclick=${() => this.activateRow(row)}
      >
        <td role="gridcell" class="flag">
          ${this.readonly ? this.renderFlagMark(row) : this.renderFlagButton(row)}
        </td>
        <td role="gridcell" class="date">${row.date}</td>
        <td role="gridcell">${row.description}</td>
        ${editing
          ? this.renderEditCells(row)
          : html`
              <td role="gridcell">
                ${row.category ?? html`<span class="muted">—</span>`}
              </td>
              <td role="gridcell">${row.vendor ?? ''}</td>
            `}
        <td role="gridcell" class="amount">
          <wc-money .amount=${row.amount} align="end"></wc-money>
        </td>
        ${this.showAccount
          ? html`<td role="gridcell">
              ${editing ? this.renderEditActions() : row.accountName}
            </td>`
          : nothing}
      </tr>
    `;
  }

  private renderEditCells(row: RegisterTableRow) {
    const options = this.filteredCategories;
    const active = options[this.editOptionIndex];

    return html`
      <td role="gridcell">
        <div class="combobox">
          <input
            class="category-input"
            type="text"
            role="combobox"
            aria-label="Category"
            aria-expanded=${this.editListOpen ? 'true' : 'false'}
            aria-controls=${this.editListOpen ? 'category-options' : nothing}
            aria-autocomplete="list"
            aria-activedescendant=${this.editListOpen && active
              ? `category-option-${active.id}`
              : nothing}
            .value=${this.editCategoryQuery}
            @input=${this.handleCategoryInput}
            @keydown=${this.handleCategoryKeydown}
          />
          ${this.editListOpen
            ? html`
                <ul class="options" id="category-options" role="listbox" aria-label="Categories">
                  ${options.map(
                    (option, index) => html`
                      <li
                        id=${`category-option-${option.id}`}
                        role="option"
                        aria-selected=${index === this.editOptionIndex ? 'true' : 'false'}
                        @mousedown=${() => this.chooseCategory(option)}
                      >
                        ${categoryLabel(option)}
                      </li>
                    `,
                  )}
                </ul>
              `
            : nothing}
        </div>
      </td>
      <td role="gridcell">
        <wa-input
          class="vendor-input"
          label="Vendor"
          size="s"
          value=${row.vendor ?? ''}
          @keydown=${this.handleVendorKeydown}
        ></wa-input>
      </td>
    `;
  }

  private renderEditActions() {
    return html`
      <div class="edit-actions">
        <button type="button" @click=${() => this.commitEdit()}>Save</button>
        <button type="button" @click=${() => this.cancelEdit()}>Cancel</button>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-register-table': WcRegisterTable;
  }

  interface HTMLElementEventMap {
    'nc-row-select': CustomEvent<NcRowEventDetail>;
    'nc-row-activate': CustomEvent<NcRowEventDetail>;
    'nc-edit-commit': CustomEvent<NcEditCommitDetail>;
    'nc-edit-cancel': CustomEvent<NcRowEventDetail>;
    'nc-flag-toggle': CustomEvent<NcFlagToggleDetail>;
    'nc-search-focus': CustomEvent<void>;
  }
}
