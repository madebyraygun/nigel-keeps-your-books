import { html, type TemplateResult } from 'lit';
import '@nigel/ui';

import type { ScreenContext } from './context.js';

export function renderRegister(_ctx: ScreenContext): TemplateResult {
  return html`
    <wc-empty-state
      icon="wc-icon-register"
      heading="Register"
      message="The searchable transaction register, with inline category and vendor editing, arrives in task 31.12."
    ></wc-empty-state>
  `;
}
