import { roundHalfEven, type SendFailureView } from '@nigel/ui';

import { ApiError } from '../api/index.js';
import type { ConflictDetails, SendErrorDetails, SendStep } from '../api/types.js';
import { conflictDetailsOf } from './manager-errors.js';

export { conflictDetailsOf };

/** What a refusal is about, for the sentences that need a noun. */
export type InvoicingSubject = 'invoice' | 'client';

/** A figure inside a sentence, rounded and grouped the way `fmt::money` is. */
export function money(amount: number): string {
  return new Intl.NumberFormat(undefined, {
    style: 'currency',
    currency: 'USD',
  }).format(roundHalfEven(amount, 2));
}

function plural(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? '' : 's'}`;
}

/**
 * What to tell the user about a refused invoicing write.
 *
 * Modelled on `manager-errors.ts` exactly, and for its reason: the codes are
 * the contract, so the sentence is built here from `details` and the server's
 * English is never shown for a reason we recognize. That is what makes these
 * strings the only thing a translation has to touch, and what lets the count
 * and the figures be formatted the way the rest of the app formats them.
 *
 * The same two deliberate exceptions: a **400** renders the server's message,
 * because `Invalid payment method: bitcoin. Must be one of: …` names the
 * offending value and the legal set; and an **unrecognized 409 reason** does
 * too, because inventing "something conflicted" would hide the only
 * information there is.
 */
export function invoicingGuardrailMessage(
  error: unknown,
  subject: InvoicingSubject,
): string {
  const details = conflictDetailsOf(error);

  if (details) {
    const sentence = sentenceFor(details);
    if (sentence) return sentence;
  }

  if (error instanceof ApiError) return error.message;
  return subject === 'client'
    ? 'Could not save that client.'
    : 'Could not save that invoice.';
}

function sentenceFor(details: ConflictDetails): string | null {
  const count = details.count ?? 0;

  switch (details.reason) {
    case 'void':
      return 'This invoice is void. A cancelled invoice cannot be edited, sent or paid.';
    case 'already_void':
      return 'This invoice has already been voided. The view has been refreshed.';
    case 'not_draft':
      return details.status
        ? `Only a draft can be edited, and this invoice is ${details.status}.`
        : 'Only a draft invoice can be edited.';
    case 'has_payments':
      return details.paid !== undefined && details.total !== undefined
        ? `${money(details.paid)} of ${money(details.total)} has already been paid. Nigel will not edit or void an invoice once money has come in.`
        : 'A payment has already been recorded against this invoice.';
    case 'no_balance':
      return details.paid !== undefined && details.total !== undefined
        ? `This invoice has no outstanding balance — ${money(details.paid)} of ${money(details.total)} is already recorded.`
        : 'This invoice has no outstanding balance.';
    case 'has_invoices':
      return `${plural(count, 'invoice')} ${count === 1 ? 'bills' : 'bill'} this client. Nigel will not delete a client that has been billed.`;
    case 'duplicate_name':
      return details.name
        ? `A client named “${details.name}” already exists.`
        : 'That client name is already taken.';
    case 'client_missing_email':
      return details.clientName
        ? `${details.clientName} has no email address. Add one on the client before sending.`
        : 'This client has no email address. Add one before sending.';
    case 'invoice_not_payable':
      return 'This invoice has nothing to bill for, so there is nothing to send.';
    case 'send_not_configured':
      return missingKeysSentence(details.missing ?? []);
    default:
      return null;
  }
}

function missingKeysSentence(missing: string[]): string {
  if (missing.length === 0) return 'Sending is not configured yet.';
  return `Sending needs ${missing.join(', ')}, which ${
    missing.length === 1 ? 'is' : 'are'
  } not set.`;
}

export interface GuardrailAction {
  label: string;
  /** Query for the screen the guardrail points at. */
  params: URLSearchParams;
}

/**
 * The one invoicing guardrail that can point somewhere useful.
 *
 * "7 invoices bill this client" is a dead end on its own; the link filters the
 * invoices list down to exactly them, which is the `has_active_rules`
 * precedent.
 */
export function invoicingGuardrailAction(
  error: unknown,
  clientId: number,
): GuardrailAction | null {
  if (conflictDetailsOf(error)?.reason !== 'has_invoices') return null;
  return {
    label: 'Show those invoices',
    params: new URLSearchParams({ clientId: String(clientId) }),
  };
}

/** A stale view rather than a real block: refetching is the whole fix. */
export function isStaleInvoiceConflict(error: unknown): boolean {
  return conflictDetailsOf(error)?.reason === 'already_void';
}

/** The `details` of a send failure, whatever status it came back as. */
export function sendDetailsOf(error: unknown): SendErrorDetails | null {
  if (!(error instanceof ApiError)) return null;
  const details = error.details;
  if (typeof details !== 'object' || details === null) return null;
  return details as SendErrorDetails;
}

/** Which step stopped a send, or null when the failure names none. */
export function failedStep(error: unknown): SendStep | null {
  return sendDetailsOf(error)?.step ?? null;
}

/** The steps that completed before the failure, in execution order. */
export function completedSteps(error: unknown): SendStep[] {
  return sendDetailsOf(error)?.completed ?? [];
}

const SERVICE_NAMES: Record<string, string> = {
  stripe: 'Stripe',
  r2: 'Cloudflare R2',
  mailgun: 'Mailgun',
};

/** Our words for what each step was doing when it stopped. */
const STEP_HEADLINES: Record<SendStep, (service: string) => string> = {
  config: () => 'Nigel could not read the invoicing settings.',
  load: () => 'The invoice could not be loaded.',
  precheck: () => 'This invoice cannot be sent.',
  payment_link: (service) =>
    `${service || 'Stripe'} would not create the payment link.`,
  render: () => 'The invoice could not be rendered.',
  publish: (service) =>
    `Publishing the invoice page${service ? ` to ${service}` : ''} failed.`,
  email: (service) =>
    `The page was published, but ${service || 'the mail gateway'} would not send the email.`,
  record: () => 'The send could not be recorded.',
};

/**
 * A send failure, turned into the three strings the dialog renders.
 *
 * Our words for **what** failed, the upstream's own for **why**: `r2 403:
 * SignatureDoesNotMatch` is the only information anyone has about why R2
 * refused, and a sentence reconstructed from the status would be a worse bug
 * report. What we add is structure — the step, the service, and what it means
 * for the invoice — never a substitute.
 *
 * `retryable` is false in exactly two situations: a refusal that would refuse
 * again identically (no configuration, no client email, a void invoice), and a
 * failure after the email went out, where the client already has the invoice
 * and trying again is a fresh decision rather than a repeat.
 */
export function sendFailureMessage(error: unknown, number: number): SendFailureView {
  const details = sendDetailsOf(error);
  const message = error instanceof ApiError ? error.message : String(error);

  // A 501 is this build having no `pdf` feature: the render step will refuse
  // identically every time, and the fix is a different binary rather than
  // another attempt. It belongs with the pre-flight refusals below, not with
  // the outages.
  const featureMissing = error instanceof ApiError && error.status === 501;

  // The pre-flight refusals: same request, same answer, so no Try again.
  const conflict = conflictDetailsOf(error);
  if (conflict?.reason === 'send_not_configured') {
    return {
      headline: 'Sending is not configured yet.',
      message: missingKeysSentence(conflict.missing ?? []),
      note: 'These are settings, not something this invoice can fix.',
      retryable: false,
      actionLabel: 'Open settings',
      actionHref: '#/settings',
    };
  }
  if (conflict && sentenceFor(conflict)) {
    return {
      headline: sentenceFor(conflict) as string,
      message,
      retryable: false,
    };
  }

  const step = details?.step;
  if (!step) {
    return {
      headline: 'The invoice could not be sent.',
      message,
      retryable: details?.emailSent !== true && !featureMissing,
    };
  }

  const service = details?.service ? (SERVICE_NAMES[details.service] ?? details.service) : '';
  const emailSent = details.emailSent === true;

  return {
    headline: STEP_HEADLINES[step](service),
    message,
    note: emailSent
      ? `The invoice was emailed but Nigel could not record it. Run \`nigel invoice show ${number}\` to check before sending it again.`
      : featureMissing
        ? `No email was sent. This build of Nigel cannot render a PDF, so no invoice can be sent from it.`
        : `No email was sent${
            details.invoiceStatus ? `, and invoice #${number} is still a ${details.invoiceStatus}` : ''
          }.`,
    retryable: !emailSent && !featureMissing,
  };
}
