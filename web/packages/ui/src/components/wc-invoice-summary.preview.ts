import { html } from 'lit';
import './wc-invoice-summary.js';
import type { Preview } from '../../preview/types.js';

interface SummaryProps {
  number: number;
  status: string;
  clientName: string | null;
  total: number;
  balance: number;
  issueDate: string;
  dueDate: string | null;
}

const base: SummaryProps = {
  number: 1251,
  status: 'sent',
  clientName: 'Acme Co',
  total: 1850,
  balance: 1850,
  issueDate: '2026-08-07',
  dueDate: '2026-09-06',
};

function summary(props: Partial<SummaryProps>) {
  const value = { ...base, ...props };
  return html`
    <wc-invoice-summary
      .number=${value.number}
      status=${value.status}
      .clientName=${value.clientName}
      .total=${value.total}
      .balance=${value.balance}
      .issueDate=${value.issueDate}
      .dueDate=${value.dueDate}
    ></wc-invoice-summary>
  `;
}

const preview: Preview = {
  id: 'wc-invoice-summary',
  title: 'Invoice summary',
  group: 'Invoicing',
  description:
    'The detail header: number, status, client, total and what is still outstanding.',
  layout: 'stack',
  states: [
    { name: 'draft', render: () => summary({ number: 1252, status: 'draft', dueDate: null }) },
    { name: 'sent', render: () => summary({}) },
    {
      name: 'partial',
      render: () =>
        summary({ number: 1250, status: 'partial', total: 3200, balance: 1200 }),
    },
    {
      name: 'overdue',
      render: () =>
        summary({ number: 1249, status: 'overdue', total: 960, balance: 960, dueDate: '2026-06-30' }),
    },
    {
      name: 'paid',
      render: () => summary({ number: 1248, status: 'paid', total: 4000, balance: 0 }),
    },
    {
      name: 'void',
      render: () =>
        summary({ number: 1247, status: 'void', total: 500, balance: 0, dueDate: null }),
    },
    { name: 'no-due-date', render: () => summary({ dueDate: null }) },
    { name: 'orphaned-client', render: () => summary({ clientName: null }) },
  ],
};

export default preview;
