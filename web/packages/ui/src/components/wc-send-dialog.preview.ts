import { html } from 'lit';
import './wc-send-dialog.js';
import type { SendStepView } from './wc-send-dialog.js';
import type { Preview } from '../../preview/types.js';

const LABELS: Record<string, string> = {
  config: 'Reading the invoicing settings',
  load: 'Loading the invoice',
  precheck: 'Checking the invoice can be sent',
  payment_link: 'Creating the Stripe payment link',
  render: 'Rendering the invoice',
  publish: 'Publishing to R2',
  email: 'Emailing the client',
  record: 'Recording the send',
};

const ORDER = Object.keys(LABELS);

function trace(done: string[], running?: string, failed?: string): SendStepView[] {
  return ORDER.map((step) => ({
    step,
    label: LABELS[step],
    state:
      step === failed
        ? 'failed'
        : step === running
          ? 'running'
          : done.includes(step)
            ? step === 'payment_link'
              ? 'reused'
              : 'ok'
            : 'pending',
  }));
}

const base = html`
  <wc-send-dialog
    open
    .number=${1251}
    .total=${1850}
    .recipient=${'ap@acme.test'}
    .publishHost=${'billing.rygn.io'}
    .subject=${'Invoice #1251 from Raygun'}
  ></wc-send-dialog>
`;

const preview: Preview = {
  id: 'wc-send-dialog',
  title: 'Send dialog',
  group: 'Invoicing',
  description:
    'The confirmation, the step trace, and the outcome. The one dialog that survives its own request — a step trace has nowhere else to be rendered.',
  layout: 'stack',
  states: [
    { name: 'confirm', render: () => base },
    {
      name: 'blocked-no-email',
      render: () => html`
        <wc-send-dialog
          open
          .number=${1249}
          .total=${960}
          .recipient=${''}
          .blocked=${'Globex has no email address. Add one on the client before sending.'}
        ></wc-send-dialog>
      `,
    },
    {
      name: 'in-flight',
      render: () => html`
        <wc-send-dialog
          open
          phase="sending"
          .number=${1251}
          .total=${1850}
          .recipient=${'ap@acme.test'}
          .steps=${trace(['config', 'load', 'precheck', 'payment_link', 'render'], 'publish')}
        ></wc-send-dialog>
      `,
    },
    {
      name: 'sent',
      render: () => html`
        <wc-send-dialog
          open
          phase="sent"
          .number=${1251}
          .total=${1850}
          .recipient=${'ap@acme.test'}
          .publicUrl=${'https://billing.rygn.io/i/aBc123XyZ/'}
          .steps=${trace(ORDER)}
        ></wc-send-dialog>
      `,
    },
    {
      name: 'failed-at-publish',
      render: () => html`
        <wc-send-dialog
          open
          phase="failed"
          .number=${1251}
          .total=${1850}
          .recipient=${'ap@acme.test'}
          .steps=${trace(
            ['config', 'load', 'precheck', 'payment_link', 'render'],
            undefined,
            'publish',
          )}
          .failure=${{
            headline: 'Publishing the invoice page failed.',
            message: 'r2 403: SignatureDoesNotMatch',
            note: 'No email was sent, and invoice #1251 is still a draft.',
            retryable: true,
          }}
        ></wc-send-dialog>
      `,
    },
    {
      name: 'failed-at-record',
      render: () => html`
        <wc-send-dialog
          open
          phase="failed"
          .number=${1251}
          .total=${1850}
          .recipient=${'ap@acme.test'}
          .steps=${trace(
            ['config', 'load', 'precheck', 'payment_link', 'render', 'publish', 'email'],
            undefined,
            'record',
          )}
          .failure=${{
            headline: 'The send could not be recorded.',
            message: 'database is locked',
            note: 'The invoice was emailed but Nigel could not record it. Run `nigel invoice show 1251` to check before sending again.',
            retryable: false,
          }}
        ></wc-send-dialog>
      `,
    },
  ],
};

export default preview;
