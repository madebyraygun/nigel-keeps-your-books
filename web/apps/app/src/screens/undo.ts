import { html, type TemplateResult } from 'lit';
import '@nigel/ui';

import type { ScreenContext } from './context.js';

export function renderUndo(_ctx: ScreenContext): TemplateResult {
  return html`
    <wc-empty-state
      icon="wc-icon-undo"
      heading="Undo last import"
      message="Undoing the most recent import arrives in task 31.17."
    ></wc-empty-state>
  `;
}
