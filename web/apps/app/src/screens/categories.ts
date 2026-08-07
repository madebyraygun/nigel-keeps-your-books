import { LitElement, html, css, nothing, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import '@nigel/ui';
import {
  confirmDialog,
  EMPTY_CATEGORY_FORM,
  formLineSuggestions,
  validateCategoryForm,
  type CategoryFormErrors,
  type CategoryFormValue,
  type ManagerAction,
  type ManagerColumn,
  type ManagerRow,
  type NcCategoryFormChangeDetail,
  type NcManagerActionDetail,
} from '@nigel/ui';

import { ApiError, type ApiClient } from '../api/index.js';
import type { CategoryRow } from '../api/types.js';
import {
  categoryPatch,
  isEmptyPatch,
  newCategoryRequest,
  toCategoryForm,
} from './categories-data.js';
import { guardrailAction, guardrailMessage } from './manager-errors.js';
import type { ScreenContext } from './context.js';
import type { ScreenId } from './registry.js';

const COLUMNS: ManagerColumn[] = [
  { key: 'name', label: 'Name' },
  { key: 'categoryType', label: 'Type' },
  { key: 'taxLine', label: 'Tax line' },
  { key: 'formLine', label: 'Form line', mono: true },
];

const ACTIONS: ManagerAction[] = [
  { name: 'edit', label: 'Edit', icon: 'wc-icon-edit' },
  { name: 'delete', label: 'Delete', icon: 'wc-icon-trash', variant: 'danger' },
];

interface Editor {
  /** The category being edited; absent when creating. */
  id?: number;
  value: CategoryFormValue;
}

/**
 * The chart of accounts — `category_manager.rs` on the web, with every field
 * the TUI collects and the K-1 form-line vocabulary spelled out beside the one
 * that needs it.
 *
 * Rows stay in the order the server sends them: income first, then alphabetical.
 * That is the reading order the whole app uses for categories, and re-sorting
 * here would make this the one screen where the chart looks different.
 */
@customElement('nigel-categories-screen')
export class NigelCategoriesScreen extends LitElement {
  static styles = css`
    :host {
      display: block;
    }
  `;

  @property({ attribute: false })
  client!: ApiClient;

  @property({ attribute: false })
  navigate?: (screen: ScreenId, params?: URLSearchParams) => void;

  @state() private categories: CategoryRow[] = [];
  @state() private loading = true;
  @state() private error: string | null = null;
  @state() private editor: Editor | null = null;
  @state() private formErrors: CategoryFormErrors = {};
  @state() private saving = false;
  @state() private dialogError: string | null = null;
  @state() private busyId: number | null = null;

  /** What the error region's button does, when it has one. */
  private errorAction: { label: string; run: () => void } | null = null;

  firstUpdated(): void {
    void this.load();
  }

  private async load(): Promise<void> {
    this.loading = true;
    try {
      this.categories = await this.client.getCategories();
      this.error = null;
      this.errorAction = null;
    } catch (error) {
      this.categories = [];
      this.error =
        error instanceof ApiError
          ? error.message
          : 'Could not load the chart of accounts.';
      this.errorAction = { label: 'Try again', run: () => void this.load() };
    } finally {
      this.loading = false;
    }
  }

  private get rows(): ManagerRow[] {
    return this.categories.map((category) => ({
      id: category.id,
      label: category.name,
      cells: [
        category.name,
        category.categoryType === 'income' ? 'Income' : 'Expense',
        category.taxLine,
        category.formLine,
      ],
    }));
  }

  // -- editing --------------------------------------------------------------

  private openCreate = (): void => {
    this.editor = { value: EMPTY_CATEGORY_FORM };
    this.formErrors = {};
    this.dialogError = null;
  };

  private closeEditor = (): void => {
    this.editor = null;
    this.formErrors = {};
    this.dialogError = null;
  };

  private handleFormChange = (event: Event): void => {
    if (!this.editor) return;
    const detail = (event as CustomEvent<NcCategoryFormChangeDetail>).detail;
    this.editor = { ...this.editor, value: detail.value };
  };

  private handleAction = (event: Event): void => {
    const { action, id } = (event as CustomEvent<NcManagerActionDetail>).detail;
    const category = this.categories.find((candidate) => candidate.id === id);
    if (!category) return;

    if (action === 'edit') {
      this.editor = { id, value: toCategoryForm(category) };
      this.formErrors = {};
      this.dialogError = null;
      return;
    }
    if (action === 'delete') void this.confirmDelete(category);
  };

  private handleSave = async (): Promise<void> => {
    const editor = this.editor;
    if (!editor || this.saving) return;

    const errors = validateCategoryForm(editor.value);
    this.formErrors = errors;
    if (Object.keys(errors).length > 0) return;

    const current =
      editor.id === undefined
        ? undefined
        : this.categories.find((category) => category.id === editor.id);

    if (current) {
      const patch = categoryPatch(current, editor.value);
      if (isEmptyPatch(patch)) {
        this.closeEditor();
        return;
      }
      await this.send(() => this.client.updateCategory(current.id, patch));
      return;
    }

    await this.send(() => this.client.createCategory(newCategoryRequest(editor.value)));
  };

  private async send(request: () => Promise<unknown>): Promise<void> {
    this.saving = true;
    this.dialogError = null;
    try {
      await request();
      this.closeEditor();
      await this.load();
    } catch (error) {
      this.dialogError = guardrailMessage(error, 'category');
    } finally {
      this.saving = false;
    }
  }

  // -- deleting -------------------------------------------------------------

  private async confirmDelete(category: CategoryRow): Promise<void> {
    const confirmed = await confirmDialog({
      heading: 'Delete category',
      message: `Delete “${category.name}”? It is removed from the chart of accounts and can no longer be assigned.`,
      confirmLabel: 'Delete',
      variant: 'danger',
    });
    if (!confirmed) return;

    this.busyId = category.id;
    try {
      await this.client.deleteCategory(category.id);
      this.error = null;
      this.errorAction = null;
      await this.load();
    } catch (error) {
      this.error = guardrailMessage(error, 'category');

      // "3 active rules assign this category" is a dead end without somewhere
      // to go; the rules screen can filter itself down to exactly those three.
      const action = guardrailAction(error, category.id);
      this.errorAction = action
        ? {
            label: action.label,
            run: () => this.navigate?.('rules', action.params),
          }
        : null;
    } finally {
      this.busyId = null;
    }
  }

  private handleErrorAction = (): void => {
    this.errorAction?.run();
  };

  private handleErrorDismiss = (): void => {
    this.error = null;
    this.errorAction = null;
  };

  // -- rendering ------------------------------------------------------------

  render() {
    const empty = !this.loading && this.categories.length === 0;

    return html`
      <wc-manager-layout
        heading="Chart of Accounts"
        description="Categories map transactions onto Schedule C and Form 1120-S lines."
        .count=${this.loading ? null : this.categories.length}
        add-label="Add category"
        ?busy=${this.loading}
        ?empty=${empty}
        .error=${this.error}
        error-action-label=${this.errorAction?.label ?? ''}
        @nc-manager-add=${this.openCreate}
        @nc-manager-error-action=${this.handleErrorAction}
        @nc-manager-error-dismiss=${this.handleErrorDismiss}
      >
        <wc-manager-table
          caption="Chart of accounts"
          .columns=${COLUMNS}
          .rows=${this.rows}
          .actions=${ACTIONS}
          .busyId=${this.busyId}
          @nc-manager-action=${this.handleAction}
        ></wc-manager-table>

        <wc-empty-state
          slot="empty"
          icon="wc-icon-category"
          heading="No categories"
          message="The chart of accounts is empty — add one to start categorizing."
        ></wc-empty-state>

        ${this.renderEditor()}
      </wc-manager-layout>
    `;
  }

  private renderEditor(): TemplateResult | typeof nothing {
    const editor = this.editor;
    if (!editor) return nothing;

    const creating = editor.id === undefined;

    return html`
      <wc-manager-dialog
        slot="overlay"
        open
        heading=${creating ? 'Add category' : 'Edit category'}
        confirm-label=${creating ? 'Add category' : 'Save'}
        ?busy=${this.saving}
        .error=${this.dialogError}
        @nc-manager-save=${this.handleSave}
        @nc-manager-cancel=${this.closeEditor}
      >
        <wc-category-form
          .value=${editor.value}
          .errors=${this.formErrors}
          .suggestions=${formLineSuggestions(
            this.categories.map((category) => category.formLine),
          )}
          ?disabled=${this.saving}
          @nc-category-form-change=${this.handleFormChange}
        ></wc-category-form>
      </wc-manager-dialog>
    `;
  }
}

export function renderCategories(ctx: ScreenContext): TemplateResult {
  return html`
    <nigel-categories-screen
      .client=${ctx.client}
      .navigate=${ctx.navigate}
    ></nigel-categories-screen>
  `;
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-categories-screen': NigelCategoriesScreen;
  }
}
