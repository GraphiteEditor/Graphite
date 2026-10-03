//! The names an editor offers to finish the one being typed.

use crate::constants::{BUILTIN_FUNCTION_ALIASES, SUFFIXED_FORMS};
use crate::documentation::{BUILTIN_FUNCTIONS, Operator, Tooltip, constant_tooltip, function_tooltip, identity_tooltip, keyword_tooltip};
use crate::lexer::{CONSTANT_SPELLINGS, Constant, KEYWORDS, Lexer, Token, offset_in, operand_ended};
use crate::parser::{keywords_accepted_after, parse_quietly};
use crate::sort::names_in_scope;
use std::borrow::Cow;

/// What an offered name is, by which the offers are grouped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionKind {
	/// A value, matrix, or function a `where` clause or a function's parameters define around the name being typed.
	Local,
	Function,
	Constant,
	Keyword,
	/// The typeset alias of an operator typed in ASCII, like `≤` for `<=`.
	Operator,
}

/// A name or symbol offered to finish what's being typed, spelled as it's written in place of what's typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer<'src> {
	/// The spelling, with the `\` prefix where a builtin needs it to be reached.
	pub name: Cow<'src, str>,
	pub kind: CompletionKind,
	/// How a function is called, like `log(x, b?)`, where `?` marks an optional parameter and `…` any count of them.
	pub signature: Option<String>,
	/// A builtin's hover text, as it shows where the builtin is written, whose label is its name in words, like `Sine` for `sin`.
	pub tooltip: Option<Tooltip>,
}

/// The names that finish the one being typed, and the span of it, with any `\` prefix, that an accepted name replaces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completions<'src> {
	pub start: usize,
	pub end: usize,
	/// By kind in the variants' order, so a definition comes before any builtin it shadows, each alphabetical but for the constants,
	/// whose symbols lead their related words.
	pub offers: Vec<Offer<'src>>,
	/// The index of the offer fitting what's typed best, for an editor to highlight.
	pub best: usize,
}

/// How closely a name fits what's typed, where the closest fit is the one an editor highlights.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Fit {
	Whole,
	Prefix,
	/// The typed letters appear in the name in order, like `sn` in `sin`.
	Scattered,
}

/// How a name fits what's typed, or `None` unless the typed letters appear in it in order starting with its first.
fn fit(typed: &str, name: &str) -> Option<Fit> {
	if name == typed {
		return Some(Fit::Whole);
	}
	if name.starts_with(typed) {
		return Some(Fit::Prefix);
	}

	let (mut letters, mut typed_letters) = (name.chars(), typed.chars());
	let first = typed_letters.next()?;
	(letters.next() == Some(first) && typed_letters.all(|typed_letter| letters.any(|letter| letter == typed_letter))).then_some(Fit::Scattered)
}

/// A name that fits what's typed, before it's ranked among the offers.
struct Candidate<'src> {
	name: &'src str,
	/// A function's parameters as its signature lists them, like `x, b?`.
	parameters: Option<String>,
	tooltip: Option<Tooltip>,
	fit: Fit,
	/// Whether it's written with the `\` prefix.
	prefixed: bool,
}

/// The names holding the letters of the one the caret stands in or at either end of, in order from their first, or every name where
/// none is there yet, or the typeset alias of an ASCII operator just before the caret. Unless they're `asked` for, as by a shortcut
/// rather than by typing, names are offered only once part of one is typed before the caret. A name typed with the `\` prefix only
/// ever reaches a builtin, so only builtins are offered for it.
pub fn completions(source: &str, caret: usize, asked: bool) -> Completions<'_> {
	// The name the caret stands in or at either end of, if any, or else an operator ending at it that has an alias
	let mut lexer = Lexer::new(source);
	let mut name = None;
	let mut operator = None;
	let mut after_operand = false;
	while let Some((range, token)) = lexer.next_spanned() {
		if range.start > caret {
			break;
		}
		if range.end >= caret && matches!(token, Token::Ident(_) | Token::If | Token::Otherwise | Token::Where) {
			name = Some(range.clone());
		}
		if range.end == caret && source.get(range.clone()).is_some_and(|spelling| spelling.is_ascii()) {
			operator = typeset_alias(&token, after_operand).map(|(alias, operator)| (range.start, alias, operator));
		}
		after_operand = operand_ended(&token, after_operand);
	}
	if let Some((start, alias, operator)) = operator {
		let offer = Offer {
			name: Cow::Borrowed(alias),
			kind: CompletionKind::Operator,
			signature: None,
			tooltip: Some(operator.tooltip()),
		};
		return Completions {
			start,
			end: caret,
			offers: vec![offer],
			best: 0,
		};
	}

	// The whole name is replaced, `\` and all (even a `\` typed alone), while its letters after any `\` are what the names fit
	let (start, end) = match name {
		Some(ref name) => (name.start, name.end),
		None if source.get(..caret).is_some_and(|before| before.ends_with('\\')) => (caret - 1, caret),
		None => (caret, caret),
	};
	let written = source.get(start..end).unwrap_or_default();
	let typed = written.strip_prefix('\\').unwrap_or(written);
	let builtins_only = typed.len() < written.len();
	if !asked && caret <= end - typed.len() {
		return Completions {
			start,
			end,
			offers: Vec::new(),
			best: 0,
		};
	}

	let (defined, defining) = match name {
		_ if builtins_only => (Vec::new(), false),
		Some(_) => definitions_around(source, start, false),
		None => definitions_around(source, caret, true),
	};
	// A name being defined is a new one, so typing it offers nothing
	if defining && !asked {
		return Completions {
			start,
			end,
			offers: Vec::new(),
			best: 0,
		};
	}
	// The `\` prefix reaches a builtin past a definition of its spelling and sort, or keeps the prefix already typed
	let prefixed = |builtin: &str, function: bool| builtins_only || defined.iter().any(|(local, parameters)| *local == builtin && parameters.is_some() == function);

	let locals = defined.iter().filter_map(|(local, parameters)| {
		Some(Candidate {
			name: local,
			parameters: parameters.clone(),
			tooltip: None,
			fit: fit(typed, local)?,
			prefixed: false,
		})
	});
	let mut locals = locals.collect::<Vec<_>>();

	// An alias offers the name it's another spelling of
	let aliased = BUILTIN_FUNCTION_ALIASES.into_iter().filter_map(|(alias, name)| Some((name, fit(typed, alias)?)));
	let named = BUILTIN_FUNCTIONS.iter().filter_map(|function| Some((function.name, fit(typed, function.name)?)));
	let functions = named.chain(aliased).filter_map(|(name, closeness)| {
		let function = BUILTIN_FUNCTIONS.iter().find(|function| function.name == name)?;
		Some(Candidate {
			name: function.name,
			parameters: Some(function.parameter_list()),
			tooltip: function_tooltip(function.name),
			fit: closeness,
			prefixed: prefixed(function.name, true),
		})
	});
	let suffixed = SUFFIXED_FORMS.into_iter().filter_map(|form| {
		let builtin = form.strip_suffix('_')?;
		let function = BUILTIN_FUNCTIONS.iter().find(|function| function.name == builtin)?;

		// A suffixed form counts as whole wherever the function it's a form of is typed whole, so it keeps its place ahead of it
		let closeness = if builtin == typed { Fit::Whole } else { fit(typed, form)? };
		// Its suffix sets the last parameter, so it's called without that one
		let parameters = function
			.parameters
			.split_last()
			.map(|(_, unsuffixed)| unsuffixed.iter().map(|parameter| parameter.name).collect::<Vec<_>>().join(", "));
		Some(Candidate {
			name: form,
			parameters,
			tooltip: function_tooltip(form),
			fit: closeness,
			prefixed: prefixed(form, true),
		})
	});
	let mut functions = functions.chain(suffixed).collect::<Vec<_>>();

	// A constant's words are gathered together, in the order that keeps related constants together, so its symbol can lead them
	let mut spellings: Vec<(Constant, Vec<(&str, Fit)>)> = Vec::new();
	let fitting = CONSTANT_SPELLINGS.into_iter().filter(|(spelling, _)| spelling.is_ascii());
	for (spelling, constant, closeness) in fitting.filter_map(|(spelling, constant)| Some((spelling, constant, fit(typed, spelling)?))) {
		match spellings.iter_mut().find(|(gathered, _)| *gathered == constant) {
			Some((_, words)) => words.push((spelling, closeness)),
			None => spellings.push((constant, vec![(spelling, closeness)])),
		}
	}
	// The identity matrix leads the basis vectors it's made of
	let identity = fit(typed, "I").map(|closeness| Candidate {
		name: "I",
		parameters: None,
		tooltip: Some(identity_tooltip()),
		fit: closeness,
		prefixed: prefixed("I", false),
	});
	let constants = spellings.into_iter().flat_map(|(constant, words)| {
		let tooltip = Some(constant_tooltip(constant));
		let best = words.iter().map(|(_, closeness)| *closeness).min().unwrap_or(Fit::Scattered);

		// A symbol is never prefixed, so it takes the place of any `\` typed before the words it stands for
		let symbol = constant.symbol().map(|symbol| Candidate {
			name: symbol,
			parameters: None,
			tooltip: tooltip.clone(),
			fit: best,
			prefixed: false,
		});
		let words = words.into_iter().map(move |(word, closeness)| Candidate {
			name: word,
			parameters: None,
			tooltip: tooltip.clone(),
			fit: closeness,
			prefixed: prefixed(word, false),
		});
		symbol.into_iter().chain(words)
	});
	let constants = identity.into_iter().chain(constants).collect::<Vec<_>>();

	// A keyword is offered only where the grammar takes one, which is asked only once one is typed toward
	let keywords = KEYWORDS.into_iter().filter(|_| !builtins_only).filter_map(|(keyword, token)| {
		Some(Candidate {
			name: keyword,
			parameters: None,
			tooltip: keyword_tooltip(&token),
			fit: fit(typed, keyword)?,
			prefixed: false,
		})
	});
	let mut keywords = keywords.collect::<Vec<_>>();
	if !keywords.is_empty() {
		let accepted = keywords_accepted_after(source.get(..start).unwrap_or_default());
		keywords.retain(|keyword| accepted.contains(&keyword.name));
	}

	locals.sort_unstable_by_key(|local| (local.name, local.parameters.is_some()));
	// A suffixed form like `log_` leads the function it's a form of, and a name reached two ways keeps its closer fit
	functions.sort_unstable_by_key(|function| (function.name.strip_suffix('_').unwrap_or(function.name), !function.name.ends_with('_'), function.fit));
	keywords.sort_unstable_by_key(|keyword| keyword.name);

	let mut offers: Vec<(Offer, (Fit, usize, usize))> = Vec::new();
	for (group_index, (group, kind)) in [
		(locals, CompletionKind::Local),
		(functions, CompletionKind::Function),
		(constants, CompletionKind::Constant),
		(keywords, CompletionKind::Keyword),
	]
	.into_iter()
	.enumerate()
	{
		for Candidate {
			name,
			parameters,
			tooltip,
			fit: closeness,
			prefixed: with_prefix,
		} in group
		{
			// A name reached two ways is offered once, though a value and a function, or a definition and a builtin, may share a spelling
			let name = if with_prefix { Cow::Owned(format!("\\{name}")) } else { Cow::Borrowed(name) };
			let signature = parameters.map(|parameters| format!("{name}({parameters})"));
			if offers
				.iter()
				.any(|(offer, _)| offer.kind == kind && offer.name == name && offer.signature.is_some() == signature.is_some())
			{
				continue;
			}

			// A suffixed form like `log_` takes the place of the function it's a form of
			let bare = name.trim_start_matches('\\').trim_end_matches('_');
			let catalog_index = || BUILTIN_FUNCTIONS.iter().position(|function| function.name == bare);
			let catalog_rank = if kind == CompletionKind::Function { catalog_index().unwrap_or(usize::MAX) } else { 0 };

			offers.push((Offer { name, kind, signature, tooltip }, (closeness, group_index, catalog_rank)));
		}
	}

	// With nothing typed, every name fits alike, so the first is best. Otherwise it's the closest fit, then the earliest kind, then for
	// functions the catalog's order (so `arcs` favors `asin`), then the first offer, which puts a symbol ahead of the words it stands for.
	let best = if typed.is_empty() {
		0
	} else {
		offers.iter().enumerate().min_by_key(|(_, (_, rank))| *rank).map_or(0, |(index, _)| index)
	};

	Completions {
		start,
		end,
		offers: offers.into_iter().map(|(offer, _)| offer).collect(),
		best,
	}
}

/// The names the `where` clauses and parameters around the name at `offset` define, each function's with its parameter list, and
/// whether that name is itself being defined, found only where the whole expression parses. With no name there, a `placeholder`
/// one put in its place finds the definitions it would see.
fn definitions_around(source: &str, offset: usize, placeholder: bool) -> (Vec<(&str, Option<String>)>, bool) {
	const PLACEHOLDER: &str = "x";

	let probed_source = match placeholder {
		true => match (source.get(..offset), source.get(offset..)) {
			(Some(before), Some(after)) => Cow::Owned(format!("{before}{PLACEHOLDER}{after}")),
			_ => return (Vec::new(), false),
		},
		false => Cow::Borrowed(source),
	};
	let Some(syntax) = parse_quietly(&probed_source) else { return (Vec::new(), false) };
	let probe = names_in_scope(syntax, &probed_source, offset);

	// Each name is read back from the source, past where the placeholder stood
	let names = probe.names.into_iter().filter_map(|(name, parameters)| {
		let found = offset_in(&probed_source, name)?;
		let at = if placeholder && found > offset { found - PLACEHOLDER.len() } else { found };
		Some((source.get(at..at + name.len())?, parameters.map(|parameters| parameters.join(", "))))
	});
	(names.collect(), probe.defining)
}

/// The typeset alias an editor offers for an operator typed in ASCII where it stands as that operator: a comparison or logical one
/// after an operand, and `!` as a not before one rather than a factorial after one.
fn typeset_alias(token: &Token, after_operand: bool) -> Option<(&'static str, Operator)> {
	Some(match token {
		Token::Neq if after_operand => ("≠", Operator::NotEqual),
		Token::Le if after_operand => ("≤", Operator::LessOrEqual),
		Token::Ge if after_operand => ("≥", Operator::GreaterOrEqual),
		Token::AndAnd if after_operand => ("∧", Operator::And),
		Token::OrOr if after_operand => ("∨", Operator::Or),
		Token::Bang if !after_operand => ("¬", Operator::Not),
		_ => return None,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	/// The part an accepted name replaces and the names offered for the source with its caret at the `‸`.
	fn offered(marked: &str) -> (String, Vec<String>) {
		let caret = marked.find('‸').unwrap();
		let source = marked.replacen('‸', "", 1);
		let Completions { start, end, offers, .. } = completions(&source, caret, true);
		(source[start..end].to_string(), offers.into_iter().map(|offer| offer.name.to_string()).collect())
	}

	fn names(marked: &str) -> Vec<String> {
		offered(marked).1
	}

	/// The offer an editor highlights for the source with its caret at the `‸`.
	fn highlighted(marked: &str) -> String {
		let caret = marked.find('‸').unwrap();
		let source = marked.replacen('‸', "", 1);
		let Completions { offers, best, .. } = completions(&source, caret, true);
		offers[best].name.to_string()
	}

	/// Each offer as a menu shows it, a function by its signature.
	fn shown(marked: &str) -> Vec<String> {
		let caret = marked.find('‸').unwrap();
		let source = marked.replacen('‸', "", 1);
		let offers = completions(&source, caret, true).offers.into_iter();
		offers.map(|offer| offer.signature.unwrap_or_else(|| offer.name.to_string())).collect()
	}

	#[test]
	fn a_builtin_is_offered_with_its_tooltip() {
		let tooltips = |marked: &str| {
			let caret = marked.find('‸').unwrap();
			let source = marked.replacen('‸', "", 1);
			completions(&source, caret, true).offers.into_iter().map(|offer| offer.tooltip).collect::<Vec<_>>()
		};
		let titled = |marked: &str| {
			let caret = marked.find('‸').unwrap();
			let source = marked.replacen('‸', "", 1);
			let offers = completions(&source, caret, true).offers.into_iter();
			offers.map(|offer| (offer.name.to_string(), offer.tooltip.map(|tooltip| tooltip.label))).collect::<Vec<_>>()
		};
		let pairs = |pairs: &[(&str, Option<&str>)]| pairs.iter().map(|(name, title)| (name.to_string(), title.map(str::to_string))).collect::<Vec<_>>();

		// Each is what hovering the builtin shows in the expression, a suffixed form shown as called without the parameter its suffix sets
		assert_eq!(tooltips("lo‸"), [function_tooltip("log_"), function_tooltip("log")]);
		assert_eq!(tooltips("1 >=‸"), [Some(Operator::GreaterOrEqual.tooltip())]);

		assert_eq!(titled("si‸"), pairs(&[("sign", Some("Sign")), ("sin", Some("Sine")), ("sinh", Some("Hyperbolic Sine"))]));
		assert_eq!(titled("lo‸"), pairs(&[("log_", Some("Logarithm")), ("log", Some("Logarithm"))]));
		assert_eq!(
			titled("pi‸"),
			pairs(&[
				("pick", Some("Permutations")),
				("π", Some("Pi")),
				("pi", Some("Pi")),
				("φ", Some("Golden Ratio")),
				("phi", Some("Golden Ratio"))
			])
		);
		assert_eq!(titled("1 <=‸"), pairs(&[("≤", Some("Less Than or Equal"))]));
		assert_eq!(titled("x wh‸"), pairs(&[("within", Some("Within")), ("where", Some("Where Clause"))]));

		// A definition is the user's own, so it has no tooltip
		assert_eq!(titled("f(2) + fo‸ where foo = 1, f(t) = t"), pairs(&[("foo", None), ("floor", Some("Floor"))]));
	}

	#[test]
	fn a_typed_name_is_extended_by_the_builtins() {
		assert_eq!(offered("si‸"), ("si".to_string(), vec!["sign".to_string(), "sin".to_string(), "sinh".to_string()]));
		assert_eq!(names("2 sq‸"), ["sqrt"]);
		assert_eq!(names("sin‸"), ["sign", "sin", "sinh"], "a whole name stays offered");
		assert_eq!(offered("sq‸rt(2)"), ("sqrt".to_string(), vec!["sqrt".to_string()]), "the whole name around the caret is typed");

		// Functions come before constants, then keywords, however closely each fits
		assert_eq!(names("ta‸"), ["tan", "tanh", "translation", "τ", "tau"]);
		assert_eq!(names("pi‸"), ["pick", "π", "pi", "φ", "phi"]);
		assert_eq!(names("{1 if x, 2 oth‸"), ["otherwise"]);
		assert_eq!(names("x wh‸"), ["within", "where"]);
	}

	#[test]
	fn the_closest_fit_is_highlighted_where_it_stands() {
		// A whole name, then one beginning as typed, then one only holding its letters, with ties going by the catalog or else to the first
		assert_eq!(highlighted("sin‸"), "sin");
		assert_eq!(highlighted("pi‸"), "π");
		assert_eq!(highlighted("x wh‸"), "where");
		assert_eq!(highlighted("sn‸"), "snap");
		assert_eq!(highlighted("ta‸"), "tan");
		assert_eq!(highlighted("arcs‸"), "asin");
		assert_eq!(highlighted("log‸"), "log_", "a suffixed form counts as whole where its function is typed whole");
		assert_eq!(highlighted("sig‸ where sigma = 1"), "sigma");

		// A name is offered whole wherever the caret stands in it or at either end
		let expression = "[1; i] 8 sqrt(x) e^(x i tau / phi^2)";
		let phi = expression.find("phi").unwrap();
		for caret in phi..=phi + 3 {
			let marked = format!("{}‸{}", &expression[..caret], &expression[caret..]);
			assert_eq!(offered(&marked), ("phi".to_string(), vec!["φ".to_string(), "phi".to_string()]), "{marked}");
			assert_eq!(highlighted(&marked), "φ", "{marked}");
		}
	}

	#[test]
	fn typed_letters_find_a_name_holding_them_in_order() {
		// The letters appear in order from the name's first, so `sn` finds `sin` but not `asin`
		assert_eq!(names("sn‸"), ["sign", "sin", "sinh", "snap"]);
		assert_eq!(names("sqt‸"), ["sqrt"]);
		assert_eq!(names("x whr‸"), ["where"]);
		assert!(names("ns‸").is_empty());
	}

	#[test]
	fn functions_are_shown_by_their_signatures() {
		assert_eq!(shown("sin‸"), ["sign(x)", "sin(θ)", "sinh(x)"]);
		assert_eq!(shown("lo‸"), ["log_(x)", "log(x, b?)"], "`log2` is reached as a suffixed `log`");
		assert_eq!(shown("log‸"), ["log_(x)", "log(x, b?)"], "a suffixed form stays ahead of its function typed whole");
		assert_eq!(shown("roo‸"), ["root_(x)", "root(x, n)", "rotation(θ, axis?)", "rotor(θ, axis?)"]);

		assert_eq!(shown("mi‸"), ["lerp(a, b, t)", "matrix(q)", "median(…)", "min(…)"]);
		assert_eq!(shown("rot‸"), ["root_(x)", "root(x, n)", "rotate(v, θ, axis?)", "rotation(θ, axis?)", "rotor(θ, axis?)"]);
		assert_eq!(shown("ea‸(2) where ease(t, k) = t^k, easing = 1"), ["ease(t, k)", "easing"]);
		assert_eq!(shown("tr‸"), ["translation(A)", "trunc(x)", "true"]);
	}

	#[test]
	fn another_spelling_offers_the_builtin_it_names() {
		assert_eq!(names("arcs‸"), ["acos", "acosh", "acsc", "acsch", "asec", "asech", "asin", "asinh"]);
		assert_eq!(names("arcsinh‸"), ["asinh"]);
		assert_eq!(names("arsinh‸"), ["asinh"]);
		assert_eq!(names("av‸"), ["mean"]);
		assert_eq!(names("comb‸"), ["choose"]);
		assert_eq!(names("permutation‸"), ["pick"]);
		assert_eq!(highlighted("mix‸"), "lerp", "a whole alias fits closer than `matrix` holding its letters");
	}

	#[test]
	fn the_prefix_reaches_only_builtins() {
		let offered_names = |names: &[&str]| names.iter().map(|name| name.to_string()).collect::<Vec<_>>();
		assert_eq!(offered("\\si‸"), ("\\si".to_string(), offered_names(&["\\sign", "\\sin", "\\sinh"])));
		assert_eq!(names("\\wh‸"), ["\\within"], "the keyword `where` isn't a builtin");
		assert!(names("\\rad‸ where radius = 2").is_empty());

		// A symbol can't take the prefix, so it replaces the `\` along with the word it stands for
		assert_eq!(offered("2 \\inf‸"), ("\\inf".to_string(), offered_names(&["∞", "\\inf", "\\infinity"])));
		assert_eq!(highlighted("2 \\inf‸"), "∞");

		// The name is the whole of what's replaced, even with the caret before its `\`
		assert_eq!(offered("‸\\sqrt"), ("\\sqrt".to_string(), offered_names(&["\\sqrt"])));

		// A `\` typed alone is replaced too, so a symbol chosen for it stands without one
		let (replaced, alone) = offered("2 \\‸");
		assert_eq!(replaced, "\\");
		assert!(["∞", "\\inf", "\\sin", "\\I"].iter().all(|name| alone.iter().any(|offered| offered == name)), "{alone:?}");
		assert!(!alone.iter().any(|name| name == "where" || name == "\\∞"), "{alone:?}");
	}

	#[test]
	fn the_identity_matrix_is_offered() {
		assert_eq!(names("I‸"), ["I"]);
		assert_eq!(names("\\I‸"), ["\\I"]);
		assert!(!names("i‸").contains(&"I".to_string()), "names are matched by case");
	}

	#[test]
	fn definitions_are_offered_where_they_can_be_read() {
		assert_eq!(names("2 rad‸ where radius = 2"), ["radius"]);
		assert_eq!(names("ea‸(2) where ease(t) = t^2"), ["ease"]);
		assert_eq!(names("sig‸ where sigma = 1"), ["sigma", "sign"]);

		// A parameter is read only in its function's body, and a clause's names only within its parentheses, which the called name is outside of
		assert_eq!(names("f(1) where f(radius) = 2 rad‸"), ["radius"]);
		assert!(names("(f(1) where f(radius) = 2 radius) + rad‸").is_empty());
		assert!(names("(2 where radius = 1) + rad‸").is_empty());
		assert!(!names("fo‸(2 where foo = 1)").contains(&"foo".to_string()));

		// The definition being written can't read itself, while its clause's others can be read
		let locals = |marked: &str| {
			let caret = marked.find('‸').unwrap();
			let source = marked.replacen('‸', "", 1);
			let offers = completions(&source, caret, true).offers.into_iter();
			offers
				.filter(|offer| offer.kind == CompletionKind::Local)
				.map(|offer| offer.signature.unwrap_or_else(|| offer.name.to_string()))
				.collect::<Vec<_>>()
		};
		assert_eq!(locals("f(1) where y = ‸, z = 2"), ["z"]);
		assert_eq!(locals("f(1) where f(t) = ‸"), ["t"]);
		assert_eq!(locals("2 where rad‸ = 1"), Vec::<String>::new());

		// Where a definition's name or parameter is yet to be typed, it's no name to offer
		assert_eq!(locals("2 where ‸= 1"), Vec::<String>::new());
		assert_eq!(locals("f(1) where f(‸) = 2"), Vec::<String>::new());
	}

	#[test]
	fn a_shadowed_builtin_is_offered_with_the_prefix_that_reaches_it() {
		// A function a clause defines shadows the builtin function of its spelling
		assert_eq!(shown("si‸(0) where sin(n) = 3n"), ["sin(n)", "sign(x)", "\\sin(θ)", "sinh(x)"]);

		// A value doesn't shadow a function, so the builtin is still reached by its own spelling
		assert_eq!(shown("si‸(0) where sin = 3"), ["sin", "sign(x)", "sin(θ)", "sinh(x)"]);

		// A value shadows the constant of its spelling, while the constant's symbol still reaches it
		let constants = |marked: &str| {
			let caret = marked.find('‸').unwrap();
			let source = marked.replacen('‸', "", 1);
			let offers = completions(&source, caret, true).offers.into_iter();
			offers.filter(|offer| offer.kind == CompletionKind::Constant).map(|offer| offer.name.to_string()).collect::<Vec<_>>()
		};
		assert_eq!(constants("2 e‸ where e = 3"), ["\\e"]);
		assert_eq!(constants("p‸ where pi = 3"), ["π", "\\pi", "φ", "phi"]);
		assert_eq!(constants("i‸ + j where i = 2, j = 3"), ["\\i", "∞", "inf", "infinity"]);
		assert_eq!(constants("2 e‸ where e(t) = t"), ["e"], "a function doesn't shadow a value");

		// A value and a function of one name are both offered, and the function still shadows the builtin
		assert_eq!(shown("si‸(0) where sin = 1, sin(t) = t"), ["sin", "sin(t)", "sign(x)", "\\sin(θ)", "sinh(x)"]);
	}

	#[test]
	fn typing_offers_names_once_part_of_one_is_typed() {
		let typed = |marked: &str| {
			let caret = marked.find('‸').unwrap();
			let source = marked.replacen('‸', "", 1);
			completions(&source, caret, false).offers.into_iter().map(|offer| offer.name.to_string()).collect::<Vec<_>>()
		};

		assert_eq!(typed("2 sq‸"), ["sqrt"]);
		assert_eq!(typed("x <=‸"), ["≤"]);
		assert!(typed("2 + ‸").is_empty());
		assert!(typed("2‸").is_empty());
		assert!(typed("\\‸").is_empty());
		assert!(typed("2 ‸sin").is_empty(), "nothing of the name is typed before the caret");

		// A name being defined is new, so typing it offers nothing, while reading one offers it
		assert!(typed("2 where rad‸ = 1").is_empty());
		assert!(typed("f(1) where f(ra‸) = 2").is_empty());
		assert_eq!(typed("rad‸ where radius = 1"), ["radius"]);
	}

	#[test]
	fn names_asked_for_with_nothing_typed_include_definitions_around_the_caret() {
		assert!(names("2 + ‸ where radius = 1").contains(&"radius".to_string()));
		assert!(names("f(‸) where f(t) = t, radius = 1").contains(&"radius".to_string()));
		assert!(!names("(2 where radius = 1) + ‸").contains(&"radius".to_string()), "the clause ends at its parenthesis");
	}

	#[test]
	fn nothing_typed_offers_every_name() {
		let (typed, names) = offered("2 + ‸");
		assert!(typed.is_empty());
		assert!(["sin", "pi"].iter().all(|name| names.iter().any(|offered| offered == name)), "{names:?}");

		// Every name fits alike, so the first is highlighted, whatever its place in the catalog
		assert_eq!(highlighted("2 + ‸"), names[0]);
		assert_eq!(highlighted("‸"), names[0]);

		// A constant's symbol leads its words, and related constants stay together
		let position = |name: &str| names.iter().position(|offered| offered == name).unwrap();
		assert_eq!(position("π") + 1, position("pi"));
		assert_eq!(position("∞") + 1, position("inf"));
		assert_eq!(position("true") + 1, position("false"));
		assert_eq!([position("i") + 1, position("j") + 1], [position("j"), position("k")]);
		assert_eq!(position("phi") + 1, position("e"));
	}

	#[test]
	fn keywords_are_offered_only_where_the_grammar_takes_them() {
		let keywords = |marked: &str| names(marked).into_iter().filter(|name| ["if", "otherwise", "where"].contains(&name.as_str())).collect::<Vec<_>>();

		// A clause follows a value at the top level or directly inside parentheses, and a clause's definition can't have its own
		assert_eq!(keywords("2 ‸"), ["where"]);
		assert_eq!(keywords("max(a ‸"), ["where"]);
		assert!(keywords("2 + ‸").is_empty());
		assert!(keywords("‸").is_empty());
		assert!(keywords("[a ‸").is_empty());
		assert!(keywords("a where b = 1 ‸").is_empty());

		// A case's value is followed by its condition or `otherwise`, and a case has one or the other
		assert_eq!(keywords("{a ‸"), ["if", "otherwise"]);
		assert_eq!(keywords("{a if c, b i‸"), ["if"]);
		assert!(keywords("{a if c ‸").is_empty());
	}

	#[test]
	fn a_constant_offers_its_symbol_first() {
		assert_eq!(names("inf‸"), ["∞", "inf", "infinity"]);
		assert_eq!(names("infi‸"), ["∞", "infinity"]);
		assert_eq!(names("2 ph‸"), ["φ", "phi"]);
		assert_eq!(names("tau‸"), ["τ", "tau"]);
	}

	#[test]
	fn an_operator_typed_in_ascii_offers_its_typeset_alias() {
		assert_eq!(offered("x !=‸"), ("!=".to_string(), vec!["≠".to_string()]));
		assert_eq!(names("x <=‸"), ["≤"]);
		assert_eq!(names("x >=‸"), ["≥"]);
		assert_eq!(names("a &&‸"), ["∧"]);
		assert_eq!(names("a ||‸"), ["∨"]);

		// A `!` is a not only where no operand ends just before it, and is otherwise the factorial
		let offers = |marked: &str, alias: &str| names(marked).iter().any(|name| name == alias);
		assert_eq!(names("!‸"), ["¬"]);
		assert_eq!(names("{1 if !‸"), ["¬"]);
		assert!(!offers("5!‸", "¬"));
		assert!(!offers("(2 + 3)!‸", "¬"));

		// A `||` where an operand is expected opens two magnitudes rather than standing for an or
		assert!(!offers("x + ||‸", "∨"));
		assert!(!offers("||‸", "∨"));

		// An operator already typeset, or no longer just before the caret, has no alias to offer
		assert!(!offers("x ≠‸", "≠"));
		assert!(!offers("x != ‸", "≠"));
	}

	#[test]
	fn a_typeset_alias_reads_as_its_ascii_operator() {
		for (ascii, typeset) in [("1 != 2", "1 ≠ 2"), ("1 <= 2", "1 ≤ 2"), ("2 >= 1", "2 ≥ 1"), ("1 && 0", "1 ∧ 0"), ("0 || 1", "0 ∨ 1"), ("!0", "¬0")] {
			assert_eq!(crate::evaluate(ascii).unwrap().unwrap(), crate::evaluate(typeset).unwrap().unwrap(), "`{typeset}`");
		}
	}
}
