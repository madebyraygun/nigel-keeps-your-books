/** A choice in any category picker — the register's inline editor or review. */
export interface CategoryOption {
  id: number;
  name: string;
  categoryType: string;
}

/**
 * The label the TUI puts next to a category, `Consulting income (inc)`.
 *
 * Shared rather than written twice: the suffix is what the user types against
 * in every picker, so two implementations of it is two filter behaviours.
 */
export function categoryLabel(category: CategoryOption): string {
  return `${category.name} (${category.categoryType === 'income' ? 'inc' : 'exp'})`;
}
