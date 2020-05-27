use crate::color::Color;

// Variable types

#[derive(Debug)]
pub enum VariableValue {
	Parameter(VariableParameter),
	Argument(String),
}

#[derive(Debug)]
pub struct VariableParameter {
	pub name: String,
	pub valid_types: Vec<TypeName>,
	pub default: TypeValue,
	// pub value: TypeValue,
}

#[derive(Debug)]
pub struct VariableArgument {
	pub name: String,
}

// Value types

#[derive(Debug)]
pub enum TypeName {
	Xml,
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
	Xml(()), // TODO
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
	Argument(VariableArgument),
}
