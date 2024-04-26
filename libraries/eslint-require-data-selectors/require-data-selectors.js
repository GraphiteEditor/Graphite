// Custom rule docs <https://eslint.org/docs/latest/extend/custom-rule-tutorial>
//
// Helpful tool 1 <https://astexplorer.net/>:
//
// document.querySelector("[data-foo]");
// window.body.querySelector("[data-foo]", 456, true);
//
// Helpful tool 2 <https://estools.github.io/esquery/> (querying for `[id.name="bar"]`):
//
// document.querySelector("[data-foo]");
// window.body.querySelectorAll("[data-foo] [foo]").closest(null).matches("#bar", 123);

module.exports = {
	meta: {
		type: "problem",
		docs: {
			description:
				"Enforce that CSS selector strings used with `querySelector`, `querySelectorAll`, `closest`, and `matches` only contains `data-*` attributes and disallow `getElementsBy*` APIs.",
		},
		schema: [],
	},
	create: (context) => ({
		// After <https://github.com/estools/esquery/issues/132> was fixed, we were able to add the `> ` direct child combinator to the start of the `:has()` pseudo-selector (before `MemberExpression`) which was needed for better correctness. But this alteration hasn't been tested yet. This comment can be deleted after testing it.
		"CallExpression:has(> MemberExpression > Identifier[name=/^querySelector(All)?|closest|matches$/]) > .arguments:first-child:last-child[raw=/^\".*\"$/]": (node) => {
			const split = deepAssemble(buildParenthesesTree(node.value));
			if (split === undefined) {
				context.report({
					node,
					message: "Unbalanced parentheses in CSS selector string.",
				});
			} else {
				// Split between selector combinators
				const list = node.value.split(/ +| *> *| *~ *| *\+ *|(?<=.)(?=\[data-.+\])/).filter((s) => s !== "");
				// console.log(list);
				const valid = list.every((s) => /^\[data-[^\]]+\]$/.test(s));

				if (!valid) {
					context.report({
						node,
						message: "CSS selector strings should only contain `data-*` attributes and avoid tag type, class, ID, or non-data attribute selectors.",
					});
				}
			}
		},
		// TODO: Disallow `getElements?By*` APIs.
	}),
};

function* cartesian_product(head, ...tail) {
	const remainder = tail.length > 0 ? cartesian_product(...tail) : [[]];
	for (let r of remainder) for (let h of head) yield [h, ...r];
}

function deepAssemble(sequence) {
	const assembled = assemble(sequence);
	console.log(assembled);
	console.log("============");

	assembled.forEach((group) => {
		// TODO: Flatten contiguous arrays
		
		// group.forEach((item) => {
		// 	if (Array.isArray(item)) {
		// 		const result = assemble(item);
		// 		result.forEach((x) => {
		// 			console.log("SUB:", x);
		// 		})
		// 	} else {
		// 		console.log("TOP:", item);
		// 	}
		// });
		console.log(group);
	});
}

function assemble(sequence) {
	const output = [];

	const flatSequence = sequence.flat();
	for (let i = 0; i < flatSequence.length; i++) {
		const item = flatSequence[i];
		const previous = output[output.length - 1];
		const previousLast = previous?.[previous.length - 1];

		// Append to the last group if this item is an array, the previous group's last item was an array, or the previous group's last item ended with an open parenthesis
		if (Array.isArray(item) || Array.isArray(previousLast) || previousLast?.endsWith("(")) {
			output[output.length - 1].push(item);
		} else {
			output.push([item]);
		}
	}

	return output;
}

function buildParenthesesTree(givenString) {
	// Preprocess commas for multiple selectors, but be careful about commas within pseudo-selectors:
	//
	// a, div:not(.fun, .boring), img, foo > .bar ~ :has(> div, span:is([data-red], [data-blue] .bold:not(&.italic), .italic)), qux:not(.foo):not(.bar)
	// ================================================================================================================================================
	// a, div:not(             ), img, foo > .bar ~ :has(                                                                    ), qux:not(    ):not(    )
	//            .fun, .boring                          > div, span:is(                                                    )           .foo      .bar
	//                                                                  [data-red], [data-blue] .bold:not(        ), .italic
	//                                                                                                    &.italic
	let balancedCount = 0;
	let stringGroups = [""];

	const balanced = Array.from(givenString).every((c) => {
		if (c === ")") balancedCount -= 1;
		if (balancedCount < 0) return false;

		if (c === "(" && balancedCount === 0) {
			stringGroups[stringGroups.length - 1] += "(";
			stringGroups.push("");
		} else if (c === ")" && balancedCount === 0) {
			stringGroups.push(")");
		} else {
			stringGroups[stringGroups.length - 1] += c;
		}

		if (c === "(") balancedCount += 1;

		return true;
	});

	if (!balanced || balancedCount !== 0) return undefined;

	return stringGroups.map((text, i) => i % 2 === 1 ? buildParenthesesTree(text) : text.split(/ *, */));
}

debugger;

