// An ESLint rule requiring DOM queries to select elements by their `data-*` attributes, which markup sets for scripts to find it by,
// rather than by element types, classes, IDs, or other attributes, which are free to change with the markup's structure and styling

import { pathToFileURL } from "node:url";
import parseSelector from "postcss-selector-parser";

// Stands in for each `${…}` interpolation of a template literal while its selector is read
const INTERPOLATION = String.fromCodePoint(0xe000);

const SELECTOR_METHODS = ["querySelector", "querySelectorAll", "closest", "matches"];
const ELEMENTS_BY_METHODS = ["getElementById", "getElementsByClassName", "getElementsByName", "getElementsByTagName", "getElementsByTagNameNS"];

// Pseudo-classes taking a list of selectors, which are held to the rule too, where `:has()` takes relative ones that may begin with a combinator
const SELECTOR_LIST_PSEUDO_CLASSES = ["not", "is", "where", "matches", "-webkit-any", "-moz-any", "host", "host-context"];
const RELATIVE_SELECTOR_LIST_PSEUDO_CLASSES = ["has"];
// Pseudo-classes taking a microsyntax that may end with `of` and a list of selectors
const OF_SELECTOR_PSEUDO_CLASSES = ["nth-child", "nth-last-child"];

const MALFORMED = { messageId: "malformed", part: "" };

const requireDataSelectors = {
	meta: {
		type: "problem",
		docs: {
			description: "Require DOM queries to select elements by `data-*` attributes rather than by element type, class, ID, or other attributes.",
		},
		schema: [],
		messages: {
			typeSelector: "Select by a `data-*` attribute rather than the element type `{{part}}`.",
			classSelector: "Select by a `data-*` attribute rather than the class `{{part}}`, since classes are for styling.",
			idSelector: "Select by a `data-*` attribute rather than the ID `{{part}}`.",
			attributeSelector: "Select by a `data-*` attribute rather than the attribute `{{part}}`.",
			malformed: "This CSS selector is malformed, with an unbalanced bracket, parenthesis, quote, or combinator.",
			elementsBy: "Select with `querySelector` or `querySelectorAll` and a `data-*` attribute rather than with `{{part}}`.",
		},
	},
	create: (context) => ({
		CallExpression: (node) => {
			const method = methodName(node.callee);
			if (method === undefined) return;

			if (ELEMENTS_BY_METHODS.includes(method)) {
				context.report({ node: node.callee.property, messageId: "elementsBy", data: { part: method } });
				return;
			}
			if (!SELECTOR_METHODS.includes(method)) return;

			const argument = node.arguments[0];
			const selector = selectorText(argument);
			if (selector === undefined) return;

			const problem = selectorListProblem(selector, false);
			if (problem) context.report({ node: argument, messageId: problem.messageId, data: { part: problem.part.replaceAll(INTERPOLATION, "${…}") } });
		},
	}),
};
export default requireDataSelectors;

// The name of the method a call is made on, like `querySelector` in `element.querySelector(…)` or `element["querySelector"](…)`
function methodName(callee) {
	if (callee.type !== "MemberExpression") return undefined;
	if (!callee.computed && callee.property.type === "Identifier") return callee.property.name;
	if (callee.computed && callee.property.type === "Literal" && typeof callee.property.value === "string") return callee.property.value;
	return undefined;
}

// A selector written as a string or template literal, with each interpolation marked, or `undefined` where it's computed some other way
function selectorText(argument) {
	if (argument?.type === "Literal" && typeof argument.value === "string") return argument.value;
	if (argument?.type === "TemplateLiteral") return argument.quasis.map((quasi) => quasi.value.cooked ?? quasi.value.raw).join(INTERPOLATION);
	return undefined;
}

// The first problem with a list of comma-separated selectors, if any, where a relative list may begin each selector with a combinator
function selectorListProblem(list, relative) {
	let parsed;
	try {
		parsed = parseSelector().astSync(list);
	} catch {
		return MALFORMED;
	}

	return selectorsProblem(parsed.nodes, relative);
}

// The first problem with parsed selectors, if any, which the parser leaves to be found where its combinators stand or what its parts name
function selectorsProblem(selectors, relative) {
	return selectors.map((selector) => combinatorProblem(selector.nodes, relative) ?? selector.nodes.map(nodeProblem).find(Boolean)).find(Boolean);
}

// A selector with no parts, or a combinator with no compound selector on either side of it, which the parser accepts but a browser rejects
function combinatorProblem(nodes, relative) {
	if (nodes.length === 0) return MALFORMED;

	const combinator = (node) => node?.type === "combinator";
	if (combinator(nodes[0]) && !relative) return MALFORMED;
	if (combinator(nodes[nodes.length - 1])) return MALFORMED;
	if (nodes.some((node, index) => combinator(node) && combinator(nodes[index + 1]))) return MALFORMED;

	return undefined;
}

// The problem with one part of a compound selector, if any
function nodeProblem(node) {
	const part = String(node).trim();

	switch (node.type) {
		// An interpolation may stand for a whole selector rather than an element type, so it's left unchecked
		case "tag":
			return node.value.includes(INTERPOLATION) ? undefined : { messageId: "typeSelector", part };
		// A class or ID is one whatever an interpolation names it
		case "class":
			return { messageId: "classSelector", part };
		case "id":
			return { messageId: "idSelector", part };
		// An interpolated name may be anything, including a `data-*` attribute
		case "attribute": {
			const name = node.attribute.toLowerCase();
			return name.startsWith("data-") || name.includes(INTERPOLATION) ? undefined : { messageId: "attributeSelector", part };
		}
		case "pseudo":
			return pseudoClassArgumentProblem(node);
		// The universal and nesting selectors, and combinators, name nothing
		default:
			return undefined;
	}
}

// The first problem with the selectors a pseudo-class takes as its argument, if it takes any, where pseudo-elements take none
function pseudoClassArgumentProblem(node) {
	if (node.value.startsWith("::")) return undefined;
	const name = node.value.slice(1).toLowerCase();

	if (SELECTOR_LIST_PSEUDO_CLASSES.includes(name)) return selectorsProblem(node.nodes, false);
	if (RELATIVE_SELECTOR_LIST_PSEUDO_CLASSES.includes(name)) return selectorsProblem(node.nodes, true);

	// The parser reads a microsyntax like `2n + 1` as selectors too, so only the selectors after `of` are read again on their own
	if (OF_SELECTOR_PSEUDO_CLASSES.includes(name)) {
		const written = String(node).trim();
		const argument = written.slice(node.value.length + 1, -1);
		const of = /\sof\s/i.exec(argument);
		return of ? selectorListProblem(argument.slice(of.index + of[0].length), false) : undefined;
	}

	return undefined;
}

// Running this file directly with `node eslint-require-data-selectors.js` tests the rule after a change, where RuleTester throws at the first case that fails
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	const { RuleTester } = await import("eslint");

	const query = (selector) => `element.querySelector(${selector})`;
	const invalid = (code, messageId) => ({ code, errors: [{ messageId }] });

	new RuleTester().run("require-data-selectors", requireDataSelectors, {
		valid: [
			// Data attributes joined by every combinator, spaced or not, and compounded
			query(`"[data-foo]"`),
			query(`"[data-foo] [data-bar]"`),
			query(`"[data-foo] > [data-bar]"`),
			query(`"[data-foo]>[data-bar]"`),
			query(`"[data-foo] ~ [data-bar]"`),
			query(`"[data-foo]~[data-bar]"`),
			query(`"[data-foo] + [data-bar]"`),
			query(`"[data-foo]+[data-bar]"`),
			query(`" [data-foo][data-bar] "`),
			query(`"[data-foo], [data-bar] [data-baz]"`),

			// Attribute values, operators, and case flags
			query(`"[data-foo='a, b] (c)']"`),
			query(`"[data-foo = \\"x\\"]"`),
			query(`"[data-foo~=x i]"`),
			query(`"[data-foo|=en]"`),
			query(`"[DATA-FOO]"`),

			// Pseudo-classes, and the universal selector, which name no element type
			query(`":scope > [data-tab]"`),
			query(`"[data-input]:enabled"`),
			query(`":hover"`),
			query(`"[data-foo] *"`),
			query(`"[data-foo]::before"`),
			query(`"[data-foo]:not([data-bar])"`),
			query(`":is([data-a], [data-b]) > [data-c]"`),
			query(`":where([data-a])"`),
			query(`"[data-a]:has(> [data-b], + [data-c])"`),
			query(`"[data-list] > :nth-child(2n + 1 of [data-item])"`),
			query(`"[data-list] > :nth-child(2n+1)"`),

			// Interpolations in an attribute's value or name, or standing for whole selectors
			query(`\`[data-token="\${index}"]\``),
			query("`[data-${name}]`"),
			query("`[${attribute}]`"),
			query("`${selector} > [data-foo]`"),
			query("`:is(${selectors})`"),

			// Every method that takes a selector, called on anything, optionally, or by a computed name
			`element.querySelectorAll("[data-foo]")`,
			`element.closest("[data-foo]")`,
			`element.matches(":focus-visible")`,
			`element?.querySelector("[data-foo]")`,
			`element["closest"]("[data-foo]")`,

			// Selectors computed some other way, and other methods
			`element.querySelector(selector)`,
			`element.closest(prefix + "foo")`,
			`list.find(".foo")`,
			`element.getAttribute("class")`,
			`layers.getElementsByDepth(2)`,
		],
		invalid: [
			// Element types, classes, IDs, and other attributes
			invalid(query(`".foo"`), "classSelector"),
			invalid(query(`"div"`), "typeSelector"),
			invalid(query(`"div.foo"`), "typeSelector"),
			invalid(query(`"#foo"`), "idSelector"),
			invalid(query(`"div#foo"`), "typeSelector"),
			invalid(query(`"div#foo.bar"`), "typeSelector"),
			invalid(query(`"svg|rect"`), "typeSelector"),
			invalid(query(`"foo[data-bar]"`), "typeSelector"),
			invalid(query(`"[foo][data-bar]"`), "attributeSelector"),
			invalid(query(`"[data-bar][foo]"`), "attributeSelector"),
			invalid(query(`"[title]"`), "attributeSelector"),
			invalid(query(`"foo[title]"`), "typeSelector"),
			invalid(query(`"[data-foo] input"`), "typeSelector"),
			invalid(query(`"[data-foo] > .bar"`), "classSelector"),
			invalid(query(`"[data-foo], .bar"`), "classSelector"),

			// Within the selectors a pseudo-class takes
			invalid(query(`"[data-foo]:not(.bar)"`), "classSelector"),
			invalid(query(`"[data-foo]:not([disabled])"`), "attributeSelector"),
			invalid(query(`":is([data-a], div)"`), "typeSelector"),
			invalid(query(`":has(> span)"`), "typeSelector"),
			invalid(query(`":nth-child(2 of .item)"`), "classSelector"),

			// A class or attribute is one whatever an interpolation names it, as is a non-data attribute with an interpolated value
			invalid(query("`.${name}`"), "classSelector"),
			invalid(query(`\`[for="checkbox-\${id}"]\``), "attributeSelector"),

			// Unbalanced brackets, parentheses, quotes, or combinators
			invalid(query(`"[data-foo"`), "malformed"),
			invalid(query(`"[data-foo]]"`), "malformed"),
			invalid(query(`"[data-foo]:not([data-bar]"`), "malformed"),
			invalid(query(`"[data-foo='bar]"`), "malformed"),
			invalid(query(`"[data-foo] >"`), "malformed"),
			invalid(query(`"> [data-foo]"`), "malformed"),
			invalid(query(`"[data-foo] > > [data-bar]"`), "malformed"),
			invalid(query(`"[data-a],,[data-b]"`), "malformed"),
			invalid(query(`""`), "malformed"),

			// Every method that takes a selector, and those selecting by other means
			invalid(`element.closest("button")`, "typeSelector"),
			invalid(`element.matches(".foo")`, "classSelector"),
			invalid(`self?.div()?.querySelector(".text-input input")`, "classSelector"),
			invalid(`element["querySelectorAll"]("div")`, "typeSelector"),
			invalid(`document.getElementById("foo")`, "elementsBy"),
			invalid(`element.getElementsByClassName("foo")`, "elementsBy"),
			invalid(`element.getElementsByTagName("div")`, "elementsBy"),
			invalid(`document.getElementsByName("foo")`, "elementsBy"),
		],
	});
}
