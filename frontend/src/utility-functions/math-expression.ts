// The glyphs a math expression is displayed with, math alphabet letters for its names and typeset arithmetic operators, which stand for the plain characters typed

// The math alphabets are contiguous runs of the Mathematical Alphanumeric Symbols block, except the italic `h`,
// which Unicode had already placed at U+210E for Planck's constant
const ALPHABETS: { style: "italic" | "bold"; first: number; last: number; glyphs: number }[] = [
	{ style: "italic", first: 0x41, last: 0x5a, glyphs: 0x1d434 },
	{ style: "italic", first: 0x61, last: 0x7a, glyphs: 0x1d44e },
	{ style: "italic", first: 0x391, last: 0x3a9, glyphs: 0x1d6e2 },
	{ style: "italic", first: 0x3b1, last: 0x3c9, glyphs: 0x1d6fc },
	{ style: "bold", first: 0x41, last: 0x5a, glyphs: 0x1d400 },
	{ style: "bold", first: 0x61, last: 0x7a, glyphs: 0x1d41a },
	{ style: "bold", first: 0x391, last: 0x3a9, glyphs: 0x1d6a8 },
	{ style: "bold", first: 0x3b1, last: 0x3c9, glyphs: 0x1d6c2 },
];
const ITALIC_H = 0x210e;
// The capitals' slot for the unassigned U+03A2 holds each math alphabet's capital theta symbol
const UNASSIGNED_CAPITAL = 0x3a2;
const CAPITAL_THETA_SYMBOL = "ϴ";

// The minus sign, dot operator, and division slash
const OPERATOR_GLYPHS: Record<string, string> = { "-": "−", "*": "⋅", "/": "∕" };
const OPERATORS_BY_GLYPH: Record<string, string> = Object.fromEntries(Object.entries(OPERATOR_GLYPHS).map(([operator, glyph]) => [glyph, operator]));

// Symbols beyond the displayed operator glyphs that a pasted formula is likely written with, and a Windows line break's carriage return, read as what's typed for them
const PASTED_SPELLINGS: Record<string, string> = { "×": "*", "·": "*", "÷": "/", "²": "^2", "³": "^3", "\r": "" };

// The glyph a letter is displayed with in the given math alphabet, or the character itself where no such glyph exists
export function mathGlyph(character: string, style: "italic" | "bold"): string {
	const code = character.codePointAt(0);
	if (code === undefined) return character;
	if (style === "italic" && code === 0x68) return String.fromCodePoint(ITALIC_H);

	const letter = character === CAPITAL_THETA_SYMBOL ? UNASSIGNED_CAPITAL : code;
	const alphabet = ALPHABETS.find((alphabet) => alphabet.style === style && letter >= alphabet.first && letter <= alphabet.last);
	return alphabet ? String.fromCodePoint(alphabet.glyphs + letter - alphabet.first) : character;
}

// The glyph an arithmetic operator is displayed with, or the character itself where it is no such operator
export function operatorGlyph(character: string): string {
	return OPERATOR_GLYPHS[character] || character;
}

// What's typed for a displayed glyph or a pasted symbol, or the character itself where it is neither
function fromMathGlyph(character: string): string {
	const code = character.codePointAt(0);
	if (code === undefined) return character;
	if (code === ITALIC_H) return "h";
	if (character in OPERATORS_BY_GLYPH) return OPERATORS_BY_GLYPH[character];
	if (character in PASTED_SPELLINGS) return PASTED_SPELLINGS[character];

	const alphabet = ALPHABETS.find((alphabet) => code >= alphabet.glyphs && code <= alphabet.glyphs + alphabet.last - alphabet.first);
	if (!alphabet) return character;

	const typed = alphabet.first + code - alphabet.glyphs;
	return typed === UNASSIGNED_CAPITAL ? CAPITAL_THETA_SYMBOL : String.fromCodePoint(typed);
}

// The text typed for a run of displayed glyphs or pasted symbols
export function fromMathGlyphs(display: string): string {
	return Array.from(display, fromMathGlyph).join("");
}
