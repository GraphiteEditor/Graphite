use crate::color::Color;
use crate::color_palette::ColorPalette;
use crate::layout_abstract_types::*;
use crate::layout_system::*;

pub struct AttributeParser {
	capture_attribute_declaration_parameter_regex: regex::Regex,
	capture_attribute_type_sequences_regex: regex::Regex,
	match_integer_regex: regex::Regex,
	match_decimal_regex: regex::Regex,
	split_by_string_templates_regex: regex::Regex,
	capture_color_name_in_palette_regex: regex::Regex,
}

impl AttributeParser {
	// Prebuild all the regex patterns
	pub fn new() -> Self {
		let capture_attribute_declaration_parameter_regex: regex::Regex = regex::Regex::new(
			// Parameter: ?: (?, ... | ...) = ?
			r"^\s*(\w*)\s*(:)\s*(\()\s*((?:(?:\w+)(?:\s*,\s*\w+)*)(?:\s*\|\s*(?:(?:\w+)(?:\s*,\s*\w+)*))*)\s*(\))\s*(=)\s*([\s\w'\[\]@%\-.,]+|`[^`]*`|\[\[.*\]\])\s*$",
		)
		.unwrap();

		let capture_attribute_type_sequences_regex: regex::Regex = regex::Regex::new(concat!(
			// Argument: {{?}}
			r#"^\s*(\{\{)\s*(\w*)\s*(\}\})\s*$|"#,
			// Layout: [[?]]
			r#"^\s*(\[\[)\s*(.*)\s*(\]\])\s*$|"#,
			// Integer: ?
			r#"^\s*(-?\d+)\s*$|"#,
			// Decimal: ?
			r#"^\s*(-?(?:(?:\d+\.\d*)|(?:\d*\.\d+)))\s*$|"#,
			// AbsolutePx: ?px
			r#"^\s*(-?(?:(?:\d+(?:\.\d*)?)|(?:\d*(?:\.\d+))))([Pp][Xx])\s*$|"#,
			// Percent: ?%
			r#"^\s*(-?(?:(?:\d+(?:\.\d*)?)|(?:\d*(?:\.\d+))))(%)\s*$|"#,
			// PercentRemainder: ?@
			r#"^\s*(-?(?:(?:\d+(?:\.\d*)?)|(?:\d*(?:\.\d+))))(@)\s*$|"#,
			// Inner: inner
			r#"^\s*([Ii][Nn][Nn][Ee][Rr])\s*$|"#,
			// Width: width
			r#"^\s*([Ww][Ii][Dd][Tt][Hh])\s*$|"#,
			// Height: height
			r#"^\s*([Hh][Ee][Ii][Gg][Hh][Tt])\s*$|"#,
			// TemplateString: `? ... {{?}} ...`
			r#"^\s*(`)(.*)(`)\s*$|"#,
			// Color: [?]
			r#"^\s*(\[)(.*)(\])\s*$|"#,
			// Bool: true/false
			r#"^\s*([Tt][Rr][Uu][Ee]|[Ff][Aa][Ll][Ss][Ee])\s*$|"#,
			// None: none
			r#"^\s*([Nn][Oo][Nn][Ee])\s*$"#,
		))
		.unwrap();

		let match_integer_regex = regex::Regex::new(r"^\s*(-?\d+)\s*$").unwrap();

		let match_decimal_regex = regex::Regex::new(r"^\s*(-?(?:(?:\d+\.\d*)|(?:\d*\.\d+)))\s*$").unwrap();

		let split_by_string_templates_regex = regex::Regex::new(r"\{\{|\}\}").unwrap();

		let capture_color_name_in_palette_regex = regex::Regex::new(r"\s*'(.*)'\s*").unwrap();

		Self {
			capture_attribute_declaration_parameter_regex,
			capture_attribute_type_sequences_regex,
			match_integer_regex,
			match_decimal_regex,
			split_by_string_templates_regex,
			capture_color_name_in_palette_regex,
		}
	}

	pub fn parse_attribute_argument_types(&self, input: &str) -> Vec<TypeValueOrArgument> {
		let attribute_types = input.split(",").map(|piece| piece.trim()).collect::<Vec<&str>>();
		let list = attribute_types
			.iter()
			.map(|attribute_type| self.parse_attribute_argument_type(attribute_type))
			.collect::<Vec<TypeValueOrArgument>>();
		list
	}

	pub fn parse_attribute_argument_type(&self, attribute_type: &str) -> TypeValueOrArgument {
		// Match with the regular expression
		let captures = self
			.capture_attribute_type_sequences_regex
			.captures(attribute_type)
			.map(|captures| captures.iter().skip(1).flat_map(|c| c).map(|c| c.as_str()).collect::<Vec<_>>());

		// Match against the captured values as a list of tokens
		let tokens = captures.as_ref().map(|c| c.as_slice());
		match tokens {
			// Argument: {{?}}
			Some(["{{", name, "}}"]) => TypeValueOrArgument::VariableArgument(String::from(*name)),
			// Layout: [[?]]
			Some(["[[", xml_syntax, "]]"]) => {
				// Remove any whitespace in order to test if any XML syntax is present
				let trimmed = xml_syntax.trim();

				// Build either an empty vector (for empty XML input) or a vector with the one parsed XML fragment
				let layout_entries = if trimmed.len() == 0 {
					vec![]
				}
				else {
					let unescaped = Self::unescape_xml(trimmed);
					let parsed = LayoutSystem::parse_xml_node(&self, &unescaped[..], false).unwrap();
					// Put the single parsed node in a vector (TODO: this should set any number of parsed nodes once `parse_xml_node` becomes `parse_xml_nodes`)
					vec![parsed]
				};

				// Return the `Layout` typed value with the empty vector or vector with the parsed XML fragment
				TypeValueOrArgument::TypeValue(TypeValue::Layout(layout_entries))
			},
			// Integer: ?
			Some([value]) if self.match_integer_regex.is_match(value) => {
				let integer = value
					.parse::<i64>()
					.expect(&format!("Invalid value `{}` specified in the attribute type `{}` when parsing XML layout", value, attribute_type)[..]);
				TypeValueOrArgument::TypeValue(TypeValue::Integer(integer))
			},
			// Decimal: ?
			Some([value]) if self.match_decimal_regex.is_match(value) => {
				let decimal = value
					.parse::<f64>()
					.expect(&format!("Invalid value `{}` specified in the attribute type `{}` when parsing XML layout", value, attribute_type)[..]);
				TypeValueOrArgument::TypeValue(TypeValue::Decimal(decimal))
			},
			// AbsolutePx: px
			Some([value, px]) if px.eq_ignore_ascii_case("px") => {
				let pixels = value
					.parse::<f64>()
					.expect(&format!("Invalid value `{}` specified in the attribute type`{}` when parsing XML layout", value, attribute_type)[..]);
				let dimension = Dimension::AbsolutePx(pixels);
				TypeValueOrArgument::TypeValue(TypeValue::Dimension(dimension))
			},
			// Percent: ?%
			Some([value, "%"]) => {
				let percent = value
					.parse::<f64>()
					.expect(&format!("Invalid value `{}` specified in the attribute type `{}` when parsing XML layout", value, attribute_type)[..]);
				let dimension = Dimension::Percent(percent);
				TypeValueOrArgument::TypeValue(TypeValue::Dimension(dimension))
			},
			// PercentRemainder: ?@
			Some([value, "@"]) => {
				let percent_remainder = value
					.parse::<f64>()
					.expect(&format!("Invalid value `{}` specified in the attribute type `{}` when parsing XML layout", value, attribute_type)[..]);
				let dimension = Dimension::PercentRemainder(percent_remainder);
				TypeValueOrArgument::TypeValue(TypeValue::Dimension(dimension))
			},
			// Inner: inner
			Some([inner]) if inner.eq_ignore_ascii_case("inner") => TypeValueOrArgument::TypeValue(TypeValue::Dimension(Dimension::Inner)),
			// Width: width
			Some([width]) if width.eq_ignore_ascii_case("width") => TypeValueOrArgument::TypeValue(TypeValue::Dimension(Dimension::Width)),
			// Height: height
			Some([height]) if height.eq_ignore_ascii_case("height") => TypeValueOrArgument::TypeValue(TypeValue::Dimension(Dimension::Height)),
			// TemplateString: `? ... {{?}} ...`
			Some(["`", string, "`"]) => {
				let mut segments = Vec::<TemplateStringSegment>::new();
				let mut is_template = false;

				// Alternate between string and handlebars, always starting wtih string even if empty, and push abstract tokens of non-empty ones to the TemplateString sequence
				for part in self.split_by_string_templates_regex.split(string) {
					// Push only non-empty template string segments (a String or Argument)
					if !part.is_empty() {
						// Based on whether we are alternating to a string or template, push the appropriate abstract token
						let segment = match is_template {
							false => TemplateStringSegment::String(String::from(part)),
							true => TemplateStringSegment::Argument(TypeValueOrArgument::VariableArgument(String::from(part))),
						};
						segments.push(segment);
					}

					// The next iteration will switch from a template to a string or vice versa
					is_template = !is_template;
				}

				TypeValueOrArgument::TypeValue(TypeValue::TemplateString(segments))
			},
			// Color: [?]
			Some(["[", color_name, "]"]) => {
				let color = match self.capture_color_name_in_palette_regex.captures(color_name) {
					Some(captures) => {
						let palette_color = captures
							.get(1)
							.expect(
								&format!(
									"Invalid palette color name `{}` specified in the attribute type `{}` when parsing XML layout",
									color_name, attribute_type
								)[..],
							)
							.as_str();
						ColorPalette::lookup_palette_color(palette_color).into_color_srgb()
					},
					None => {
						let parsed = color_name.parse::<css_color_parser::Color>();
						let css_color = parsed.expect(
							&format!(
								"Invalid CSS color name `{}` specified in the attribute type `{}` when parsing XML layout",
								color_name, attribute_type
							)[..],
						);
						Color::new(
							css_color.r as f32 / 255.0,
							css_color.g as f32 / 255.0,
							css_color.b as f32 / 255.0,
							css_color.a as f32 / 255.0,
						)
					},
				};

				TypeValueOrArgument::TypeValue(TypeValue::Color(color))
			},
			// Bool: true/false
			Some([true_or_false]) if true_or_false.eq_ignore_ascii_case("true") || true_or_false.eq_ignore_ascii_case("false") => {
				let boolean = true_or_false.eq_ignore_ascii_case("true");
				TypeValueOrArgument::TypeValue(TypeValue::Bool(boolean))
			},
			// None: none
			Some([none]) if none.eq_ignore_ascii_case("none") => TypeValueOrArgument::TypeValue(TypeValue::None),
			// Unrecognized type pattern
			_ => panic!("Invalid attribute type `{}` when parsing XML layout", attribute_type),
		}
	}

	pub fn parse_attribute_parameter_declaration(&self, attribute_declaration: &str) -> VariableParameter {
		// Match with the regular expression
		let captures = self
			.capture_attribute_declaration_parameter_regex
			.captures(attribute_declaration)
			.map(|captures| captures.iter().skip(1).flat_map(|c| c).map(|c| c.as_str()).collect::<Vec<_>>());

		// Match against the captured values as a list of tokens
		let tokens = captures.as_ref().map(|c| c.as_slice());
		match tokens {
			// Parameter: ?: (?, ... | ...) = ?
			Some([name, ":", "(", raw_types_list, ")", "=", default_value]) => {
				// Variable name bound in the parameter
				let name = String::from(*name);

				// Split the type sequences up into a list of options separated by vertical bars
				let type_sequence_options = String::from(*raw_types_list)
					.split("|")
					.map(|group| {
						// Split each type sequence into individual types separated by commas
						group
							.split(",")
							.map(|individual_type| {
								// Remove any whitespace around the type
								let individual_type = individual_type.trim();

								// Return the case-insensitive TypeName enum for the individual type
								match &individual_type.to_ascii_lowercase()[..] {
									"layout" => TypeName::Layout,
									"integer" => TypeName::Integer,
									"decimal" => TypeName::Decimal,
									"absolutepx" => TypeName::AbsolutePx,
									"percent" => TypeName::Percent,
									"percentremainder" => TypeName::PercentRemainder,
									"inner" => TypeName::Inner,
									"width" => TypeName::Width,
									"height" => TypeName::Height,
									"templatestring" => TypeName::TemplateString,
									"color" => TypeName::Color,
									"bool" => TypeName::Bool,
									"none" => TypeName::None,
									_ => panic!(
										"Invalid type `{}` specified in the attribute type `{}` when parsing XML layout",
										individual_type, attribute_declaration
									),
								}
							})
							.collect::<Vec<TypeName>>()
					})
					.collect::<Vec<Vec<TypeName>>>();

				// Required default value for the variable parameter if not provided
				let default_type_sequence = default_value
					.split(",")
					.map(|individual_type| match self.parse_attribute_argument_type(individual_type) {
						TypeValueOrArgument::TypeValue(type_value) => type_value,
						TypeValueOrArgument::VariableArgument(variable_value) => {
							panic!(
								"Found the default variable value `{:?}` in the attribute declaration `{}` (which only allows typed values) when parsing XML layout",
								variable_value, attribute_declaration
							);
						},
					})
					.collect::<Vec<TypeValue>>();

				// TODO: Verify the default types match the specified allowed types

				// Return the parameter
				VariableParameter::new(name, type_sequence_options, default_type_sequence)
			},
			// Unrecognized type pattern
			_ => panic!("Invalid attribute attribute declaration `{}` when parsing XML layout", attribute_declaration),
		}
	}

	/// Replace escape characters in an XML string, only supports `&, <, >, ", '`
	fn unescape_xml(xml: &str) -> String {
		// Find and replace each escape character, starting with `&` to avoid unescaping other escape sequences
		xml.replace("&amp;", "&")
			.replace("&lt;", "<")
			.replace("&gt;", ">")
			.replace("&quot;", "\"")
			.replace("apos;", "'")
			.replace("&#39;", "'")
	}
}
