import { describe, it, expect, afterEach } from 'vitest';
import './wc-invoice-preview.js';
import { PREVIEW_SANDBOX, type WcInvoicePreview } from './wc-invoice-preview.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-invoice-preview.preview.js';

async function mount(props: Partial<WcInvoicePreview> = {}): Promise<WcInvoicePreview> {
  const el = document.createElement('wc-invoice-preview');
  Object.assign(
    el,
    { src: 'about:blank', pdfSrc: 'about:blank#pdf' },
    props,
  );
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-invoice-preview', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('starts collapsed and frames nothing until it is opened', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('details')?.open).toBe(false);
    expect(el.shadowRoot?.querySelector('[data-frame]')).toBeNull();
  });

  it('frames the page once opened', async () => {
    const el = await mount({ open: true });
    const frame = el.shadowRoot?.querySelector('[data-frame]');
    expect(frame).toBeTruthy();
    expect(frame?.getAttribute('src')).toBe('about:blank');
    expect(frame?.getAttribute('title')).toBe('Invoice preview');
  });

  it('never grants the frame allow-same-origin', async () => {
    // The document is served from the SPA's own origin; allow-same-origin
    // would hand a page rendered from invoice data the app's cookies and
    // storage, and there is nothing the preview needs them for.
    expect(PREVIEW_SANDBOX).not.toContain('allow-same-origin');
    expect(PREVIEW_SANDBOX).not.toContain('allow-scripts');

    const el = await mount({ open: true });
    const sandbox = el.shadowRoot?.querySelector('[data-frame]')?.getAttribute('sandbox');
    expect(sandbox).toBe(PREVIEW_SANDBOX);
  });

  it('names the unset keys without naming their values', async () => {
    const el = await mount({ open: true, missing: ['r2_bucket', 'public_base_url'] });
    const notice = el.shadowRoot?.querySelector('[data-missing]');
    expect(notice?.getAttribute('message')).toContain('r2_bucket, public_base_url');
    expect(notice?.getAttribute('message')).toContain('cannot be sent');
  });

  it('offers no PDF link on a build that cannot render one', async () => {
    const el = await mount({ open: true, pdfAvailable: false });
    expect(el.shadowRoot?.querySelector('[data-pdf-link]')).toBeNull();
    expect(el.shadowRoot?.querySelector('[data-pdf-unavailable]')?.textContent).toContain(
      'not available',
    );
  });

  it('offers both addresses when the build can render a PDF', async () => {
    const el = await mount({ open: true });
    expect(el.shadowRoot?.querySelector('[data-html-link]')).toBeTruthy();
    expect(el.shadowRoot?.querySelector('[data-pdf-link]')?.getAttribute('href')).toBe(
      'about:blank#pdf',
    );
  });
});

describePreviewA11y(preview);
