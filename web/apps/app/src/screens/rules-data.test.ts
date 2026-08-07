import { describe, it, expect } from 'vitest';
import {
  asMatchType,
  categoryIdFrom,
  filterRules,
  isEmptyRulePatch,
  newRuleRequest,
  rulePatch,
  toRuleForm,
} from './rules-data.js';
import type { RuleRow } from '../api/types.js';

const rule: RuleRow = {
  id: 7,
  pattern: 'ADOBE',
  matchType: 'contains',
  vendor: 'Adobe',
  category: 'Software / Subscriptions',
  categoryId: 12,
  priority: 10,
  hitCount: 42,
};

describe('asMatchType', () => {
  it('accepts the three the categorizer understands', () => {
    expect(asMatchType('contains')).toBe('contains');
    expect(asMatchType('starts_with')).toBe('starts_with');
    expect(asMatchType('regex')).toBe('regex');
  });

  it('refuses anything else rather than passing it on', () => {
    expect(asMatchType('fuzzy')).toBeUndefined();
    expect(asMatchType('')).toBeUndefined();
  });
});

describe('newRuleRequest', () => {
  it('sends match type and priority explicitly, not by omission', () => {
    // What the form showed is what gets written; the server's defaults are
    // right but they are not the form's promise.
    expect(newRuleRequest(toRuleForm(rule))).toEqual({
      pattern: 'ADOBE',
      categoryId: 12,
      vendor: 'Adobe',
      matchType: 'contains',
      priority: 10,
    });
  });

  it('trims the pattern and nulls an empty vendor', () => {
    expect(
      newRuleRequest({ ...toRuleForm(rule), pattern: '  SQ * ', vendor: '  ' }),
    ).toMatchObject({ pattern: 'SQ *', vendor: null });
  });

  it('falls back to contains for a match type it does not know', () => {
    expect(newRuleRequest({ ...toRuleForm(rule), matchType: 'fuzzy' }).matchType).toBe(
      'contains',
    );
  });
});

describe('rulePatch', () => {
  it('sends nothing when nothing changed', () => {
    expect(rulePatch(rule, toRuleForm(rule))).toEqual({});
  });

  it('sends only what changed', () => {
    expect(rulePatch(rule, { ...toRuleForm(rule), priority: 20 })).toEqual({
      priority: 20,
    });
  });

  it('clears a vendor with an explicit null', () => {
    expect(rulePatch(rule, { ...toRuleForm(rule), vendor: '' })).toEqual({ vendor: null });
  });

  it('carries a category change', () => {
    expect(rulePatch(rule, { ...toRuleForm(rule), categoryId: 14 })).toEqual({
      categoryId: 14,
    });
  });

  it('never writes back a match type it does not recognize', () => {
    // A rule displayed with an out-of-vocabulary type must not be retyped by
    // the act of editing its priority.
    const odd: RuleRow = { ...rule, matchType: 'fuzzy' };
    expect(rulePatch(odd, { ...toRuleForm(odd), priority: 3 })).toEqual({ priority: 3 });
    expect(rulePatch(odd, { ...toRuleForm(odd), matchType: 'regex' })).toEqual({
      matchType: 'regex',
    });
  });

  it('ignores a category cleared to null rather than sending one', () => {
    // categoryId: null is a 400 — uncategorizing is not what a rule edit does.
    expect(rulePatch(rule, { ...toRuleForm(rule), categoryId: null })).toEqual({});
  });
});

describe('isEmptyRulePatch', () => {
  it('recognizes the patch that must never be sent', () => {
    expect(isEmptyRulePatch({})).toBe(true);
    expect(isEmptyRulePatch({ vendor: null })).toBe(false);
  });
});

describe('categoryIdFrom', () => {
  it('reads the filter out of the query', () => {
    expect(categoryIdFrom(new URLSearchParams('categoryId=12'))).toBe(12);
  });

  it('is null for absent or unparseable values', () => {
    expect(categoryIdFrom(new URLSearchParams())).toBeNull();
    expect(categoryIdFrom(new URLSearchParams('categoryId=all'))).toBeNull();
  });
});

describe('filterRules', () => {
  const other: RuleRow = { ...rule, id: 8, categoryId: 14, pattern: 'SQ *' };

  it('returns everything when unfiltered', () => {
    expect(filterRules([rule, other], null)).toHaveLength(2);
  });

  it('keeps only the rules assigning that category', () => {
    expect(filterRules([rule, other], 14).map((r) => r.id)).toEqual([8]);
  });
});
