import { LitElement, html, css, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import '@nigel/ui';
import {
  confirmDialog,
  dispatchNcToast,
  type ImportHistoryRow,
  type NcImportUndoDetail,
} from '@nigel/ui';

import { ApiError, type ApiClient } from '../api/index.js';
import { toImportRows, undoConfirmMessage, undoneMessage } from './undo-data.js';
import type { ScreenContext } from './context.js';
import type { ScreenId } from './registry.js';

/**
 * Undoing an import.
 *
 * A superset of `undo_manager.rs`, which can only offer the most recent
 * import because a terminal has nothing to point at. `delete_import` has
 * always taken an id, so the web lists them all and undoes the one chosen.
 */
@customElement('nigel-undo-screen')
export class NigelUndoScreen extends LitElement {
  static styles = css`
    :host {
      display: block;
      max-width: 52rem;
    }
  `;

  @property({ attribute: false })
  client!: ApiClient;

  @property({ attribute: false })
  params: URLSearchParams = new URLSearchParams();

  @property({ attribute: false })
  navigate?: (screen: ScreenId, params?: URLSearchParams) => void;

  @state() private imports: ImportHistoryRow[] = [];
  @state() private loading = true;
  @state() private error: string | null = null;
  @state() private busyId: number | null = null;

  firstUpdated(): void {
    void this.load();
  }

  private load = async (): Promise<void> => {
    this.loading = true;
    try {
      this.imports = toImportRows(await this.client.getImports());
      this.error = null;
    } catch (error) {
      this.imports = [];
      this.error =
        error instanceof ApiError
          ? error.message
          : 'Could not load the import history.';
    } finally {
      this.loading = false;
    }
  };

  private handleUndo = (event: Event): void => {
    const { id } = (event as CustomEvent<NcImportUndoDetail>).detail;
    const item = this.imports.find((candidate) => candidate.id === id);
    if (item) void this.confirmUndo(item);
  };

  private async confirmUndo(item: ImportHistoryRow): Promise<void> {
    const confirmed = await confirmDialog({
      heading: 'Undo this import?',
      message: undoConfirmMessage(item),
      confirmLabel: 'Undo import',
      variant: 'danger',
    });
    if (!confirmed) return;

    this.busyId = item.id;
    try {
      const undone = await this.client.deleteImport(item.id);
      dispatchNcToast(this, {
        message: undoneMessage(item.filename, undone),
        variant: 'success',
      });
    } catch (error) {
      // Most likely somebody undid it in another tab. Never a silent success:
      // the route answers 404 rather than reporting nothing deleted.
      dispatchNcToast(this, {
        message:
          error instanceof ApiError ? error.message : 'Could not undo that import.',
        variant: 'danger',
      });
    } finally {
      this.busyId = null;
      // Refetch either way rather than splicing: every other row's count is
      // the server's to state, and a failure means this list was already stale.
      await this.load();
    }
  }

  render() {
    return html`
      <wc-panel
        heading="Undo an import"
        description="Rolls back an import and every transaction it created. Categorizations made since are removed with them."
      >
        <wc-import-history
          .imports=${this.imports}
          ?loading=${this.loading}
          .error=${this.error}
          .busyId=${this.busyId}
          @nc-import-undo=${this.handleUndo}
          @nc-retry=${this.load}
        ></wc-import-history>
      </wc-panel>
    `;
  }
}

export function renderUndo(ctx: ScreenContext): TemplateResult {
  return html`
    <nigel-undo-screen
      .client=${ctx.client}
      .params=${ctx.params}
      .navigate=${ctx.navigate}
    ></nigel-undo-screen>
  `;
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-undo-screen': NigelUndoScreen;
  }
}
