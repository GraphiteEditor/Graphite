use crate::color::Color;

#[derive(Debug)]
pub struct VariableParameter {
	pub name: String,
	pub type_sequence_options: Vec<Vec<TypeName>>,
	pub type_sequence_default: Vec<TypeValue>,
}

impl VariableParameter {
	pub fn new(name: String, valid_types: Vec<Vec<TypeName>>, default: Vec<TypeValue>) -> Self {
		Self {
			name,
			type_sequence_options: valid_types,
			type_sequence_default: default,
		}
	}
}

#[derive(Debug)]
pub struct VariableArgument {
	pub name: String,
}

impl VariableArgument {
	pub fn new(name: String) -> Self {
		Self { name }
	}
}

#[derive(Debug)]
pub enum TypeValueOrArgument {
	TypeValue(TypeValue),
	VariableArgument(VariableArgument),
}

#[derive(Debug)]
pub enum TypeName {
	// GuiXml, // TODO
	Integer,
	Decimal,
	AbsolutePx,
	Percent,
	PercentRemainder,
	Inner,
	Width,
	Height,
	TemplateString,
	Color,
	Bool,
	None,
}

#[derive(Debug)]
pub enum TypeValue {
	// GuiXml(()), // TODO
	Integer(i64),
	Decimal(f64),
	AbsolutePx(f32),
	Percent(f32),
	PercentRemainder(f32),
	Inner,
	Width,
	Height,
	TemplateString(Vec<TemplateStringSegment>),
	Color(Color),
	Bool(bool),
	None,
}

#[derive(Debug)]
pub enum TemplateStringSegment {
	String(String),
	Argument(TypeValueOrArgument),
}
