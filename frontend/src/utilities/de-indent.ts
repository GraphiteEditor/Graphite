// From: https://github.com/MartinKolarik/dedent-js
export function deIndent(templateStrings: TemplateStringsArray | string, ...values: unknown[]) {
	const matches = [];
	const strings = typeof templateStrings === "string" ? [templateStrings] : templateStrings.slice();

	// 1. Remove trailing whitespace.
	strings[strings.length - 1] = strings[strings.length - 1].replace(/\r?\n([\t ]*)$/, "");

	// 2. Find all line breaks to determine the highest common indentation level.
	for (let i = 0; i < strings.length; i += 1) {
		const match = strings[i].match(/\n[\t ]+/g);
		if (match) {
			matches.push(...match);
		}
	}

	// 3. Remove the common indentation from all strings.
	if (matches.length) {
		const size = Math.min(...matches.map((value) => value.length - 1));
		const pattern = new RegExp(`\n[\t ]{${size}}`, "g");

		for (let i = 0; i < strings.length; i += 1) {
			strings[i] = strings[i].replace(pattern, "\n");
		}
	}

	// 4. Remove leading whitespace.
	strings[0] = strings[0].replace(/^\r?\n/, "");

	// 5. Perform interpolation.
	let string = strings[0];

	for (let i = 0; i < values.length; i += 1) {
		string += values[i] + strings[i + 1];
	}

	return string;
}
