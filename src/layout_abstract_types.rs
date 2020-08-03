use crate::color::Color;
use crate::layout_abstract_syntax::*;

/// Parameter definition for an attribute in the root tag of a component XML layout
#[derive(Debug, Clone, PartialEq)]
pub struct VariableParameter {
	// Name of the variable binding that can be used within the component in {{template tags}}
	pub name: String,
	// Combinations of allowed sequences of types that can be passed to instances of this component
	pub type_sequence_options: Vec<Vec<TypeName>>,
	// A single sequence of default values that get used if an instance of this component never has the corresponding argument passed to it
	pub type_sequence_default: Vec<TypeValue>,
}

impl VariableParameter {
	/// Construct a parameter definition for a variable accepted by a component definition, with the variable name, allowed combinations of types, and the default value sequence
	pub fn new(name: String, valid_types: Vec<Vec<TypeName>>, default: Vec<TypeValue>) -> Self {
		Self {
			name,
			type_sequence_options: valid_types,
			type_sequence_default: default,
		}
	}
}

// ====================================================================================================

/// Wrapper for either a `TypeValue` struct or the name of a variable argument (just a `String`)
#[derive(Debug, Clone, PartialEq)]
pub enum TypeValueOrArgument {
	TypeValue(TypeValue),
	VariableArgument(String),
}

// ====================================================================================================

/// All possible names for types of values in the reactive data and layout system
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

// ====================================================================================================

/// Concrete values for data in the various types allowed by the reactive data and layout system
#[derive(Debug, Clone, PartialEq)]
pub enum TypeValue {
	Layout(Vec<NodeTree>),
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
			_ => panic!("Expected a dimension"),
		}
	}
}

// ====================================================================================================

/// A piece of a template string, made up of many of these enums concatenated together in alternating order between `String` and `Argument`, where the latter is a value or argument variable
#[derive(Debug, Clone, PartialEq)]
pub enum TemplateStringSegment {
	String(String),
	Argument(TypeValueOrArgument),
}

// ====================================================================================================

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

// ====================================================================================================

/// Dimensions along the four sides of a box layout
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
