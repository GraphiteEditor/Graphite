const { RuleTester } = require("eslint");
const myRule = require("./require-data-selectors");

const ruleTester = new RuleTester({ parserOptions: { ecmaVersion: 2015 } });

ruleTester.run(
	"require-data-selectors",
	myRule,
	{
		valid: [
			{ code: `document.querySelector("[data-foo]")` },
			{ code: `document.querySelector("[data-foo] [data-bar]")` },
			{ code: `document.querySelector("[data-foo] > [data-bar]")` },
			{ code: `document.querySelector("[data-foo]>[data-bar]")` },
			{ code: `document.querySelector("[data-foo] ~ [data-bar]")` },
			{ code: `document.querySelector("[data-foo]~[data-bar]")` },
			{ code: `document.querySelector("[data-foo] + [data-bar]")` },
			{ code: `document.querySelector("[data-foo]+[data-bar]")` },
			{ code: `document.querySelector(" [data-foo][data-bar] ")` },
		],
		invalid: [
			{ code: `document.querySelector(".foo")`, errors: 1 },
			{ code: `document.querySelector("div")`, errors: 1 },
			{ code: `document.querySelector("div.foo")`, errors: 1 },
			{ code: `document.querySelector("#foo")`, errors: 1 },
			{ code: `document.querySelector("div#foo")`, errors: 1 },
			{ code: `document.querySelector("div#foo.bar")`, errors: 1 },
			{ code: `document.querySelector("foo[data-bar]")`, errors: 1 },
			{ code: `document.querySelector("[foo][data-bar]")`, errors: 1 },
			{ code: `document.querySelector("[title]")`, errors: 1 },
			{ code: `document.querySelector("foo[title]")`, errors: 1 },
		],
	}
);

console.log("All tests passed!");
