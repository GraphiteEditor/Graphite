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
	Dimension(Dimension),
	TemplateString(Vec<TemplateStringSegment>),
	Color(Color),
	Bool(bool),
	None,
}

impl TypeValue {
	/// Converts this to a dimension, panics if not possible.
	pub fn expect_dimension(&self) -> Dimension {
		match self {
			Self::Dimension(dimension) => *dimension,
			_ => panic!("expected a dimension"),
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub enum TemplateStringSegment {
	String(String),
	Argument(TypeValueOrArgument),
}

/// A dimension is a measure along an axis.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Dimension {
	/// Absolute value in pixels.
	AbsolutePx(f64),
	/// Percent of parent container size along the same axis.
	Percent(f64),
	/// Percent of free space remaining in parent container.
	PercentRemainder(f64),
	/// Minimum size required to fit the children.
	Inner,
	/// Size relative to the width of this component.
	Width,
	/// Size relative to the height of this component.
	Height,
}

/// Dimensions along a box's four sides.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BoxDimensions {
	pub top: Dimension,
	pub right: Dimension,
	pub bottom: Dimension,
	pub left: Dimension,
}

impl BoxDimensions {
	/// Construct new box dimensions, with values given for each side.
	pub fn new(top: Dimension, right: Dimension, bottom: Dimension, left: Dimension) -> Self {
		Self { top, right, bottom, left }
	}

	/// Construct new box dimensions, with same values used for top-bottom and left-right.
	pub fn symmetric(vertical: Dimension, horizontal: Dimension) -> Self {
		Self::new(vertical, horizontal, vertical, horizontal)
	}

	/// Construct new box dimensions with the same value for all sides.
	pub fn all(value: Dimension) -> Self {
		Self::new(value, value, value, value)
	}

	/// Sets the padding on the top and bottom sides.
	pub fn set_vertical(&mut self, value: Dimension) {
		self.top = value;
		self.bottom = value;
	}

	/// Sets the padding on the left and right sides.
	pub fn set_horizontal(&mut self, value: Dimension) {
		self.left = value;
		self.right = value;
	}
}
