use crate::color::Color;
use crate::color_palette::ColorPalette;
use crate::layout_abstract_syntax::*;
use crate::layout_abstract_types::*;

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
			r"^\s*(\w*)\s*(:)\s*(\()\s*((?:(?:\w+)(?:\s*,\s*\w+)*)(?:\s*\|\s*(?:(?:\w+)(?:\s*,\s*\w+)*))*)\s*(\))\s*(=)\s*([\s\w'\[\]@%\-.`,]*?)\s*$",
		)
		.unwrap();

		let capture_attribute_type_sequences_regex: regex::Regex = regex::Regex::new(concat!(
			// Argument: {{?}}
			r#"^\s*(\{\{)\s*(\w*)\s*(\}\})\s*$|"#,
			// Integer: ?
			r#"^\s*(-?\d+)\s*$|"#,
			// Decimal: ?
			r#"^\s*(-?(?:(?:\d+\.\d*)|(?:\d*\.\d+)))\s*$|"#,
			// AbsolutePx: px
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

	pub fn parse_attribute_types(&self, input: &str) -> AttributeValue {
		let attribute_types = input.split(",").map(|piece| piece.trim()).collect::<Vec<&str>>();
		let list = attribute_types
			.iter()
			.map(|attribute_type| self.parse_attribute_type(attribute_type))
			.collect::<Vec<TypeValueOrArgument>>();
		AttributeValue::TypeValue(list)
	}

	pub fn parse_attribute_type(&self, attribute_type: &str) -> TypeValueOrArgument {
		// Match with the regular expression
		let captures = self
			.capture_attribute_type_sequences_regex
			.captures(attribute_type)
			.map(|captures| captures.iter().skip(1).flat_map(|c| c).map(|c| c.as_str()).collect::<Vec<_>>());

		// Match against the captured values as a list of tokens
		let tokens = captures.as_ref().map(|c| c.as_slice());
		match tokens {
			// Argument: {{?}}
			Some(["{{", name, "}}"]) => {
				let name = String::from(*name);
				TypeValueOrArgument::VariableArgument(VariableArgument::new(name))
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
					.parse::<f32>()
					.expect(&format!("Invalid value `{}` specified in the attribute type`{}` when parsing XML layout", value, attribute_type)[..]);
				TypeValueOrArgument::TypeValue(TypeValue::AbsolutePx(pixels))
			},
			// Percent: ?%
			Some([value, "%"]) => {
				let percent = value
					.parse::<f32>()
					.expect(&format!("Invalid value `{}` specified in the attribute type `{}` when parsing XML layout", value, attribute_type)[..]);
				TypeValueOrArgument::TypeValue(TypeValue::Percent(percent))
			},
			// PercentRemainder: ?@
			Some([value, "@"]) => {
				let percent_remainder = value
					.parse::<f32>()
					.expect(&format!("Invalid value `{}` specified in the attribute type `{}` when parsing XML layout", value, attribute_type)[..]);
				TypeValueOrArgument::TypeValue(TypeValue::PercentRemainder(percent_remainder))
			},
			// Inner: inner
			Some([inner]) if inner.eq_ignore_ascii_case("inner") => TypeValueOrArgument::TypeValue(TypeValue::Inner),
			// Width: width
			Some([width]) if width.eq_ignore_ascii_case("width") => TypeValueOrArgument::TypeValue(TypeValue::Width),
			// Height: height
			Some([height]) if height.eq_ignore_ascii_case("height") => TypeValueOrArgument::TypeValue(TypeValue::Height),
			// TemplateString: `? ... {{?}} ...`
			Some(["`", string, "`"]) => {
				let mut segments = Vec::<TemplateStringSegment>::new();
				let mut is_template = false;

				for part in self.split_by_string_templates_regex.split(string) {
					let segment = match is_template {
						true => TemplateStringSegment::String(String::from(part)),
						false => TemplateStringSegment::Argument(TypeValueOrArgument::VariableArgument(VariableArgument::new(String::from(part)))),
					};
					segments.push(segment);
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

	pub fn parse_attribute_declaration(&self, attribute_declaration: &str) -> AttributeValue {
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
									// "layout" => TypeName::Layout, // TODO
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
					.map(|individual_type| match self.parse_attribute_type(individual_type) {
						TypeValueOrArgument::TypeValue(type_value) => type_value,
						TypeValueOrArgument::VariableArgument(variable_value) => {
							panic!(
								"Found the default variable value `{:?}` in the attribute declaration `{}` which only allows typed values, when parsing XML layout",
								variable_value, attribute_declaration
							);
						},
					})
					.collect::<Vec<TypeValue>>();

				// Return the parameter
				AttributeValue::VariableParameter(VariableParameter::new(name, type_sequence_options, default_type_sequence))
			},
			// Unrecognized type pattern
			_ => panic!("Invalid attribute attribute declaration `{}` when parsing XML layout", attribute_declaration),
		}
	}
}
