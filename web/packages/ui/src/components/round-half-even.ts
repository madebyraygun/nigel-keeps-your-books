/**
 * Round to `digits` decimals the way Rust's `{:.N}` does, which is how every
 * figure in `src/fmt.rs` and `cli/report/text.rs` reaches the page.
 *
 * Two rules, and JavaScript's own formatters break one each. `Intl` rounds the
 * shortest decimal that identifies the double rather than the double itself, so
 * it reads 0.565 as a tie and rounds it up, where the stored value is
 * 0.564999999999999947 and rounds down. `toFixed` reads the stored value, but
 * settles a genuine tie — an exact half such as 0.125 or 12.25 — away from
 * zero, where Rust settles it on the even neighbour.
 *
 * So: `toFixed` for the ordinary case, and the even rule applied by hand for a
 * tie. A tie at `digits` decimals is exactly `odd / 2^(digits + 1)`, and
 * multiplying by a power of two is exact in floating point, which is what makes
 * the test for one reliable rather than another rounding of its own.
 */
export function roundHalfEven(value: number, digits: number): number {
  if (!Number.isFinite(value)) return value;

  const halves = value * 2 ** (digits + 1);
  if (Number.isInteger(halves) && Math.abs(halves % 2) === 1) {
    const scale = 10 ** digits;
    const down = Math.floor(value * scale);
    return (down % 2 === 0 ? down : down + 1) / scale;
  }

  return Number(value.toFixed(digits));
}
