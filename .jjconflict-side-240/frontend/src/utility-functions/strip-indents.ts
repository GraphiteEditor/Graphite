export function stripIndents(stringPieces: TemplateStringsArray, ...substitutions: unknown[]): string {
	const interleavedSubstitutions = stringPieces.flatMap((stringPiece, index) => [stringPiece, substitutions[index] !== undefined ? substitutions[index] : ""]);
	const stringLines = interleavedSubstitutions.join("").split("\n");

	const visibleLineTabPrefixLengths = stringLines.map((line) => (/\S/.test(line) ? (line.match(/^(\t*)/) || [])[1].length : Infinity));
	const commonTabPrefixLength = Math.min(...visibleLineTabPrefixLengths);

	const linesWithoutCommonTabPrefix = stringLines.map((line) => line.substring(commonTabPrefixLength));
	const multiLineString = linesWithoutCommonTabPrefix.join("\n");

	return multiLineString.trim();
}
