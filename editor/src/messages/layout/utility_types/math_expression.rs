//! What the math parser reads of an expression, converted for a math expression widget into the UTF-16 code units its frontend's strings use.

use crate::messages::layout::utility_types::layout_widget::hash_menu_list_entry_sections;
use crate::messages::layout::utility_types::widget_prelude::*;
use math_parser::parser::{ErrorMessage, ParseError};

/// The source's UTF-16 length up to each byte, so the parser's byte offsets convert to the frontend's code units in one lookup.
fn utf16_offsets(source: &str) -> Vec<u32> {
	let mut units = vec![0_u32; source.len() + 1];
	let mut count = 0;
	for (byte, character) in source.char_indices() {
		units[byte..byte + character.len_utf8()].fill(count);
		count += character.len_utf16() as u32;
	}
	units[source.len()] = count;
	units
}

/// Why a math expression fails to parse, or `None` when it parses. Where the node applies a lone operator or function name like
/// `+` or `min` across its items, that counts as valid too.
pub fn math_expression_error(expression: &str, accepts_reducers: bool) -> Option<ParseError> {
	// A blank expression is unfinished rather than wrong, so it goes unflagged like any empty field
	if expression.trim().is_empty() || (accepts_reducers && math_parser::reducer::classify_reducer(expression, math_parser::context::NothingMap).is_some()) {
		return None;
	}
	math_parser::ast::Node::try_parse_from_str(expression).err()
}

/// The stretches of a math expression its parse error points at, for the frontend to underline. An empty span, where something is
/// missing like at the end of the input, has nothing to underline, so it widens to the character before it, past any whitespace.
pub fn math_expression_error_ranges(source: &str, error: Option<&ParseError>) -> Vec<MathExpressionRange> {
	let units = utf16_offsets(source);

	let spans = error.into_iter().flat_map(|error| error.messages().iter().filter_map(ErrorMessage::span));
	spans
		.filter_map(|span| {
			let before = source.get(..span.start)?.trim_end();
			let span = match before.chars().next_back() {
				Some(character) if span.is_empty() => before.len() - character.len_utf8()..before.len(),
				_ => span,
			};
			Some(MathExpressionRange {
				start: *units.get(span.start)?,
				end: *units.get(span.end)?,
			})
		})
		.collect()
}

/// The names offered to finish the one being typed in a math expression, in sections by kind, each entry's value being the text
/// that replaces the stretch of code units from `start` to `end`, the name the caret is in or beside. A call's `()` takes the caret
/// between them, except that a suffixed form's `_`, like `log_`'s, is selected instead, for its suffix to be typed over or after it.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(large_number_types_as_bigints))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MathExpressionCompletions {
	pub start: u32,
	pub end: u32,
	pub entries: MenuListEntrySections,
	#[serde(rename = "entriesHash")]
	pub entries_hash: u64,
	/// The index among all the sections' entries of the one fitting what's typed best, which starts highlighted.
	pub best: u32,
}

/// The names offered to finish the one the caret, given in code units, stands in or beside. Unless they're `asked` for, as by a
/// shortcut rather than by typing, they're offered only once part of a name is typed before the caret.
pub fn math_expression_completions(source: &str, caret: u32, asked: bool) -> MathExpressionCompletions {
	let units = utf16_offsets(source);
	// Each character's bytes share the offset of its first, so the first byte reaching the caret begins the character after it
	let byte = units.partition_point(|&unit| unit < caret).min(source.len());
	let completions = math_parser::completion::completions(source, byte, asked);
	let best = completions.best as u32;

	// A function comes with the parentheses of its call, unless they're already there, spaced or not
	let parenthesized = source.get(completions.end..).unwrap_or_default().trim_start().starts_with('(');

	let mut entries: MenuListEntrySections = Vec::new();
	let mut previous_kind = None;
	for offer in completions.offers {
		if previous_kind != Some(offer.kind) {
			entries.push(Vec::new());
			previous_kind = Some(offer.kind);
		}
		if let Some(section) = entries.last_mut() {
			let insertion = if offer.signature.is_some() && !parenthesized {
				format!("{}()", offer.name)
			} else {
				offer.name.to_string()
			};
			let label = offer.signature.unwrap_or_else(|| offer.name.to_string());

			// A builtin is named at the row's end and hovers with the tooltip it has where it's written
			let entry = MenuListEntry::new(insertion).label(label);
			let entry = match offer.tooltip {
				Some(tooltip) => entry
					.annotation(tooltip.label.clone())
					.tooltip_label(tooltip.label)
					.tooltip_description(tooltip.description)
					.tooltip_code(tooltip.form.unwrap_or_default()),
				None => entry,
			};
			section.push(entry);
		}
	}

	MathExpressionCompletions {
		start: units[completions.start],
		end: units[completions.end],
		entries_hash: hash_menu_list_entry_sections(&entries),
		entries,
		best,
	}
}

/// The value of math typed in place of a number, written as a number literal, or `None` where it isn't a finite real number.
pub fn math_expression_number(source: &str) -> Option<String> {
	use math_parser::value::{Number, Value};

	let Value::Number(number) = math_parser::evaluate(source).ok()?.ok()?.into_value()?;
	if let Number::Integer(integer) = number {
		return Some(integer.to_string());
	}

	let real = graphene_std::math::float_noise::round_away_float_noise(number.as_real()?);
	if !real.is_finite() {
		return None;
	}

	// A magnitude far from 1 is written in scientific notation, which a literal can hold too
	let magnitude = real.abs();
	Some(if magnitude != 0. && !(1e-6..1e16).contains(&magnitude) {
		format!("{real:e}")
	} else {
		real.to_string()
	})
}

/// A math expression's tokens for the frontend to color and typeset, each naming its tooltip by its index among the tooltips, which
/// are listed once each however many tokens share one, like a matrix literal's brackets and separators.
pub struct MathExpressionTokens {
	pub tokens: Vec<MathExpressionToken>,
	pub tooltips: Vec<MathExpressionTooltip>,
}

/// The tokens of a math expression for the frontend to color and typeset, with the parser's byte offsets converted to code units.
pub fn math_expression_tokens(source: &str, accepts_reducers: bool) -> MathExpressionTokens {
	let units = utf16_offsets(source);
	let mut tooltips: Vec<MathExpressionTooltip> = Vec::new();

	let tokens = math_parser::highlight::highlight(source, accepts_reducers).into_iter().map(|token| {
		let tooltip = token.tooltip.map(|tooltip| MathExpressionTooltip {
			label: tooltip.label,
			form: tooltip.form,
			description: tooltip.description,
			signature: tooltip.signature.map(|signature| MathExpressionSignature {
				opening: signature.opening,
				parameters: signature.parameters.into_iter().map(str::to_string).collect(),
				separator: signature.separator.to_string(),
				closing: signature.closing.to_string(),
				range: signature.range.to_string(),
				pointers: signature
					.pointers
					.into_iter()
					.map(|pointer| MathExpressionPointer {
						line: pointer.line as u32,
						column: pointer.column as u32,
						arrow: pointer.arrow.to_string(),
					})
					.collect(),
				lines: signature.lines.into_iter().map(|line| line as u32).collect(),
			}),
		});
		let tooltip = tooltip.map(|tooltip| match tooltips.iter().position(|listed| *listed == tooltip) {
			Some(index) => index as u32,
			None => {
				tooltips.push(tooltip);
				(tooltips.len() - 1) as u32
			}
		});

		MathExpressionToken {
			start: units[token.start],
			end: units[token.end],
			role: token.role.into(),
			superscript: token.superscript,
			subscript: token.subscript.map(|offset| units[token.start + offset] - units[token.start]),
			partner: token.partner.map(|partner| partner as u32),
			separates: token.separates.map(|opener| opener as u32),
			tooltip,
			literal: token.literal.map(|literal| MathExpressionLiteral {
				start: units[literal.start],
				after_operand: literal.after_operand,
			}),
		}
	});
	let tokens = tokens.collect();

	MathExpressionTokens { tokens, tooltips }
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn math_typed_for_a_number_is_written_as_its_value() {
		assert_eq!(math_expression_number("1/4").as_deref(), Some("0.25"));
		assert_eq!(math_expression_number("0.1 + 0.2").as_deref(), Some("0.3"), "float noise is rounded away");
		assert_eq!(math_expression_number("2^60").as_deref(), Some("1152921504606846976"), "an integer stays exact");
		assert_eq!(math_expression_number("-pi").as_deref(), Some("-3.14159265359"), "rounding away noise keeps 12 significant digits");
		assert_eq!(math_expression_number("10^-9 / 4").as_deref(), Some("2.5e-10"), "a tiny magnitude is written in scientific notation");

		// Anything other than a finite real number has no number to write
		for source in ["1/0", "2i", "x + 1", "I", "2 +"] {
			assert_eq!(math_expression_number(source), None, "`{source}`");
		}
	}

	#[test]
	fn math_expression_completions_count_code_units_and_group_by_kind() {
		// `π` is one code unit and `𝑥` two, so the typed `ta` begins 6 units in, though 9 bytes in
		let completions = math_expression_completions("π𝑥 + ta", 8, false);
		assert_eq!((completions.start, completions.end), (6, 8));
		assert_eq!(
			math_expression_completions("x wh", 4, false).best,
			1,
			"`where` is highlighted after `within`, which only holds the letters"
		);

		// A function shows its signature while its call is what's inserted
		let sections = |completions: MathExpressionCompletions| {
			let section = |section: &Vec<MenuListEntry>| section.iter().map(|entry| (entry.value.clone(), entry.label.clone())).collect::<Vec<_>>();
			completions.entries.iter().map(section).collect::<Vec<_>>()
		};
		let pair = |value: &str, label: &str| (value.to_string(), label.to_string());
		assert_eq!(
			sections(completions),
			[
				vec![pair("tan()", "tan(θ)"), pair("tanh()", "tanh(x)"), pair("translation()", "translation(A)")],
				vec![pair("τ", "τ"), pair("tau", "tau")]
			]
		);

		// Parentheses already after the name, spaced or not, are the call's own
		assert_eq!(
			sections(math_expression_completions("ta (2)", 2, false)),
			[
				vec![pair("tan", "tan(θ)"), pair("tanh", "tanh(x)"), pair("translation", "translation(A)")],
				vec![pair("τ", "τ"), pair("tau", "tau")]
			]
		);

		// A suffixed form comes with its call's parentheses like any function
		let completions = math_expression_completions("log", 3, false);
		assert_eq!(sections(completions), [vec![pair("log_()", "log_(x)"), pair("log()", "log(x, b?)")]]);

		// An operator typed in ASCII offers its typeset alias in place of it
		let completions = math_expression_completions("!", 1, false);
		assert_eq!((completions.start, completions.end), (0, 1));
		assert_eq!(sections(completions), [vec![pair("¬", "¬")]]);

		// Typing where no name is begun offers nothing, unless the names are asked for
		assert!(math_expression_completions("2 + ", 4, false).entries.is_empty());
		assert!(!math_expression_completions("2 + ", 4, true).entries.is_empty());
	}
}
