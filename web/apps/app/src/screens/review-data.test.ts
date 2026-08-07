import { describe, it, expect } from 'vitest';
import { singleIdFrom, summarize, toReviewItem, type Decision } from './review-data.js';
import type { FlaggedTxn, RegisterRow } from '../api/types.js';

const queueTxn: FlaggedTxn = {
  id: 7,
  date: '2025-03-04',
  description: 'ADOBE CREATIVE CLOUD',
  amount: -54.99,
  accountName: 'BofA Credit Card',
};

const registerRow: RegisterRow = {
  ...queueTxn,
  category: 'Software / Subscriptions',
  categoryId: 12,
  vendor: 'Adobe Inc',
  isFlagged: false,
};

describe('toReviewItem', () => {
  it('reads a queue entry, which carries no category', () => {
    expect(toReviewItem(queueTxn)).toEqual({
      id: 7,
      date: '2025-03-04',
      description: 'ADOBE CREATIVE CLOUD',
      amount: -54.99,
      accountName: 'BofA Credit Card',
      category: null,
      vendor: null,
    });
  });

  it('keeps what a full register row already carries', () => {
    const item = toReviewItem(registerRow);
    expect(item.category).toBe('Software / Subscriptions');
    expect(item.vendor).toBe('Adobe Inc');
  });
});

describe('summarize', () => {
  const decision = (ruleId: number | null): Decision => ({
    transactionId: 1,
    ruleId,
  });

  it('counts nothing for an empty stack', () => {
    expect(summarize([])).toEqual({ reviewed: 0, skipped: 0, rulesCreated: 0 });
  });

  it('tells decisions from skips, and counts the rules among them', () => {
    expect(summarize([decision(100), null, decision(null), null, decision(101)])).toEqual(
      { reviewed: 3, skipped: 2, rulesCreated: 2 },
    );
  });
});

describe('singleIdFrom', () => {
  it('finds an id', () => {
    expect(singleIdFrom(new URLSearchParams('id=185'))).toBe(185);
  });

  it('is null for the plain queue route', () => {
    expect(singleIdFrom(new URLSearchParams(''))).toBeNull();
  });

  it.each(['id=nonsense', 'id=0', 'id=-4', 'id=1.5', 'id='])(
    'refuses %s rather than asking the server for it',
    (query) => {
      expect(singleIdFrom(new URLSearchParams(query))).toBeNull();
    },
  );
});
