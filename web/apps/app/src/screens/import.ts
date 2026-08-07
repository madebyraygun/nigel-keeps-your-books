import { html, type TemplateResult } from 'lit';
import '@nigel/ui';

import type { ScreenContext } from './context.js';

export function renderImport(_ctx: ScreenContext): TemplateResult {
  return html`
    <wc-empty-state
      icon="wc-icon-import"
      heading="Import"
      message="Importing bank CSV and XLSX statements arrives in task 31.14."
    ></wc-empty-state>
  `;
}
