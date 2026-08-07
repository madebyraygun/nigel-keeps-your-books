import { html, type TemplateResult } from 'lit';
import '@nigel/ui';

import type { ScreenContext } from './context.js';

export function renderReports(_ctx: ScreenContext): TemplateResult {
  return html`
    <wc-empty-state
      icon="wc-icon-report"
      heading="Reports"
      message="Profit and loss, expenses, tax, cash flow, balance, flagged, register, and the K-1 worksheet arrive in task 31.15."
    ></wc-empty-state>
  `;
}
