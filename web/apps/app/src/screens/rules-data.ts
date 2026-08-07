import { MATCH_TYPES, type RuleFormValue } from '@nigel/ui';
import type {
  NewRuleRequest,
  RuleMatchType,
  RulePatch,
  RuleRow,
} from '../api/types.js';

/** Empty means "no vendor", which on the wire is null rather than "". */
function orNull(value: string): string | null {
  const trimmed = value.trim();
  return trimmed === '' ? null : trimmed;
}

/**
 * A match type the API will accept, or undefined.
 *
 * A rule row carries whatever is in the database. Every write path validates,
 * so an out-of-vocabulary value cannot be created through nigel — but it can be
 * displayed, and displaying one must not turn into writing it back.
 */
export function asMatchType(value: string): RuleMatchType | undefined {
  return (MATCH_TYPES as readonly string[]).includes(value)
    ? (value as RuleMatchType)
    : undefined;
}

export function toRuleForm(row: RuleRow): RuleFormValue {
  return {
    pattern: row.pattern,
    matchType: row.matchType,
    categoryId: row.categoryId,
    vendor: row.vendor ?? '',
    priority: row.priority,
  };
}

/**
 * Match type and priority are sent explicitly rather than left to the server's
 * defaults, so the rule that gets written is the one the form was showing.
 */
export function newRuleRequest(value: RuleFormValue): NewRuleRequest {
  return {
    pattern: value.pattern.trim(),
    categoryId: value.categoryId as number,
    vendor: orNull(value.vendor),
    matchType: asMatchType(value.matchType) ?? 'contains',
    priority: value.priority,
  };
}

/** The smallest legal PATCH: only what changed. */
export function rulePatch(current: RuleRow, next: RuleFormValue): RulePatch {
  const patch: RulePatch = {};

  const pattern = next.pattern.trim();
  if (pattern !== current.pattern) patch.pattern = pattern;

  if (next.matchType !== current.matchType) {
    const matchType = asMatchType(next.matchType);
    if (matchType) patch.matchType = matchType;
  }

  const vendor = orNull(next.vendor);
  if (vendor !== current.vendor) patch.vendor = vendor;

  if (next.categoryId !== null && next.categoryId !== current.categoryId) {
    patch.categoryId = next.categoryId;
  }

  if (next.priority !== current.priority) patch.priority = next.priority;

  return patch;
}

export function isEmptyRulePatch(patch: RulePatch): boolean {
  return Object.keys(patch).length === 0;
}

/** `#/rules?categoryId=12`, or null when the list is unfiltered. */
export function categoryIdFrom(params: URLSearchParams): number | null {
  const raw = params.get('categoryId');
  if (raw === null) return null;
  const parsed = Number.parseInt(raw, 10);
  return Number.isNaN(parsed) ? null : parsed;
}

export function filterRules(rules: RuleRow[], categoryId: number | null): RuleRow[] {
  return categoryId === null
    ? rules
    : rules.filter((rule) => rule.categoryId === categoryId);
}
