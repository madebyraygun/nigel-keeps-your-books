import { describe, it, expect } from 'vitest';
import { ApiError } from '../api/index.js';
import { conflictError } from '../__mocks__/fake-api-client.js';
import {
  conflictDetailsOf,
  guardrailAction,
  guardrailMessage,
  isStaleListConflict,
} from './manager-errors.js';

describe('conflictDetailsOf', () => {
  it('reads the details of a 409', () => {
    expect(conflictDetailsOf(conflictError('has_transactions', { count: 5 }))).toEqual({
      reason: 'has_transactions',
      count: 5,
    });
  });

  it('ignores anything that is not a 409 with object details', () => {
    expect(conflictDetailsOf(new Error('boom'))).toBeNull();
    expect(
      conflictDetailsOf(
        new ApiError({
          code: 'bad_request',
          rawCode: 'bad_request',
          message: 'Name is required',
          status: 400,
        }),
      ),
    ).toBeNull();
    expect(
      conflictDetailsOf(
        new ApiError({
          code: 'conflict',
          rawCode: 'conflict',
          message: 'Refused',
          status: 409,
          details: 'has_transactions',
        }),
      ),
    ).toBeNull();
  });
});

describe('guardrailMessage', () => {
  it('explains a blocked account delete with the count', () => {
    expect(
      guardrailMessage(conflictError('has_transactions', { count: 5 }), 'account'),
    ).toBe(
      'This account has 5 transactions. Nigel will not delete an account that still has activity.',
    );
  });

  it('explains a blocked category delete in the category’s own terms', () => {
    expect(
      guardrailMessage(conflictError('has_transactions', { count: 37 }), 'category'),
    ).toBe('37 transactions use this category. Recategorize them first.');
  });

  it('explains a category still assigned by rules', () => {
    expect(
      guardrailMessage(conflictError('has_active_rules', { count: 3 }), 'category'),
    ).toBe('3 active rules assign this category. Delete those rules first.');
  });

  it('says one thing once', () => {
    expect(
      guardrailMessage(conflictError('has_transactions', { count: 1 }), 'category'),
    ).toBe('1 transaction uses this category. Recategorize them first.');
    expect(
      guardrailMessage(conflictError('has_active_rules', { count: 1 }), 'category'),
    ).toBe('1 active rule assigns this category. Delete those rules first.');
    expect(
      guardrailMessage(conflictError('has_transactions', { count: 1 }), 'account'),
    ).toBe(
      'This account has 1 transaction. Nigel will not delete an account that still has activity.',
    );
  });

  it('names the taken name, and the subject it was taken for', () => {
    expect(
      guardrailMessage(
        conflictError('duplicate_name', { name: 'BofA Checking' }),
        'account',
      ),
    ).toBe('An account named “BofA Checking” already exists.');
    expect(
      guardrailMessage(conflictError('duplicate_name', { name: 'Software' }), 'category'),
    ).toBe('A category named “Software” already exists.');
  });

  it('copes with a duplicate_name that carries no name', () => {
    expect(guardrailMessage(conflictError('duplicate_name'), 'category')).toBe(
      'That category name is already taken.',
    );
  });

  it('explains an already-deleted rule as the stale list it is', () => {
    expect(guardrailMessage(conflictError('already_inactive'), 'rule')).toBe(
      'This rule has already been deleted. The list has been refreshed.',
    );
  });

  it('never shows the server’s sentence for a reason it knows', () => {
    // The codes are the contract; the English is ours, which is what makes it
    // translatable and the count formattable.
    const message = guardrailMessage(
      conflictError('has_transactions', {
        count: 5,
        message: 'Cannot delete: account has 5 transactions',
      }),
      'account',
    );
    expect(message).not.toContain('Cannot delete');
  });

  it('falls back to the server’s message for a reason it does not know', () => {
    expect(
      guardrailMessage(
        conflictError('some_future_reason', { message: 'Refused for reasons.' }),
        'rule',
      ),
    ).toBe('Refused for reasons.');
  });

  it('shows a 400 verbatim, because it names the offending value', () => {
    const error = new ApiError({
      code: 'bad_request',
      rawCode: 'bad_request',
      message: 'Invalid match type: fuzzy. Must be one of: contains, starts_with, regex',
      status: 400,
    });
    expect(guardrailMessage(error, 'rule')).toBe(
      'Invalid match type: fuzzy. Must be one of: contains, starts_with, regex',
    );
  });

  it('has something to say about a failure that is not an ApiError at all', () => {
    expect(guardrailMessage(new TypeError('undefined is not a function'), 'account')).toBe(
      'Could not save that account.',
    );
  });
});

describe('guardrailAction', () => {
  it('points at the rules that are doing the blocking', () => {
    const action = guardrailAction(conflictError('has_active_rules', { count: 3 }), 12);
    expect(action?.label).toBe('Show those rules');
    expect(action?.params.get('categoryId')).toBe('12');
  });

  it('offers nothing for a block with nowhere to go', () => {
    expect(guardrailAction(conflictError('has_transactions', { count: 3 }), 12)).toBeNull();
    expect(guardrailAction(new Error('boom'), 12)).toBeNull();
  });
});

describe('isStaleListConflict', () => {
  it('is true only for a rule that was already deleted', () => {
    expect(isStaleListConflict(conflictError('already_inactive'))).toBe(true);
    expect(isStaleListConflict(conflictError('has_transactions', { count: 1 }))).toBe(
      false,
    );
  });
});
