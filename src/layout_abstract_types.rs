use crate::color::Color;
use crate::layout_abstract_syntax::*;

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
pub enum TypeValueOrArgument {
	TypeValue(TypeValue),
	VariableArgument(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeName {
	Layout,
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

pub type ComponentAst = rctree::Node<LayoutAbstractNode>;
pub type Component = Vec<LayoutAbstractNode>;

#[derive(Debug, Clone, PartialEq)]
pub enum TypeValue {
	Layout(Vec<ComponentAst>),
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

#[derive(Debug, Clone, PartialEq)]
pub enum TemplateStringSegment {
	String(String),
	Argument(TypeValueOrArgument),
}
