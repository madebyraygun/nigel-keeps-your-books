import { LitElement, html, css, nothing } from 'lit';
import { customElement, property, state, query } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/format-bytes/format-bytes.js';
import '../icons/icons.js';

export interface NcFileSelectDetail {
  file: File;
}

export interface NcFileErrorDetail {
  message: string;
}

/** 25 MB — what `POST /api/imports/upload` accepts before answering 413. */
export const DEFAULT_MAX_BYTES = 25 * 1024 * 1024;

/**
 * A file well: drag a statement onto it, or click to open the picker.
 *
 * Custom rather than a Web Awesome primitive because there is no `wa-file`.
 * The well itself is a `<button>`, so the keyboard path and the mouse path are
 * the same one — a `<div>` with a `tabindex` and a keydown handler would be
 * reimplementing a button badly.
 *
 * It checks the extension and the size before emitting, which duplicates
 * checks the server also makes. That duplication is deliberate: the server can
 * only answer 400 or 413 after the whole file has crossed the wire, and making
 * someone wait out a 30 MB upload to be told the limit is 25 MB is a poor way
 * to say so.
 */
@customElement('wc-dropzone')
export class WcDropzone extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .well {
      display: grid;
      justify-items: center;
      gap: var(--wa-space-2xs, 4px);
      width: 100%;
      padding: var(--wa-space-2xl, 32px) var(--wa-space-l, 16px);
      font: inherit;
      color: inherit;
      text-align: center;
      background: var(--wa-color-surface);
      border: 2px dashed var(--wa-color-border);
      border-radius: var(--wa-radius-l, 12px);
      cursor: pointer;
    }

    .well:hover:not(:disabled) {
      border-color: var(--wa-color-brand);
    }

    .well:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 2px;
    }

    .well:disabled {
      cursor: default;
      opacity: 0.6;
    }

    .zone.dragover .well {
      border-color: var(--wa-color-brand);
      border-style: solid;
      background: var(--wa-color-surface-raised, var(--wa-color-surface));
    }

    .icon {
      --nc-icon-size: 28px;
      color: var(--wa-color-muted);
    }

    .prompt {
      font-weight: var(--wa-font-weight-medium, 500);
    }

    .hint {
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
    }

    .selected {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: var(--wa-space-s, 8px);
      padding: var(--wa-space-m, 12px);
      background: var(--wa-color-surface);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-l, 12px);
    }

    .filename {
      font-weight: var(--wa-font-weight-medium, 500);
      overflow-wrap: anywhere;
    }

    .size {
      color: var(--wa-color-muted);
      font-size: var(--wa-font-size-s, 13px);
    }

    .spacer {
      flex: 1 1 auto;
    }

    .replace {
      font: inherit;
      font-size: var(--wa-font-size-s, 13px);
      padding: var(--wa-space-2xs, 4px) var(--wa-space-s, 8px);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-sm, 6px);
      background: none;
      color: inherit;
      cursor: pointer;
    }

    .replace:hover:not(:disabled) {
      border-color: var(--wa-color-brand);
    }

    .replace:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 2px;
    }

    .replace:disabled {
      cursor: default;
      opacity: 0.6;
    }

    .error {
      margin: var(--wa-space-xs, 6px) 0 0;
      color: var(--wa-color-danger);
      font-size: var(--wa-font-size-s, 13px);
    }

    input[type='file'] {
      position: absolute;
      width: 1px;
      height: 1px;
      padding: 0;
      margin: -1px;
      overflow: hidden;
      clip: rect(0 0 0 0);
      white-space: nowrap;
      border: 0;
    }
  `;

  /** Extensions the picker offers and the well accepts, comma separated. */
  @property({ type: String })
  accept = '.csv,.xlsx,.xls';

  /** Largest file to emit. Zero disables the check. */
  @property({ type: Number, attribute: 'max-bytes' })
  maxBytes = DEFAULT_MAX_BYTES;

  /** The selected file's name, or empty for none. */
  @property({ type: String })
  filename = '';

  @property({ type: Number })
  size = 0;

  /** Shown under the well. Set by the owner, including from a failed upload. */
  @property({ type: String })
  error = '';

  @property({ type: Boolean, reflect: true })
  busy = false;

  @property({ type: Boolean, reflect: true })
  disabled = false;

  /** Purely visual, so it stays internal rather than becoming a property. */
  @state() private dragover = false;

  @query('input[type="file"]') private input?: HTMLInputElement;

  private get blocked(): boolean {
    return this.disabled || this.busy;
  }

  private get extensions(): string[] {
    return this.accept
      .split(',')
      .map((part) => part.trim().toLowerCase())
      .filter((part) => part.length > 0);
  }

  /** The reason this file cannot be used, or null if it can. */
  private reject(file: File): string | null {
    const extensions = this.extensions;
    const name = file.name.toLowerCase();
    if (extensions.length > 0 && !extensions.some((ext) => name.endsWith(ext))) {
      return `nigel reads ${extensions.join(', ')} statements. That one is something else.`;
    }
    if (this.maxBytes > 0 && file.size > this.maxBytes) {
      const limit = Math.round(this.maxBytes / (1024 * 1024));
      return `That file is over the ${limit} MB limit.`;
    }
    return null;
  }

  private offer(file: File | undefined): void {
    if (!file || this.blocked) return;

    const reason = this.reject(file);
    if (reason !== null) {
      this.dispatchEvent(
        new CustomEvent<NcFileErrorDetail>('nc-file-error', {
          detail: { message: reason },
          bubbles: true,
          composed: true,
        }),
      );
      return;
    }

    this.dispatchEvent(
      new CustomEvent<NcFileSelectDetail>('nc-file-select', {
        detail: { file },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private handleBrowse = (): void => {
    if (this.blocked) return;
    this.input?.click();
  };

  private handleInputChange = (event: Event): void => {
    const input = event.target as HTMLInputElement;
    this.offer(input.files?.[0]);
    // Let the same file be chosen twice in a row; without this a re-pick after
    // a failed upload is silent, because `change` never fires for an equal value.
    input.value = '';
  };

  private handleDragOver = (event: DragEvent): void => {
    if (this.blocked) return;
    event.preventDefault();
    this.dragover = true;
  };

  private handleDragLeave = (): void => {
    this.dragover = false;
  };

  private handleDrop = (event: DragEvent): void => {
    event.preventDefault();
    this.dragover = false;
    if (this.blocked) return;
    this.offer(event.dataTransfer?.files?.[0]);
  };

  private handleClear = (): void => {
    if (this.blocked) return;
    this.dispatchEvent(
      new CustomEvent('nc-file-clear', { bubbles: true, composed: true }),
    );
  };

  render() {
    return html`
      <div
        class="zone ${this.dragover ? 'dragover' : ''}"
        @dragover=${this.handleDragOver}
        @dragleave=${this.handleDragLeave}
        @drop=${this.handleDrop}
      >
        ${this.filename ? this.renderSelected() : this.renderWell()}
      </div>
      <input
        type="file"
        accept=${this.accept}
        tabindex="-1"
        aria-hidden="true"
        @change=${this.handleInputChange}
      />
      ${this.error
        ? html`<p class="error" role="alert">${this.error}</p>`
        : nothing}
    `;
  }

  private renderWell() {
    return html`
      <button
        type="button"
        class="well"
        ?disabled=${this.blocked}
        @click=${this.handleBrowse}
      >
        <wc-icon-import class="icon"></wc-icon-import>
        <span class="prompt">Drop a statement here, or choose a file</span>
        <span class="hint">${this.extensions.join(', ')}</span>
      </button>
    `;
  }

  private renderSelected() {
    return html`
      <div class="selected">
        <wc-icon-import class="icon"></wc-icon-import>
        <span class="filename" role="status">${this.filename}</span>
        ${this.size > 0
          ? html`<wa-format-bytes class="size" .value=${this.size}></wa-format-bytes>`
          : nothing}
        <span class="spacer"></span>
        <button
          type="button"
          class="replace"
          ?disabled=${this.blocked}
          @click=${this.handleBrowse}
        >
          Choose a different file
        </button>
        <button
          type="button"
          class="replace"
          ?disabled=${this.blocked}
          @click=${this.handleClear}
        >
          Remove
        </button>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-dropzone': WcDropzone;
  }

  interface HTMLElementEventMap {
    'nc-file-select': CustomEvent<NcFileSelectDetail>;
    'nc-file-error': CustomEvent<NcFileErrorDetail>;
  }
}
