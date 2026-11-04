use core_types::Ctx;
use core_types::list::Item;
use core_types::math::float_noise::round_away_float_noise;
use core_types::misc::{format_f64, parse_f64};
use math_parser::ast;
use math_parser::context::{EvalContext, NothingMap, ValueProvider};
use math_parser::value::Value;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use std::collections::HashMap;

/// The most symbols a generation may grow to, since each generation can multiply the string's length.
const MAX_SYMBOLS: usize = 1_000_000;

/// Why the "L-System" node's string stops short of what its inputs describe.
#[derive(Debug, thiserror::Error)]
pub enum LSystemError {
	#[error("Line {line} of the rules isn't a valid rule: {reason}")]
	InvalidRule { line: usize, reason: String },
	#[error("The axiom isn't valid: {0}")]
	InvalidAxiom(String),
	#[error("A rule's math gives no number: {0}")]
	Evaluation(String),
	#[error("Stopped after {completed} generations, since the next would be longer than a million symbols")]
	TooLong { completed: i64 },
}

/// Grows a string by rewriting all of its symbols at once with the rules, once per generation, for drawing with the **Turtle Path** node.
///
/// Since every symbol with a rule is replaced in each generation, patterns repeat inside themselves at ever finer scales, like the branching of plants or the folds of fractal curves.
#[node_macro::node(name("L-System"), category("String"), icon("NodeLSystem"))]
fn l_system(
	_: impl Ctx,
	/// The string to start growing from.
	axiom: Item<String>,
	/// One rule per line, like `F = F+F-F-F+F`, which replaces every `F` with `F+F-F-F+F` in each generation. Symbols without a rule stay as they are.
	///
	/// A rule can also check the symbols around one, as in `A < B > C = D`, which rewrites a `B` only between an `A` and a `C`. When several rules for a symbol end with a chance, like `F = F[+F]F : 0.6`, one of them is picked at random by those odds.
	///
	/// Symbols can carry numbers, like `F(10)`. A rule names them to work out new numbers with math, and to apply only when the condition after `:` holds, as in `F(x) : x > 2 = F(x/2)[+F(x/2)]`.
	///
	/// Lines starting with `//` are skipped.
	#[default("F = F+F-F-F+F")]
	#[widget(ParsedWidgetOverride::Custom = "text_area")]
	rules: Item<String>,
	/// Symbols to look past when a rule checks the symbols around one, usually the turns between them.
	#[default("+-")]
	ignored_symbols: Item<String>,
	/// How many times the rules rewrite the whole string. Each generation can multiply its length.
	#[default(3)]
	#[hard(0..)]
	generations: Item<i64>,
	/// Seed to determine unique variations on which rule is picked where several give chances.
	#[hard(0..)]
	seed: Item<i64>,
) -> Result<Item<String>, (Item<String>, LSystemError)> {
	// The grown string takes the axiom's place in its item, keeping the attributes it carries
	let mut string = axiom;

	let axiom = match parse_axiom(string.element()) {
		Ok(axiom) => axiom,
		Err(reason) => {
			string.element_mut().clear();
			return Err((string, LSystemError::InvalidAxiom(reason)));
		}
	};
	let (rule_set, rule_error) = RuleSet::parse(rules.element());
	let ignored_symbols: Vec<char> = ignored_symbols.element().chars().filter(|symbol| !symbol.is_whitespace()).collect();

	let (modules, rewrite_error) = rewrite(axiom, &rule_set, *generations.element(), &ignored_symbols, *seed.element() as u64);
	*string.element_mut() = modules_to_string(&modules);

	// An invalid rule is reported first, since it is usually being typed and explains any other surprise
	match rule_error.or(rewrite_error) {
		Some(error) => Err((string, error)),
		None => Ok(string),
	}
}

/// One symbol of an L-system string, with the numbers in parentheses after it, like `F(10, 2)`.
#[derive(Clone, Debug, PartialEq)]
struct Module {
	symbol: char,
	parameters: Vec<f64>,
}

/// A symbol a rule matches, with names for the numbers it carries, like `F(x, y)`.
struct Pattern {
	symbol: char,
	parameter_names: Vec<String>,
}

/// A symbol a rule writes, with the math that works out each of its numbers.
struct WrittenSymbol {
	symbol: char,
	parameters: Vec<ast::Node>,
}

/// One line of the rules: `left < symbol > right : condition = successor : chance`, where all but the symbol and successor are optional.
struct Rule {
	/// The symbols that must come before the rewritten one, in reading order.
	left_context: Vec<Pattern>,
	predecessor: Pattern,
	/// The symbols that must come after the rewritten one, in reading order.
	right_context: Vec<Pattern>,
	condition: Option<ast::Node>,
	successor: Vec<WrittenSymbol>,
	/// The odds of picking this rule among the other matching rules that give chances.
	chance: Option<f64>,
}

/// The rules, looked up by the symbol each one rewrites.
struct RuleSet {
	rules: Vec<Rule>,
	by_symbol: HashMap<char, Vec<usize>>,
	uses_context: bool,
}

impl RuleSet {
	/// Parses one rule per line, keeping the valid rules and the error of the first invalid one.
	fn parse(text: &str) -> (Self, Option<LSystemError>) {
		let mut rules = Vec::new();
		let mut first_error = None;

		for (index, line) in text.lines().enumerate() {
			let line = line.trim();
			if line.is_empty() || line.starts_with("//") {
				continue;
			}

			match parse_rule(line) {
				Ok(rule) => rules.push(rule),
				Err(reason) if first_error.is_none() => first_error = Some(LSystemError::InvalidRule { line: index + 1, reason }),
				Err(_) => {}
			}
		}

		let mut by_symbol: HashMap<char, Vec<usize>> = HashMap::new();
		for (index, rule) in rules.iter().enumerate() {
			by_symbol.entry(rule.predecessor.symbol).or_default().push(index);
		}
		let uses_context = rules.iter().any(|rule| !rule.left_context.is_empty() || !rule.right_context.is_empty());

		(Self { rules, by_symbol, uses_context }, first_error)
	}
}

/// The numbers a matched rule's names stand for.
#[derive(Default)]
struct Bindings<'a>(Vec<(&'a str, f64)>);

impl ValueProvider for Bindings<'_> {
	fn get_value(&self, name: &str) -> Option<Value> {
		self.0.iter().find(|(bound_name, _)| *bound_name == name).map(|&(_, value)| Value::from_f64(value))
	}
}

impl<'a> Bindings<'a> {
	/// Names the numbers of a module that matches the pattern, or gives `false` when the symbol or count of numbers differs.
	fn bind(&mut self, pattern: &'a Pattern, module: &Module) -> bool {
		if pattern.symbol != module.symbol || pattern.parameter_names.len() != module.parameters.len() {
			return false;
		}

		self.0.extend(pattern.parameter_names.iter().map(String::as_str).zip(module.parameters.iter().copied()));
		true
	}
}

/// Rewrites every module once per generation, giving the last complete generation and why it stopped early, if it did.
fn rewrite(axiom: Vec<Module>, rule_set: &RuleSet, generations: i64, ignored_symbols: &[char], seed: u64) -> (Vec<Module>, Option<LSystemError>) {
	let mut rng = StdRng::seed_from_u64(seed);
	let mut modules = axiom;

	for generation in 0..generations {
		let partners = if rule_set.uses_context { bracket_partners(&modules) } else { Vec::new() };
		let neighbors = Neighbors {
			modules: &modules,
			partners: &partners,
			ignored_symbols,
		};

		let mut next = Vec::with_capacity(modules.len());
		for (index, module) in modules.iter().enumerate() {
			let chosen = match choose_rule(rule_set, &neighbors, index, &mut rng) {
				Ok(chosen) => chosen,
				Err(reason) => return (modules, Some(LSystemError::Evaluation(reason))),
			};

			let Some((rule, bindings)) = chosen else {
				next.push(module.clone());
				continue;
			};
			for written in &rule.successor {
				let parameters = match written.parameters.iter().map(|expression| evaluate_number(expression, &bindings)).collect() {
					Ok(parameters) => parameters,
					Err(reason) => return (modules, Some(LSystemError::Evaluation(reason))),
				};
				next.push(Module { symbol: written.symbol, parameters });
			}

			if next.len() > MAX_SYMBOLS {
				return (modules, Some(LSystemError::TooLong { completed: generation }));
			}
		}

		modules = next;
	}

	(modules, None)
}

/// The rule that rewrites the module at `index` with its names bound. The first matching rule applies, unless it gives a chance, in which case one of the matching rules that give chances is picked by the odds.
fn choose_rule<'a>(rule_set: &'a RuleSet, neighbors: &Neighbors, index: usize, rng: &mut StdRng) -> Result<Option<(&'a Rule, Bindings<'a>)>, String> {
	let module = &neighbors.modules[index];
	let Some(candidates) = rule_set.by_symbol.get(&module.symbol) else { return Ok(None) };

	let mut chance_matches = Vec::new();
	for &rule_index in candidates {
		let rule = &rule_set.rules[rule_index];
		let Some(bindings) = matching_bindings(rule, neighbors, index)? else { continue };

		match rule.chance {
			Some(chance) => chance_matches.push((rule, bindings, chance)),
			None if chance_matches.is_empty() => return Ok(Some((rule, bindings))),
			None => {}
		}
	}

	let total: f64 = chance_matches.iter().map(|(_, _, chance)| chance).sum();
	if total <= 0. {
		return Ok(None);
	}

	let mut pick = rng.random::<f64>() * total;
	let last = chance_matches.len() - 1;
	for (position, (rule, bindings, chance)) in chance_matches.into_iter().enumerate() {
		if pick < chance || position == last {
			return Ok(Some((rule, bindings)));
		}
		pick -= chance;
	}

	Ok(None)
}

/// The rule's names bound to the numbers of the module at `index` and its context, or `None` when the rule doesn't match there.
fn matching_bindings<'a>(rule: &'a Rule, neighbors: &Neighbors, index: usize) -> Result<Option<Bindings<'a>>, String> {
	let mut bindings = Bindings::default();
	if !bindings.bind(&rule.predecessor, &neighbors.modules[index]) {
		return Ok(None);
	}

	// The left context is checked from the nearest symbol outward, so in reverse reading order
	let mut position = index;
	for pattern in rule.left_context.iter().rev() {
		let Some(previous) = neighbors.previous(position) else { return Ok(None) };
		if !bindings.bind(pattern, &neighbors.modules[previous]) {
			return Ok(None);
		}
		position = previous;
	}

	let mut position = index;
	for pattern in &rule.right_context {
		let Some(next) = neighbors.next(position) else { return Ok(None) };
		if !bindings.bind(pattern, &neighbors.modules[next]) {
			return Ok(None);
		}
		position = next;
	}

	if let Some(condition) = &rule.condition
		&& evaluate_number(condition, &bindings)? == 0.
	{
		return Ok(None);
	}

	Ok(Some(bindings))
}

/// A generation's modules, read the way context sees them: along the turtle's path, past side branches and ignored symbols.
struct Neighbors<'a> {
	modules: &'a [Module],
	/// The index of each bracket's partner, from [`bracket_partners`].
	partners: &'a [Option<usize>],
	ignored_symbols: &'a [char],
}

impl Neighbors<'_> {
	/// The nearest module before `index` on the way back toward the start, stepping over whole branches and out of the one `index` sits in.
	fn previous(&self, index: usize) -> Option<usize> {
		let mut current = index;
		while current > 0 {
			current -= 1;
			match self.modules[current].symbol {
				']' => current = self.partners[current]?,
				'[' => {}
				symbol if self.ignored_symbols.contains(&symbol) => {}
				_ => return Some(current),
			}
		}
		None
	}

	/// The nearest module after `index` along the same branch, stepping over side branches, or `None` where the branch ends.
	fn next(&self, index: usize) -> Option<usize> {
		let mut current = index + 1;
		while current < self.modules.len() {
			match self.modules[current].symbol {
				'[' => current = self.partners[current]? + 1,
				']' => return None,
				symbol if self.ignored_symbols.contains(&symbol) => current += 1,
				_ => return Some(current),
			}
		}
		None
	}
}

/// The index of each `[` and `]` module's partner, so context matching can step over whole branches in O(1).
fn bracket_partners(modules: &[Module]) -> Vec<Option<usize>> {
	let mut partners = vec![None; modules.len()];
	let mut open = Vec::new();

	for (index, module) in modules.iter().enumerate() {
		match module.symbol {
			'[' => open.push(index),
			']' => {
				if let Some(opening) = open.pop() {
					partners[index] = Some(opening);
					partners[opening] = Some(index);
				}
			}
			_ => {}
		}
	}

	partners
}

/// Writes modules back out as a string, with each one's numbers in parentheses after it.
fn modules_to_string(modules: &[Module]) -> String {
	let mut text = String::with_capacity(modules.len());

	for module in modules {
		text.push(module.symbol);
		if !module.parameters.is_empty() {
			let numbers: Vec<String> = module.parameters.iter().map(|&number| format_f64(number)).collect();
			text.push('(');
			text.push_str(&numbers.join(","));
			text.push(')');
		}
	}

	text
}

/// Reads the axiom's modules, whose numbers may be written as math like `F(10*2)`.
fn parse_axiom(text: &str) -> Result<Vec<Module>, String> {
	split_symbols(text)?
		.into_iter()
		.map(|(symbol, parameters)| {
			let parameters = arguments(parameters).into_iter().map(evaluate_constant).collect::<Result<_, _>>()?;
			Ok(Module { symbol, parameters })
		})
		.collect()
}

/// Parses one line of the rules.
fn parse_rule(line: &str) -> Result<Rule, String> {
	let Some(separator) = rule_separator(line) else {
		return Err("it needs an `=` between the symbol and what it becomes".to_string());
	};
	let (left_side, right_side) = (&line[..separator], &line[separator + 1..]);

	// The left side reads `left < symbol > right : condition`, where all but the symbol are optional
	let (pattern_text, condition_text) = split_at_first(left_side, ':');
	let (left_context_text, rest) = match split_at_first(pattern_text, '<') {
		(left_context, Some(rest)) => (Some(left_context), rest),
		(rest, None) => (None, rest),
	};
	let (predecessor_text, right_context_text) = split_at_first(rest, '>');

	let mut predecessor = parse_patterns(predecessor_text)?;
	if predecessor.len() != 1 {
		return Err("the left side must name exactly one symbol to rewrite".to_string());
	}
	let predecessor = predecessor.remove(0);

	let left_context = left_context_text.map(parse_patterns).transpose()?.unwrap_or_default();
	let right_context = right_context_text.map(parse_patterns).transpose()?.unwrap_or_default();
	let condition = condition_text.map(parse_expression).transpose()?;

	// The right side reads `successor : chance`, where the chance is optional
	let (successor_text, chance_text) = match right_side.rfind(':') {
		Some(colon) => (&right_side[..colon], Some(right_side[colon + 1..].trim())),
		None => (right_side, None),
	};
	let chance = chance_text
		.map(|text| {
			parse_f64(text)
				.filter(|chance| *chance >= 0.)
				.ok_or_else(|| format!("the chance `{text}` isn't a number of zero or more"))
		})
		.transpose()?;

	let successor = split_symbols(successor_text)?
		.into_iter()
		.map(|(symbol, parameters)| {
			let parameters = arguments(parameters).into_iter().map(parse_expression).collect::<Result<_, _>>()?;
			Ok(WrittenSymbol { symbol, parameters })
		})
		.collect::<Result<_, String>>()?;

	Ok(Rule {
		left_context,
		predecessor,
		right_context,
		condition,
		successor,
		chance,
	})
}

/// Reads symbols with names for their numbers, like `A(x)B`, as a rule's predecessor or context.
fn parse_patterns(text: &str) -> Result<Vec<Pattern>, String> {
	split_symbols(text)?
		.into_iter()
		.map(|(symbol, parameters)| {
			let parameter_names = arguments(parameters)
				.into_iter()
				.map(|name| {
					let name = name.trim();
					let mut characters = name.chars();
					let valid = characters.next().is_some_and(|first| first.is_ascii_lowercase()) && characters.all(|character| character.is_ascii_alphanumeric() || character == '_');
					if valid {
						Ok(name.to_string())
					} else {
						Err(format!("`{name}` can't name a number, since names start with a lowercase letter, like `x`"))
					}
				})
				.collect::<Result<_, _>>()?;
			Ok(Pattern { symbol, parameter_names })
		})
		.collect()
}

/// The byte index of the `=` between a rule's two sides, passing over the ones in comparisons like `==`, `<=`, `>=` and `!=`.
fn rule_separator(line: &str) -> Option<usize> {
	let bytes = line.as_bytes();
	(0..bytes.len()).find(|&index| {
		let previous = index.checked_sub(1).map(|previous| bytes[previous]);
		bytes[index] == b'=' && !matches!(previous, Some(b'=' | b'<' | b'>' | b'!')) && bytes.get(index + 1) != Some(&b'=')
	})
}

/// Splits the text at the first `delimiter` outside parentheses, giving the text before it and, if found, the text after it.
fn split_at_first(text: &str, delimiter: char) -> (&str, Option<&str>) {
	let mut depth = 0;
	for (index, character) in text.char_indices() {
		match character {
			'(' => depth += 1,
			')' => depth -= 1,
			_ if character == delimiter && depth == 0 => return (&text[..index], Some(&text[index + character.len_utf8()..])),
			_ => {}
		}
	}
	(text, None)
}

/// Reads a string as symbols, each with the text inside any parentheses right after it. Whitespace between symbols is skipped.
fn split_symbols(text: &str) -> Result<Vec<(char, Option<&str>)>, String> {
	let mut symbols = Vec::new();
	let mut characters = text.char_indices().peekable();

	while let Some((index, character)) = characters.next() {
		if character.is_whitespace() {
			continue;
		}
		if matches!(character, '(' | ')') {
			return Err(format!("`{character}` must come right after a symbol, like `F(10)`"));
		}

		let after = index + character.len_utf8();
		if !text[after..].starts_with('(') {
			symbols.push((character, None));
			continue;
		}

		let Some(close) = closing_parenthesis(text, after) else {
			return Err(format!("the `(` after `{character}` is never closed"));
		};
		symbols.push((character, Some(&text[after + 1..close])));
		while characters.next_if(|&(index, _)| index <= close).is_some() {}
	}

	Ok(symbols)
}

/// The byte index of the `)` that closes the `(` at `open`.
fn closing_parenthesis(text: &str, open: usize) -> Option<usize> {
	let mut depth = 0;
	for (index, character) in text[open..].char_indices() {
		match character {
			'(' => depth += 1,
			')' => {
				depth -= 1;
				if depth == 0 {
					return Some(open + index);
				}
			}
			_ => {}
		}
	}
	None
}

/// Splits the text inside a symbol's parentheses at its top-level commas, keeping the commas of nested calls like `max(a, b)`.
fn arguments(parameters: Option<&str>) -> Vec<&str> {
	let Some(text) = parameters.filter(|text| !text.trim().is_empty()) else { return Vec::new() };

	let mut pieces = Vec::new();
	let mut depth = 0;
	let mut start = 0;
	for (index, character) in text.char_indices() {
		match character {
			'(' => depth += 1,
			')' => depth -= 1,
			',' if depth == 0 => {
				pieces.push(&text[start..index]);
				start = index + 1;
			}
			_ => {}
		}
	}
	pieces.push(&text[start..]);

	pieces
}

fn parse_expression(text: &str) -> Result<ast::Node, String> {
	ast::Node::try_parse_from_str(text.trim()).map_err(|error| error.to_string())
}

/// Reads a written number, or works out the math of an expression like `10*2`.
fn evaluate_constant(text: &str) -> Result<f64, String> {
	if let Some(number) = parse_f64(text.trim()) {
		return Ok(number);
	}
	evaluate_number(&parse_expression(text)?, &Bindings::default())
}

/// Works out the math of an expression as a number, rounding away float noise so the string stays readable.
fn evaluate_number(expression: &ast::Node, bindings: &Bindings) -> Result<f64, String> {
	let result = expression.eval(&EvalContext::new(bindings, NothingMap)).map_err(|error| error.to_string())?;
	let number = result.as_value().and_then(|value| value.as_real()).ok_or_else(|| format!("`{result}` isn't a number"))?;
	Ok(round_away_float_noise(number))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn grow(axiom: &str, rules: &str, generations: i64) -> Result<String, (String, String)> {
		grow_with(axiom, rules, generations, "+-", 0)
	}

	fn grow_with(axiom: &str, rules: &str, generations: i64, ignored_symbols: &str, seed: i64) -> Result<String, (String, String)> {
		let item = |text: &str| Item::new_from_element(text.to_string());
		l_system((), item(axiom), item(rules), item(ignored_symbols), Item::new_from_element(generations), Item::new_from_element(seed))
			.map(Item::into_element)
			.map_err(|(result, error)| (result.into_element(), error.to_string()))
	}

	#[test]
	fn rewrites_every_symbol_at_once() {
		assert_eq!(grow("F", "F = F+F-F-F+F", 1).unwrap(), "F+F-F-F+F");
		assert_eq!(grow("FX", "X = X+YF+\nY = -FX-Y", 2).unwrap(), "FX+YF++-FX-YF+");
		assert_eq!(grow("FX", "X = X+YF+", 0).unwrap(), "FX");
	}

	#[test]
	fn skips_whitespace_and_comment_lines() {
		assert_eq!(grow("F G", "// F = G\n\nF = F + F", 1).unwrap(), "F+FG");
	}

	#[test]
	fn matches_context_past_ignored_symbols_and_branches() {
		assert_eq!(grow("ABC", "A < B > C = X", 1).unwrap(), "AXC");
		assert_eq!(grow("ABD", "A < B > C = X", 1).unwrap(), "ABD");
		assert_eq!(grow("A+BC", "A < B > C = X", 1).unwrap(), "A+XC");

		// Left context steps out of the branch a symbol starts and over whole branches before it
		assert_eq!(grow("A[B]", "A < B = X", 1).unwrap(), "A[X]");
		assert_eq!(grow("A[C]B", "A < B = X", 1).unwrap(), "A[C]X");

		// Right context ends with the branch
		assert_eq!(grow("[AB]", "A > B = X", 1).unwrap(), "[XB]");
		assert_eq!(grow("[A]B", "A > B = X", 1).unwrap(), "[A]B");
	}

	#[test]
	fn the_first_matching_rule_applies() {
		assert_eq!(grow("ABB", "A < B = X\nB = Y", 1).unwrap(), "AXY");
	}

	#[test]
	fn works_out_parameters_and_conditions() {
		let rules = "F(x) : x > 2 = F(x/2)[+F(x/2)]";
		assert_eq!(grow("F(10)", rules, 1).unwrap(), "F(5)[+F(5)]");
		assert_eq!(grow("F(5*2)", rules, 2).unwrap(), "F(2.5)[+F(2.5)][+F(2.5)[+F(2.5)]]");

		// Below the condition's threshold, nothing changes
		assert_eq!(grow("F(1)", rules, 3).unwrap(), "F(1)");

		// A symbol carrying a different count of numbers doesn't match
		assert_eq!(grow("F(1,2)F", rules, 1).unwrap(), "F(1,2)F");

		// Context binds its own names
		assert_eq!(grow("A(3)B(4)", "A(x) < B(y) = B(x+y)", 1).unwrap(), "A(3)B(7)");
	}

	#[test]
	fn picks_among_chances_the_same_way_for_a_seed() {
		let rules = "F = A : 1\nF = B : 1";
		let first = grow_with("FFFFFFFFFFFFFFFFFFFF", rules, 1, "", 7).unwrap();
		assert_eq!(first, grow_with("FFFFFFFFFFFFFFFFFFFF", rules, 1, "", 7).unwrap());
		assert!(first.contains('A') && first.contains('B'));

		// A chance of zero is never picked
		assert_eq!(grow_with("FFFF", "F = A : 0\nF = B : 1", 1, "", 3).unwrap(), "BBBB");
	}

	#[test]
	fn reports_the_first_invalid_line_and_keeps_the_valid_rules() {
		let (result, error) = grow("F", "F = FF\nnonsense\nG(X) = G", 1).unwrap_err();
		assert_eq!(result, "FF");
		assert_eq!(error, "Line 2 of the rules isn't a valid rule: it needs an `=` between the symbol and what it becomes");

		let (_, error) = grow("F", "G(X) = G", 1).unwrap_err();
		assert!(error.starts_with("Line 1 of the rules isn't a valid rule: `X` can't name a number"));

		let (result, error) = grow("F(", "", 1).unwrap_err();
		assert_eq!(result, "");
		assert_eq!(error, "The axiom isn't valid: the `(` after `F` is never closed");
	}

	#[test]
	fn stops_before_a_generation_grows_too_long() {
		let (result, error) = grow("F", "F = FFFFFFFFFF", 7).unwrap_err();
		assert_eq!(result.len(), 1_000_000);
		assert_eq!(error, "Stopped after 6 generations, since the next would be longer than a million symbols");
	}
}
