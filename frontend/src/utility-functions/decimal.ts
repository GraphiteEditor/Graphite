// Exact arithmetic on numbers as they're written, so stepping one digit never disturbs the others the way float rounding would

// A written number, `units × 10^(exponent - places)`, where `units` holds all its digits as one integer, `places` counts those after the
// decimal point, and `exponent` is the power of ten after an `e` where it's written in scientific notation
export type Decimal = { units: bigint; places: number; exponent: number | undefined };

// A digit of a written number: one in its mantissa by the power of ten of the whole value it stands for, or one in its exponent by its
// power of ten within the exponent
export type Digit = { part: "mantissa" | "exponent"; place: number };

const WRITTEN = /^([+-]?)(\d*)(?:\.(\d*))?(?:[eE]([+-]?\d+))?$/;

// Far past any float's exponent, while its powers of ten stay quick to step and compare by
const MAX_EXPONENT = 10_000;

export function parseDecimal(text: string): Decimal | undefined {
	const match = WRITTEN.exec(text.trim());
	if (!match) return undefined;

	const [_, sign, whole, fractionDigits, written] = match;
	const fraction = fractionDigits || "";
	if (whole === "" && fraction === "") return undefined;

	const exponent = written === undefined ? undefined : Number(written);
	if (exponent !== undefined && Math.abs(exponent) > MAX_EXPONENT) return undefined;

	const magnitude = BigInt(`${whole}${fraction}`);
	return { units: sign === "-" ? -magnitude : magnitude, places: fraction.length, exponent };
}

// Written with every place it has, a leading `0` before a bare decimal point, and no sign where it's positive
export function formatDecimal({ units, places, exponent }: Decimal): string {
	const digits = magnitudeOf(units)
		.toString()
		.padStart(places + 1, "0");
	const whole = digits.slice(0, digits.length - places);
	const fraction = places > 0 ? `.${digits.slice(digits.length - places)}` : "";
	return `${units < 0n ? "-" : ""}${whole}${fraction}${exponent === undefined ? "" : `e${exponent}`}`;
}

// Adds a count of units of a digit: in the mantissa, written down to its place where that's finer than the number's own last digit, or
// in the exponent, which stays an integer since its places are never below its ones
export function stepDigit(decimal: Decimal, digit: Digit, count: number): Decimal {
	if (digit.part === "exponent") {
		if (decimal.exponent === undefined) return decimal;
		return { ...decimal, exponent: decimal.exponent + count * 10 ** Math.max(digit.place, 0) };
	}

	const extended = extendedTo(decimal, digit.place);
	const units = extended.units + BigInt(count) * 10n ** BigInt(digit.place - lowestPlace(extended));
	return normalized({ ...extended, units });
}

// Rounded half away from zero to a digit place, written only down to that place
export function roundDecimal(decimal: Decimal, place: number): Decimal {
	const dropped = place - lowestPlace(decimal);
	if (dropped <= 0) return decimal;

	const scale = 10n ** BigInt(dropped);
	const rounded = (magnitudeOf(decimal.units) + scale / 2n) / scale;
	// A place above the ones keeps its zeros, since no places after the point remain to drop
	const places = Math.max(decimal.places - dropped, 0);
	const units = rounded * 10n ** BigInt(dropped - (decimal.places - places));
	return normalized({ ...decimal, units: decimal.units < 0n ? -units : units, places });
}

// Whether a number is less than, equal to, or greater than another, as -1, 0, or 1
export function compareDecimal(a: Decimal, b: Decimal): number {
	const lowest = Math.min(lowestPlace(a), lowestPlace(b));
	const [left, right] = [a, b].map((decimal) => decimal.units * 10n ** BigInt(lowestPlace(decimal) - lowest));
	return left < right ? -1 : left > right ? 1 : 0;
}

// The digit a drag steps by default: an integer's last nonzero digit, or else the third significant digit but no finer than the last
// written one, where scientific notation takes these from its mantissa
export function defaultDigit({ units, places, exponent }: Decimal): Digit {
	const offset = exponent || 0;
	const magnitude = magnitudeOf(units);
	if (magnitude === 0n) return { part: "mantissa", place: offset - places };

	if (places === 0) {
		let trailingZeros = 0;
		for (let rest = magnitude; rest % 10n === 0n; rest /= 10n) trailingZeros += 1;
		return { part: "mantissa", place: offset + trailingZeros };
	}

	const leading = magnitude.toString().length - 1 - places;
	return { part: "mantissa", place: offset + Math.max(leading - 2, -places) };
}

// The digit itself where the written number has it, or else the nearest written digit in the same part, or the default where that part is gone
export function clampedDigit(text: string, digit: Digit): Digit {
	const decimal = parseDecimal(text);
	if (!decimal || digitIndex(text, digit) !== undefined) return digit;

	const written = Array.from(text).flatMap((_, index) => digitAt(text, index) || []);
	const sameParts = written.filter((candidate) => candidate.part === digit.part);
	if (sameParts.length === 0) return defaultDigit(decimal);

	const distance = (candidate: Digit) => Math.abs(candidate.place - digit.place);
	return sameParts.reduce((nearest, candidate) => (distance(candidate) < distance(nearest) ? candidate : nearest));
}

export function sameDigit(a: Digit, b: Digit): boolean {
	return a.part === b.part && a.place === b.place;
}

// The digit of a written number at an index, where it has one there
export function digitAt(text: string, index: number): Digit | undefined {
	const decimal = parseDecimal(text);
	if (!decimal || !/\d/.test(text[index] || "")) return undefined;

	const { mantissaEnd, point } = layout(text);
	if (index > mantissaEnd) return { part: "exponent", place: text.length - 1 - index };
	return { part: "mantissa", place: (decimal.exponent || 0) + (index < point ? point - index - 1 : point - index) };
}

// The index of a written number's digit, where it has it
export function digitIndex(text: string, digit: Digit): number | undefined {
	const decimal = parseDecimal(text);
	if (!decimal) return undefined;

	const { mantissaEnd, point } = layout(text);
	let index = text.length - 1 - digit.place;
	if (digit.part === "mantissa") {
		const relative = digit.place - (decimal.exponent || 0);
		index = relative >= 0 ? point - relative - 1 : point - relative;
	}

	const inPart = digit.part === "mantissa" ? index < mantissaEnd : index > mantissaEnd;
	return inPart && /\d/.test(text[index] || "") ? index : undefined;
}

// Where a written number's mantissa ends and its decimal point stands, at the mantissa's end where it has none
function layout(text: string): { mantissaEnd: number; point: number } {
	const exponent = text.search(/[eE]/);
	const mantissaEnd = exponent === -1 ? text.length : exponent;
	const point = text.indexOf(".");
	return { mantissaEnd, point: point === -1 || point > mantissaEnd ? mantissaEnd : point };
}

// The power of ten of the last written digit
function lowestPlace({ places, exponent }: Decimal): number {
	return (exponent || 0) - places;
}

// Written down to at least a place, with zeros added where it stops short of it
function extendedTo(decimal: Decimal, place: number): Decimal {
	const extra = lowestPlace(decimal) - place;
	if (extra <= 0) return decimal;

	return { ...decimal, units: decimal.units * 10n ** BigInt(extra), places: decimal.places + extra };
}

// Scientific notation keeps its mantissa from 1 up to 10, moving its exponent with every digit kept, like `10.0e-8` to `1.00e-7`
function normalized(decimal: Decimal): Decimal {
	if (decimal.exponent === undefined || decimal.units === 0n) return decimal;

	const shift = magnitudeOf(decimal.units).toString().length - 1 - decimal.places;
	return { ...decimal, places: decimal.places + shift, exponent: decimal.exponent + shift };
}

function magnitudeOf(units: bigint): bigint {
	return units < 0n ? -units : units;
}
