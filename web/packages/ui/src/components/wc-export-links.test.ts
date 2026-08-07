import { describe, it, expect, afterEach } from 'vitest';
import './wc-export-links.js';
import type { WcExportLinks } from './wc-export-links.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-export-links.preview.js';

const TEXT = '/api/exports/pnl?format=text&year=2025';
const PDF = '/api/exports/pnl?format=pdf&year=2025';

async function mount(props: Partial<WcExportLinks> = {}): Promise<WcExportLinks> {
  const el = document.createElement('wc-export-links');
  Object.assign(el, { textHref: TEXT, pdfHref: PDF, ...props });
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function all<T extends Element>(el: WcExportLinks, selector: string): T[] {
  return [...(el.shadowRoot?.querySelectorAll<T>(selector) ?? [])];
}

function query<T extends Element>(el: WcExportLinks, selector: string): T | null {
  return el.shadowRoot?.querySelector<T>(selector) ?? null;
}

describe('wc-export-links', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders both formats as download links', async () => {
    const el = await mount();
    const links = all<HTMLAnchorElement>(el, 'a');
    expect(links.map((a) => a.getAttribute('href'))).toEqual([TEXT, PDF]);
    // Without `download` the browser would navigate away from the app on a
    // response that arrived without Content-Disposition.
    expect(links.every((a) => a.hasAttribute('download'))).toBe(true);
  });

  it('labels the links by format', async () => {
    const el = await mount();
    expect(all(el, 'a').map((a) => a.textContent?.trim())).toEqual(['Text', 'PDF']);
  });

  it('replaces the pdf link with a disabled control when pdf is unavailable', async () => {
    const el = await mount({ pdfAvailable: false });
    expect(all(el, 'a')).toHaveLength(1);
    const button = query<HTMLButtonElement>(el, 'button');
    expect(button?.disabled).toBe(true);
    expect(button?.textContent?.trim()).toBe('PDF');
  });

  it('states why pdf is unavailable, in text, described by the control', async () => {
    const el = await mount({ pdfAvailable: false });
    const button = query<HTMLButtonElement>(el, 'button');
    const reason = query(el, '.reason');
    // A title-only tooltip would be invisible to a keyboard and to most
    // screen readers; the reason has to be readable text.
    expect(reason?.textContent).toContain('without PDF export');
    expect(button?.getAttribute('aria-describedby')).toBe(reason?.id);
  });

  it('keeps text export available on a build without pdf', async () => {
    const el = await mount({ pdfAvailable: false });
    expect(query<HTMLAnchorElement>(el, 'a')?.getAttribute('href')).toBe(TEXT);
  });

  it('honours a custom unavailable reason', async () => {
    const el = await mount({ pdfAvailable: false, pdfUnavailableReason: 'Not today.' });
    expect(query(el, '.reason')?.textContent?.trim()).toBe('Not today.');
  });

  it('blocks a click while busy', async () => {
    const el = await mount({ busy: true });
    const link = query<HTMLAnchorElement>(el, 'a');
    const event = new MouseEvent('click', { cancelable: true, bubbles: true });
    link?.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    expect(link?.getAttribute('aria-disabled')).toBe('true');
  });

  it('lets a click through when not busy', async () => {
    const el = await mount();
    const link = query<HTMLAnchorElement>(el, 'a');
    const event = new MouseEvent('click', { cancelable: true, bubbles: true });
    link?.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
  });
});

describePreviewA11y(preview);
