// Recovers the intended number from floating point imprecision noise when that can be done reliably, e.g. 0.30000000000000004 -> 0.3.
// Rounding to each significant digit count from 1 to 12, the first candidate within a relative 1e-13 of the original is accepted.
// Actual high-precision values (like 0.3333333333333333) never pass the tolerance and are returned unchanged.
export function roundAwayFloatNoise(value: number): number {
	if (value === 0 || !Number.isFinite(value)) return value === 0 ? 0 : value;

	const exponent = Math.floor(Math.log10(Math.abs(value)));
	for (let significantDigits = 1; significantDigits <= 12; significantDigits += 1) {
		const scale = 10 ** (significantDigits - 1 - exponent);
		const rounded = Math.round(value * scale) / scale;
		if (Math.abs((rounded - value) / value) < 1e-13) return rounded;
	}

	return value;
}
