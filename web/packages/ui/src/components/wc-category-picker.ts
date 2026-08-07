import { LitElement, html, css, nothing, type PropertyValues } from 'lit';
import { customElement, property, state, query } from 'lit/decorators.js';
import { categoryLabel, type CategoryOption } from './category-option.js';

export interface NcCategoryChangeDetail {
  categoryId: number | null;
  /** The chosen category's name, or null when the selection was cleared. */
  name: string | null;
}

/** Income before expense, which is the order `GET /api/categories` returns. */
const GROUP_ORDER = ['income', 'expense'] as const;

const GROUP_LABELS: Record<string, string> = {
  income: 'Income',
  expense: 'Expense',
};

/**
 * A searchable category picker.
 *
 * Hand-built rather than `wa-select`: this Web Awesome build ships no
 * searchable select or combobox, and the register table proved the ARIA wiring
 * (`aria-activedescendant` pointing into the option list) needs a real input
 * underneath. This is that same combobox lifted out so the review screen and
 * the register do not each grow their own — the filter is a case-insensitive
 * substring over the `Name (inc)` label, which is what the TUI filters on.
 *
 * Options are grouped income-then-expense, the grouping the TUI's category
 * chart shows, using `listbox > group > option` so a screen reader announces
 * which half of the chart of accounts an option came from.
 */
@customElement('wc-category-picker')
export class WcCategoryPicker extends LitElement {
  static styles = css`
    :host {
      display: block;
      position: relative;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    label {
      display: block;
      font-size: var(--wa-font-size-s, 13px);
      font-weight: var(--wa-font-weight-medium, 500);
      margin-bottom: var(--wa-space-2xs, 4px);
    }

    input {
      width: 100%;
      box-sizing: border-box;
      font: inherit;
      color: inherit;
      background: var(--wa-color-surface);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-m, 8px);
      padding: var(--wa-space-xs, 6px) var(--wa-space-s, 8px);
    }

    input:focus-visible {
      outline: 2px solid var(--wa-color-brand);
      outline-offset: 1px;
    }

    input[aria-invalid='true'] {
      border-color: var(--wa-color-danger, #b3261e);
    }

    .options {
      position: absolute;
      z-index: 2;
      left: 0;
      right: 0;
      margin: var(--wa-space-2xs, 4px) 0 0;
      padding: 0;
      max-height: 16rem;
      overflow-y: auto;
      background: var(--wa-color-surface);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-m, 8px);
      box-shadow: var(--wa-shadow-m, 0 4px 12px rgb(0 0 0 / 18%));
    }

    .group {
      padding: 0;
    }

    .group-label {
      display: block;
      padding: var(--wa-space-2xs, 4px) var(--wa-space-s, 8px);
      font-size: var(--wa-font-size-xs, 11px);
      text-transform: uppercase;
      letter-spacing: 0.04em;
      color: var(--wa-color-muted);
    }

    [role='option'] {
      padding: var(--wa-space-2xs, 4px) var(--wa-space-s, 8px);
      cursor: pointer;
    }

    [role='option'][aria-selected='true'] {
      background: var(--wa-color-brand);
      color: var(--wa-color-on-brand, #fff);
    }

    .no-matches {
      padding: var(--wa-space-xs, 6px) var(--wa-space-s, 8px);
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
    }
  `;

  @property({ attribute: false })
  options: CategoryOption[] = [];

  /** The selected category id, or null. Settable from outside to reset. */
  @property({ type: Number })
  value: number | null = null;

  @property({ type: String })
  label = 'Category';

  @property({ type: String })
  placeholder = 'Type to filter…';

  @property({ type: Boolean })
  disabled = false;

  /** Draws the invalid state; the surrounding form decides what to do about it. */
  @property({ type: Boolean })
  invalid = false;

  /**
   * The typed filter and whether the list is showing.
   *
   * Public rather than internal state so every visual state is reachable
   * declaratively — the previews and their axe runs need an open list, and
   * poking at private fields to get one is how a preview stops matching what
   * the component actually does.
   */
  @property({ attribute: false })
  queryText = '';

  @property({ attribute: false })
  listOpen = false;

  @state() private optionIndex = 0;

  @query('input') private input?: HTMLInputElement;

  willUpdate(changed: PropertyValues<this>): void {
    // A value set from outside — a reset, or a form seeding a re-review —
    // owns the text, so the box always reads back what is actually selected.
    if (changed.has('value') && this.value !== null) {
      const chosen = this.options.find((option) => option.id === this.value);
      if (chosen) this.queryText = chosen.name;
    }
    if (changed.has('value') && this.value === null && !this.listOpen) {
      this.queryText = '';
    }
  }

  /** Case-insensitive substring over the `Name (inc)` label, as the TUI does. */
  get filtered(): CategoryOption[] {
    const needle = this.queryText.trim().toLowerCase();
    if (needle === '') return this.options;
    return this.options.filter((option) =>
      categoryLabel(option).toLowerCase().includes(needle),
    );
  }

  /** Put the cursor in the filter box — what the screen calls on a new transaction. */
  override focus(): void {
    this.input?.focus();
  }

  /** Clear the selection and the typed filter. */
  reset(): void {
    this.value = null;
    this.queryText = '';
    this.listOpen = false;
    this.optionIndex = 0;
  }

  private choose(option: CategoryOption | undefined): void {
    if (!option) return;
    this.value = option.id;
    this.queryText = option.name;
    this.listOpen = false;
    this.optionIndex = 0;
    this.dispatchEvent(
      new CustomEvent<NcCategoryChangeDetail>('nc-category-change', {
        detail: { categoryId: option.id, name: option.name },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private handleInput(event: Event): void {
    this.queryText = (event.target as HTMLInputElement).value;
    this.listOpen = true;
    this.optionIndex = 0;

    // Typing past a selection un-selects it: the box no longer reads back what
    // would be submitted, and a stale id is worse than none.
    if (this.value !== null) {
      this.value = null;
      this.dispatchEvent(
        new CustomEvent<NcCategoryChangeDetail>('nc-category-change', {
          detail: { categoryId: null, name: null },
          bubbles: true,
          composed: true,
        }),
      );
    }
  }

  private handleKeydown(event: KeyboardEvent): void {
    const options = this.ordered;

    switch (event.key) {
      case 'ArrowDown':
        this.listOpen = true;
        this.optionIndex = Math.min(this.optionIndex + 1, options.length - 1);
        break;
      case 'ArrowUp':
        this.listOpen = true;
        this.optionIndex = Math.max(this.optionIndex - 1, 0);
        break;
      case 'Home':
        if (!this.listOpen) return;
        this.optionIndex = 0;
        break;
      case 'End':
        if (!this.listOpen) return;
        this.optionIndex = Math.max(options.length - 1, 0);
        break;
      case 'Enter':
        // With the list open Enter picks; with it closed Enter belongs to the
        // form, so the review screen's "Enter applies" still works.
        if (!this.listOpen) return;
        this.choose(options[this.optionIndex]);
        break;
      case 'Escape':
        // Only swallow Escape when it has a list to close — otherwise it is
        // the review screen's Back key and must reach the host.
        if (!this.listOpen) return;
        this.listOpen = false;
        break;
      default:
        return;
    }

    event.preventDefault();
    event.stopPropagation();
  }

  private handleBlur = (): void => {
    this.listOpen = false;
  };

  private handleFocus = (): void => {
    if (!this.disabled) this.listOpen = true;
  };

  /**
   * The filtered options in the order they are drawn.
   *
   * Arrow keys walk this rather than `filtered`, so the highlight moves down
   * the list the user can see even if the server ever returns the two halves
   * of the chart of accounts interleaved.
   */
  private get ordered(): CategoryOption[] {
    return this.groups.flatMap((group) => group.options);
  }

  private get groups(): { type: string; options: CategoryOption[] }[] {
    const filtered = this.filtered;
    const seen = [
      ...GROUP_ORDER,
      ...filtered.map((option) => option.categoryType),
    ].filter((type, index, all) => all.indexOf(type) === index);

    return seen
      .map((type) => ({
        type,
        options: filtered.filter((option) => option.categoryType === type),
      }))
      .filter((group) => group.options.length > 0);
  }

  render() {
    const ordered = this.ordered;
    const active = ordered[this.optionIndex];
    const listId = 'category-options';
    // An empty list is not an expanded one: there is nothing to walk, and a
    // listbox with no options is an ARIA violation rather than a nicety.
    const expanded = this.listOpen && ordered.length > 0;

    return html`
      <label for="category-filter">${this.label}</label>
      <input
        id="category-filter"
        type="text"
        role="combobox"
        autocomplete="off"
        placeholder=${this.placeholder}
        ?disabled=${this.disabled}
        aria-expanded=${expanded ? 'true' : 'false'}
        aria-controls=${expanded ? listId : nothing}
        aria-autocomplete="list"
        aria-invalid=${this.invalid ? 'true' : 'false'}
        aria-activedescendant=${expanded && active
          ? `category-option-${active.id}`
          : nothing}
        .value=${this.queryText}
        @input=${this.handleInput}
        @keydown=${this.handleKeydown}
        @focus=${this.handleFocus}
        @blur=${this.handleBlur}
      />
      ${this.listOpen ? this.renderList(listId, ordered) : nothing}
    `;
  }

  private renderList(listId: string, ordered: CategoryOption[]) {
    if (ordered.length === 0) {
      return html`
        <div class="options no-matches" role="status">No matching categories</div>
      `;
    }

    let flatIndex = -1;

    return html`
      <div class="options" id=${listId} role="listbox" aria-label=${this.label}>
        ${this.groups.map(
          (group) => html`
            <div
              class="group"
              role="group"
              aria-label=${GROUP_LABELS[group.type] ?? group.type}
            >
              <span class="group-label" aria-hidden="true"
                >${GROUP_LABELS[group.type] ?? group.type}</span
              >
              ${group.options.map((option) => {
                flatIndex += 1;
                const index = flatIndex;
                return html`
                  <div
                    id=${`category-option-${option.id}`}
                    role="option"
                    aria-selected=${index === this.optionIndex ? 'true' : 'false'}
                    @mousedown=${(event: MouseEvent) => {
                      // mousedown, not click: blur would close the list first.
                      event.preventDefault();
                      this.choose(option);
                    }}
                  >
                    ${categoryLabel(option)}
                  </div>
                `;
              })}
            </div>
          `,
        )}
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-category-picker': WcCategoryPicker;
  }
}
