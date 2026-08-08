import { describe, it, expect } from 'vitest';

import { ApiError } from '../api/index.js';
import {
  completedSteps,
  failedStep,
  invoicingGuardrailAction,
  invoicingGuardrailMessage,
  isStaleInvoiceConflict,
  sendFailureMessage,
} from './invoicing-errors.js';

function conflict(reason: string, details: Record<string, unknown> = {}, message = 'refused') {
  return new ApiError({
    code: 'conflict',
    rawCode: 'conflict',
    message,
    status: 409,
    details: { reason, ...details },
  });
}

function upstream(details: Record<string, unknown>, message: string) {
  return new ApiError({
    code: 'upstream_failed',
    rawCode: 'upstream_failed',
    message,
    status: 502,
    details: { reason: 'send_failed', ...details },
  });
}

describe('invoicingGuardrailMessage', () => {
  it('explains a void invoice in our words, not the server’s', () => {
    expect(invoicingGuardrailMessage(conflict('void'), 'invoice')).toContain(
      'This invoice is void',
    );
  });

  it('names the status that blocked an edit', () => {
    expect(
      invoicingGuardrailMessage(conflict('not_draft', { status: 'sent' }), 'invoice'),
    ).toBe('Only a draft can be edited, and this invoice is sent.');
  });

  it('quotes the figures behind a has_payments refusal', () => {
    const message = invoicingGuardrailMessage(
      conflict('has_payments', { paid: 2000, total: 3200 }),
      'invoice',
    );
    expect(message).toContain('$2,000.00');
    expect(message).toContain('$3,200.00');
  });

  it('quotes the figures behind a no_balance refusal', () => {
    expect(
      invoicingGuardrailMessage(
        conflict('no_balance', { paid: 4000, total: 4000 }),
        'invoice',
      ),
    ).toContain('no outstanding balance');
  });

  it('counts the invoices blocking a client delete, and pluralizes', () => {
    expect(invoicingGuardrailMessage(conflict('has_invoices', { count: 7 }), 'client')).toBe(
      '7 invoices bill this client. Nigel will not delete a client that has been billed.',
    );
    expect(invoicingGuardrailMessage(conflict('has_invoices', { count: 1 }), 'client')).toContain(
      '1 invoice bills this client',
    );
  });

  it('names the duplicate client', () => {
    expect(
      invoicingGuardrailMessage(conflict('duplicate_name', { name: 'Acme Co' }), 'client'),
    ).toBe('A client named “Acme Co” already exists.');
  });

  it('names the client whose record needs an email', () => {
    expect(
      invoicingGuardrailMessage(
        conflict('client_missing_email', { clientId: 2, clientName: 'Globex' }),
        'invoice',
      ),
    ).toContain('Globex has no email address');
  });

  it('says an already-void invoice is a stale view rather than a block', () => {
    expect(isStaleInvoiceConflict(conflict('already_void'))).toBe(true);
    expect(isStaleInvoiceConflict(conflict('void'))).toBe(false);
  });

  it('falls back to the server sentence for a 400', () => {
    // `Invalid payment method: bitcoin. Must be one of: …` names the offending
    // value and the legal set; anything re-derived here would be worse.
    const error = new ApiError({
      code: 'bad_request',
      rawCode: 'bad_request',
      message: 'Invalid payment method: bitcoin. Must be one of: stripe, ach, direct_deposit, other',
      status: 400,
    });
    expect(invoicingGuardrailMessage(error, 'invoice')).toBe(error.message);
  });

  it('falls back to the server sentence for an unrecognized 409 reason', () => {
    expect(
      invoicingGuardrailMessage(conflict('some_new_rule', {}, 'Nigel refused that'), 'invoice'),
    ).toBe('Nigel refused that');
  });

  it('has a sentence of its own for something that is not an ApiError at all', () => {
    expect(invoicingGuardrailMessage(new Error('boom'), 'client')).toBe(
      'Could not save that client.',
    );
  });
});

describe('invoicingGuardrailAction', () => {
  it('points a blocked client delete at that client’s invoices', () => {
    const action = invoicingGuardrailAction(conflict('has_invoices', { count: 7 }), 3);
    expect(action?.label).toBe('Show those invoices');
    expect(action?.params.toString()).toBe('clientId=3');
  });

  it('offers nothing for a guardrail with nowhere useful to go', () => {
    expect(invoicingGuardrailAction(conflict('duplicate_name'), 3)).toBeNull();
  });
});

describe('send failure details', () => {
  it('reads the step and the steps that completed before it', () => {
    const error = upstream(
      { step: 'publish', service: 'r2', completed: ['config', 'load'] },
      'r2 403',
    );
    expect(failedStep(error)).toBe('publish');
    expect(completedSteps(error)).toEqual(['config', 'load']);
  });

  it('reads nothing out of a failure that names no step', () => {
    expect(failedStep(new Error('boom'))).toBeNull();
    expect(completedSteps(new Error('boom'))).toEqual([]);
  });
});

describe('sendFailureMessage', () => {
  it('names the step and the service in our words', () => {
    const view = sendFailureMessage(
      upstream(
        { step: 'publish', service: 'r2', emailSent: false, invoiceStatus: 'draft' },
        'r2 403: SignatureDoesNotMatch',
      ),
      1251,
    );
    expect(view.headline).toBe('Publishing the invoice page to Cloudflare R2 failed.');
  });

  it('shows the upstream message verbatim underneath', () => {
    const view = sendFailureMessage(
      upstream({ step: 'payment_link', service: 'stripe' }, 'stripe 402: card_declined'),
      1251,
    );
    expect(view.headline).toBe('Stripe would not create the payment link.');
    expect(view.message).toBe('stripe 402: card_declined');
  });

  it('says nothing went out when the failure came before the email', () => {
    const view = sendFailureMessage(
      upstream({ step: 'publish', service: 'r2', emailSent: false, invoiceStatus: 'draft' }, 'r2 403'),
      1251,
    );
    expect(view.note).toBe('No email was sent, and invoice #1251 is still a draft.');
  });

  it('says the email already went out for a record-step failure', () => {
    const view = sendFailureMessage(
      new ApiError({
        code: 'internal',
        rawCode: 'internal',
        message: 'database is locked',
        status: 500,
        details: { reason: 'send_failed', step: 'record', emailSent: true },
      }),
      1251,
    );
    expect(view.note).toContain('was emailed but Nigel could not record it');
    expect(view.note).toContain('nigel invoice show 1251');
  });

  it('offers no retry for a record-step failure', () => {
    const view = sendFailureMessage(
      new ApiError({
        code: 'internal',
        rawCode: 'internal',
        message: 'database is locked',
        status: 500,
        details: { reason: 'send_failed', step: 'record', emailSent: true },
      }),
      1251,
    );
    expect(view.retryable).toBe(false);
  });

  it('offers a retry for every failure before the email', () => {
    for (const step of ['config', 'load', 'payment_link', 'render', 'publish'] as const) {
      const view = sendFailureMessage(upstream({ step, emailSent: false }, 'nope'), 1251);
      expect(view.retryable, step).toBe(true);
    }
  });

  it('offers no retry when this build cannot render a PDF', () => {
    // A 501 at the render step is the `pdf` feature being absent: the same
    // request will refuse identically, and the fix is a different binary.
    const view = sendFailureMessage(
      new ApiError({
        code: 'feature_disabled',
        rawCode: 'feature_disabled',
        message: 'PDF export is not available in this build.',
        status: 501,
        details: { reason: 'send_failed', step: 'render', emailSent: false },
      }),
      1251,
    );
    expect(view.retryable).toBe(false);
    expect(view.note).toContain('cannot render a PDF');
    expect(view.message).toBe('PDF export is not available in this build.');
  });

  it('renders send_not_configured as key names with a settings link', () => {
    // Never the server's sentence, which names settings.json and NIGEL_ env
    // vars — good advice in a terminal, useless beside a settings screen.
    const view = sendFailureMessage(
      conflict(
        'send_not_configured',
        { missing: ['r2_bucket', 'public_base_url'] },
        'Set r2_bucket in ~/.config/nigel/settings.json or NIGEL_R2_BUCKET',
      ),
      1251,
    );
    expect(view.headline).toBe('Sending is not configured yet.');
    expect(view.message).toContain('r2_bucket, public_base_url');
    expect(view.message).not.toContain('settings.json');
    expect(view.actionHref).toBe('#/settings');
    expect(view.retryable).toBe(false);
  });

  it('carries no setting value, only its name', () => {
    const view = sendFailureMessage(
      conflict('send_not_configured', { missing: ['stripe_secret_key'] }),
      1251,
    );
    expect(view.message).toContain('stripe_secret_key');
    expect(JSON.stringify(view)).not.toContain('sk_test');
  });

  it('refuses to retry a precheck refusal, which would refuse identically', () => {
    const view = sendFailureMessage(
      conflict('client_missing_email', { clientId: 2, clientName: 'Globex' }),
      1249,
    );
    expect(view.headline).toContain('Globex has no email address');
    expect(view.retryable).toBe(false);
  });

  it('falls back to the server sentence for a 400 and an unknown 409 reason', () => {
    const badRequest = new ApiError({
      code: 'bad_request',
      rawCode: 'bad_request',
      message: 'Confirmation is required to send an invoice.',
      status: 400,
      details: { reason: 'confirmation_required' },
    });
    expect(sendFailureMessage(badRequest, 1251).message).toBe(badRequest.message);
    expect(sendFailureMessage(badRequest, 1251).headline).toBe(
      'The invoice could not be sent.',
    );

    const unknown = conflict('some_new_rule', {}, 'Nigel refused that');
    expect(sendFailureMessage(unknown, 1251).message).toBe('Nigel refused that');
  });
});
