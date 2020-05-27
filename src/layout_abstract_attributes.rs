use crate::layout_abstract_types::*;
use crate::color_palette::ColorPalette;
use crate::color::Color;

#[derive(Debug)]
pub enum Attribute {
	VariableValue(VariableValue),
	TypeValue(TypeValue),
}

pub fn parse_attribute(input: &str) -> Attribute {
	// Match variables and typed values that can be in an attribute
	let regex = regex::Regex::new(
		r#"(?x)
		^\s*(\w*)\s*(:)\s*(\()\s*(\w*\s*(?:\|\s*\w*\s*?)*)\s*(\))\s*(=)\s*(\w*)\s*$    | # Parameter           ?: (? | ... | ?) = ?
		^\s*(\{\{)\s*(\w*)\s*(\}\})\s*$                                                | # Argument            {{?}}
		^\s*(-?\d+)\s*$                                                                | # Integer             ?
		^\s*(-?(?:(?:\d+\.\d*)|(?:\d*\.\d+)))\s*$                                      | # Decimal             ?
		^\s*(-?(?:(?:\d+(?:\.\d*)?)|(?:\d*(?:\.\d+))))([Pp][Xx])\s*$                   | # AbsolutePx          ?px
		^\s*(-?(?:(?:\d+(?:\.\d*)?)|(?:\d*(?:\.\d+))))(%)\s*$                          | # Percent             ?%
		^\s*(-?(?:(?:\d+(?:\.\d*)?)|(?:\d*(?:\.\d+))))(@)\s*$                          | # PercentRemainder    ?@
		^\s*([Ii][Nn][Nn][Ee][Rr])\s*$                                                 | # Inner               inner
		^\s*([Ww][Ii][Dd][Tt][Hh])\s*$                                                 | # Width               width
		^\s*([Hh][Ee][Ii][Gg][Hh][Tt])\s*$                                             | # Height              height
		^\s*`(.*)`\s*$                                                                 | # TemplateString      `? ... {{?}} ...`
		^\s*(\[)(.*)(\])\s*$                                                           | # Color               [?]
		^\s*([Tt][Rr][Uu][Ee]|[Ff][Aa][Ll][Ss][Ee])\s*$                                | # Bool                true/false
		^\s*([Nn][Oo][Nn][Ee])\s*$                                                       # None                none
		"#
	).unwrap();

	// Match with the regular expression
	let captures = regex.captures(input).map(|captures|
		captures
			.iter()
			.skip(1)
			.flat_map(|c| c)
			.map(|c| c.as_str())
			.collect::<Vec<_>>()
	);

	// Match against the captured values as a slice
	let slices = captures.as_ref().map(|c| c.as_slice());
	match slices {
		Some([name, ":", "(", types, ")", "=", default_value]) => {
			// TODO: Extend to support a list of N types (like (AbsolutePx) (AbsolutePx) (AbsolutePx) (AbsolutePx))
			let name = String::from(*name);

			let split_types = types.split("|").map(|piece| piece.trim());
			let valid_types = split_types.map(|type_name|
				match &type_name.to_ascii_lowercase()[..] {
					"xml" => TypeName::Xml,
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
					invalid => panic!("Invalid type `{}` specified in the attribute `{}` when parsing XML layout", invalid, input),
				}
			).collect::<Vec<_>>();

			let default = match parse_attribute(default_value) {
				Attribute::TypeValue(type_value) => type_value,
				Attribute::VariableValue(variable_value) => panic!("Found the variable value `{:?}` in the attribute `{}` which only allows typed values, when parsing XML layout", variable_value, input),
			};

			Attribute::VariableValue(VariableValue::Parameter(VariableParameter {name, valid_types, default}))
		}
		Some(["{{", name, "}}"]) => {
			let name = String::from(*name);
			Attribute::VariableValue(VariableValue::Argument(name))
		}
		Some([value]) if regex::Regex::new(r"^\s*(-?\d+)\s*$").unwrap().is_match(value) => {
			let integer = value.parse::<i64>().expect(&format!("Invalid value `{}` specified in the attribute `{}` when parsing XML layout", value, input)[..]);
			Attribute::TypeValue(TypeValue::Integer(integer))
		}
		Some([value]) if regex::Regex::new(r"^\s*(-?(?:(?:\d+\.\d*)|(?:\d*\.\d+)))\s*$").unwrap().is_match(value) => {
			let decimal = value.parse::<f64>().expect(&format!("Invalid value `{}` specified in the attribute `{}` when parsing XML layout", value, input)[..]);
			Attribute::TypeValue(TypeValue::Decimal(decimal))
		}
		Some([value, px]) if px.eq_ignore_ascii_case("px") => {
			let pixels = value.parse::<f32>().expect(&format!("Invalid value `{}` specified in the attribute `{}` when parsing XML layout", value, input)[..]);
			Attribute::TypeValue(TypeValue::AbsolutePx(pixels))
		}
		Some([value, "%"]) => {
			let percent = value.parse::<f32>().expect(&format!("Invalid value `{}` specified in the attribute `{}` when parsing XML layout", value, input)[..]);
			Attribute::TypeValue(TypeValue::Percent(percent))
		}
		Some([value, "@"]) => {
			let percent_remainder = value.parse::<f32>().expect(&format!("Invalid value `{}` specified in the attribute `{}` when parsing XML layout", value, input)[..]);
			Attribute::TypeValue(TypeValue::PercentRemainder(percent_remainder))
		}
		Some([inner]) if inner.eq_ignore_ascii_case("inner") => {
			Attribute::TypeValue(TypeValue::Inner)
		}
		Some([width]) if width.eq_ignore_ascii_case("width") => {
			Attribute::TypeValue(TypeValue::Width)
		}
		Some([height]) if height.eq_ignore_ascii_case("height") => {
			Attribute::TypeValue(TypeValue::Height)
		}
		Some(["`", string, "`"]) => {
			let mut segments = Vec::<TemplateStringSegment>::new();
			let mut is_template = false;
			
			let regex = regex::Regex::new(r"\{\{|\}\}").unwrap();
			for part in regex.split(string) {
				let segment = match is_template {
					true => TemplateStringSegment::String(String::from(part)),
					false => TemplateStringSegment::Argument(VariableArgument { name: String::from(part) }),
				};
				segments.push(segment);
				is_template = !is_template;
			}

			Attribute::TypeValue(TypeValue::TemplateString(segments))
		}
		Some(["[", color_name, "]"]) => {
			let regex = regex::Regex::new(r"\s*'(.*)'\s*").unwrap();
			let color = match regex.captures(color_name) {
				Some(captures) => {
					let palette_color = captures.get(1).expect(&format!("Invalid palette color name `{}` specified in the attribute `{}` when parsing XML layout", color_name, input)[..]).as_str();
					ColorPalette::lookup_palette_color(palette_color).into_color_srgb()
				}
				None => {
					let parsed = color_name.parse::<css_color_parser::Color>();
					let css_color = parsed.expect(&format!("Invalid CSS color name `{}` specified in the attribute `{}` when parsing XML layout", color_name, input)[..]);
					Color::new(css_color.r as f32 / 255.0, css_color.g as f32 / 255.0, css_color.b as f32 / 255.0, css_color.a as f32 / 255.0)
				}
			};

			Attribute::TypeValue(TypeValue::Color(color))
		}
		Some([true_or_false]) if true_or_false.eq_ignore_ascii_case("true") || true_or_false.eq_ignore_ascii_case("false") => {
			let boolean = true_or_false.eq_ignore_ascii_case("true");
			Attribute::TypeValue(TypeValue::Bool(boolean))
		}
		Some([none]) if none.eq_ignore_ascii_case("none") => {
			Attribute::TypeValue(TypeValue::None)
		}
		_ => panic!("Invalid attribute value `{}` when parsing XML layout", input),
	}
}
