import { describe, it, expect } from 'vitest';
import { roundHalfEven } from './round-half-even.js';

describe('roundHalfEven', () => {
  it('rounds the stored value, not the shortest decimal that names it', () => {
    // 0.565 is stored as 0.564999999999999947, which `{:.2}` rounds down and
    // `Intl` — reading "0.565" — rounds up.
    expect(roundHalfEven(0.565, 2)).toBe(0.56);
    expect(roundHalfEven(1.005, 2)).toBe(1);
    expect(roundHalfEven(8.835, 2)).toBe(8.84);
  });

  it('settles an exact tie on the even neighbour', () => {
    // The halves `abs_total * 0.5` manufactures on the K-1 meals line.
    expect(roundHalfEven(0.125, 2)).toBe(0.12);
    expect(roundHalfEven(0.375, 2)).toBe(0.38);
    expect(roundHalfEven(617.125, 2)).toBe(617.12);
    expect(roundHalfEven(617.375, 2)).toBe(617.38);
    expect(roundHalfEven(12.25, 1)).toBe(12.2);
    expect(roundHalfEven(12.35, 1)).toBe(12.3);
  });

  it('rounds a negative the same distance as its positive', () => {
    expect(roundHalfEven(-0.125, 2)).toBe(-0.12);
    expect(roundHalfEven(-12.25, 1)).toBe(-12.2);
    expect(roundHalfEven(-0.565, 2)).toBe(-0.56);
  });

  it('leaves a value that already fits alone', () => {
    expect(roundHalfEven(1234.56, 2)).toBe(1234.56);
    expect(roundHalfEven(0, 2)).toBe(0);
    expect(roundHalfEven(-500, 2)).toBe(-500);
  });

  it('passes anything that is not a finite number straight through', () => {
    expect(roundHalfEven(Number.POSITIVE_INFINITY, 2)).toBe(Number.POSITIVE_INFINITY);
    expect(roundHalfEven(Number.NaN, 2)).toBeNaN();
  });
});
