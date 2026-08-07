import { describe, it, expect, afterEach, vi } from 'vitest';
import './wc-dropzone.js';
import type { WcDropzone } from './wc-dropzone.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-dropzone.preview.js';

async function mount(props: Partial<WcDropzone> = {}): Promise<WcDropzone> {
  const el = document.createElement('wc-dropzone');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function file(name: string, size = 10): File {
  const f = new File(['x'], name);
  // jsdom computes size from the parts; the oversize cases need a stated size
  // rather than 30 MB of actual string.
  Object.defineProperty(f, 'size', { value: size });
  return f;
}

/**
 * jsdom has no `DataTransfer` constructor, so a drop event is assembled by
 * hand. This is the only way to exercise the drop path at all.
 */
function drop(el: WcDropzone, files: File[]): void {
  const event = new Event('drop', { bubbles: true, cancelable: true });
  Object.defineProperty(event, 'dataTransfer', { value: { files } });
  el.shadowRoot?.querySelector('.zone')?.dispatchEvent(event);
}

function dragOver(el: WcDropzone): void {
  const event = new Event('dragover', { bubbles: true, cancelable: true });
  Object.defineProperty(event, 'dataTransfer', { value: { files: [] } });
  el.shadowRoot?.querySelector('.zone')?.dispatchEvent(event);
}

describe('wc-dropzone', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('emits the dropped file', async () => {
    const el = await mount();
    const seen = vi.fn();
    el.addEventListener('nc-file-select', (e) => seen((e as CustomEvent).detail.file));

    const dropped = file('april-2025.csv');
    drop(el, [dropped]);

    expect(seen).toHaveBeenCalledOnce();
    expect(seen.mock.calls[0][0]).toBe(dropped);
  });

  it('opens the picker when the well is clicked', async () => {
    const el = await mount();
    const input = el.shadowRoot?.querySelector('input[type="file"]') as HTMLInputElement;
    const click = vi.spyOn(input, 'click');

    (el.shadowRoot?.querySelector('.well') as HTMLButtonElement).click();

    expect(click).toHaveBeenCalledOnce();
  });

  it('refuses an extension it cannot read', async () => {
    const el = await mount();
    const selected = vi.fn();
    const failed = vi.fn();
    el.addEventListener('nc-file-select', selected);
    el.addEventListener('nc-file-error', (e) =>
      failed((e as CustomEvent).detail.message),
    );

    drop(el, [file('notes.txt')]);

    expect(selected).not.toHaveBeenCalled();
    expect(failed.mock.calls[0][0]).toContain('.csv');
  });

  it('refuses a file over the size limit', async () => {
    const el = await mount();
    const selected = vi.fn();
    const failed = vi.fn();
    el.addEventListener('nc-file-select', selected);
    el.addEventListener('nc-file-error', (e) =>
      failed((e as CustomEvent).detail.message),
    );

    drop(el, [file('huge.csv', 30 * 1024 * 1024)]);

    expect(selected).not.toHaveBeenCalled();
    expect(failed.mock.calls[0][0]).toContain('25 MB');
  });

  it('accepts a file exactly at the limit', async () => {
    const el = await mount();
    const selected = vi.fn();
    el.addEventListener('nc-file-select', selected);

    drop(el, [file('exact.csv', 25 * 1024 * 1024)]);

    expect(selected).toHaveBeenCalledOnce();
  });

  it('matches the extension regardless of case', async () => {
    const el = await mount();
    const selected = vi.fn();
    el.addEventListener('nc-file-select', selected);

    drop(el, [file('APRIL.XLSX')]);

    expect(selected).toHaveBeenCalledOnce();
  });

  it('marks the zone while a file is over it, and unmarks on leave', async () => {
    const el = await mount();

    dragOver(el);
    await el.updateComplete;
    expect(el.shadowRoot?.querySelector('.zone')?.classList.contains('dragover')).toBe(
      true,
    );

    el.shadowRoot
      ?.querySelector('.zone')
      ?.dispatchEvent(new Event('dragleave', { bubbles: true }));
    await el.updateComplete;
    expect(el.shadowRoot?.querySelector('.zone')?.classList.contains('dragover')).toBe(
      false,
    );
  });

  it('shows the filename and size once one is chosen', async () => {
    const el = await mount({ filename: 'april-2025.csv', size: 8214 });

    expect(el.shadowRoot?.querySelector('.filename')?.textContent).toBe(
      'april-2025.csv',
    );
    const bytes = el.shadowRoot?.querySelector('wa-format-bytes') as HTMLElement & {
      value: number;
    };
    expect(bytes.value).toBe(8214);
    expect(el.shadowRoot?.querySelector('.well')).toBeNull();
  });

  it('emits nc-file-clear from Remove', async () => {
    const el = await mount({ filename: 'april-2025.csv', size: 8214 });
    const cleared = vi.fn();
    el.addEventListener('nc-file-clear', cleared);

    const buttons = [...(el.shadowRoot?.querySelectorAll('.replace') ?? [])];
    (buttons.at(-1) as HTMLButtonElement).click();

    expect(cleared).toHaveBeenCalledOnce();
  });

  it('ignores drops while busy or disabled', async () => {
    for (const props of [{ busy: true }, { disabled: true }]) {
      const el = await mount(props);
      const selected = vi.fn();
      el.addEventListener('nc-file-select', selected);

      drop(el, [file('april.csv')]);

      expect(selected, JSON.stringify(props)).not.toHaveBeenCalled();
      el.remove();
    }
  });

  it('renders the error as an alert', async () => {
    const el = await mount({ error: 'That file is over the 25 MB limit.' });
    const error = el.shadowRoot?.querySelector('.error');
    expect(error?.getAttribute('role')).toBe('alert');
    expect(error?.textContent).toContain('25 MB');
  });
});

describePreviewA11y(preview);
