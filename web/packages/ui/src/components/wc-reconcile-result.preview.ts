import { html } from 'lit';
import './wc-reconcile-result.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-reconcile-result',
  title: 'Reconcile result',
  group: 'Data',
  description:
    'The two verdicts a reconciliation comes back with. The difference gets its own emphasised row rather than relying on the red, which is a second channel and not the only one.',
  layout: 'stack',
  states: [
    {
      name: 'reconciled',
      render: () => html`
        <wc-reconcile-result
          account="BofA Checking"
          month="2025-02"
          is-reconciled
          .statementBalance=${4928.01}
          .calculatedBalance=${4928.01}
          .discrepancy=${0}
        ></wc-reconcile-result>
      `,
    },
    {
      name: 'discrepancy',
      render: () => html`
        <wc-reconcile-result
          account="BofA Checking"
          month="2025-03"
          .statementBalance=${5000}
          .calculatedBalance=${4871.44}
          .discrepancy=${128.56}
        ></wc-reconcile-result>
      `,
    },
  ],
};

export default preview;
