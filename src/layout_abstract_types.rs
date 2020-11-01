use crate::color::Color;
use crate::layout_abstract_syntax::*;

/// Definition of a prop for a component, given in an attribute of the XML root tag
#[derive(Debug, Clone, PartialEq)]
pub struct PropDefinition {
	// Name of the variable binding that can be used within the component in {{template tags}}
	pub variable_name: String,
	// Combinations of allowed sequences of types that can be passed to instances of this component
	pub type_sequence_options: Vec<Vec<TypeName>>,
	// A single sequence of default values that get used if an instance of this component never has the corresponding argument passed to it
	pub type_sequence_default: Vec<TypedValue>,
}

impl PropDefinition {
	/// Construct a prop definition for a variable accepted by a component definition, with the variable name, valid combinations of types, and the default value sequence
	pub fn new(variable_name: String, valid_types: Vec<Vec<TypeName>>, default: Vec<TypedValue>) -> Self {
		Self {
			variable_name,
			type_sequence_options: valid_types,
			type_sequence_default: default,
		}
	}
}

// ====================================================================================================

/// Wrapper for either a `TypedValue` struct or the name of a prop
#[derive(Debug, Clone, PartialEq)]
pub enum TypedValueOrVariableName {
	TypedValue(TypedValue),
	VariableName(String),
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
pub enum TypedValue {
	Layout(Vec<NodeTree>),
	Integer(i64),
	Decimal(f64),
	Dimension(Dimension),
	TemplateString(Vec<TemplateStringSegment>),
	Color(Color),
	Bool(bool),
	None,
}

impl TypedValue {
	/// Converts this to a dimension, panics if not possible.
	pub fn expect_dimension(&self) -> Dimension {
		match self {
			Self::Dimension(dimension) => *dimension,
			_ => panic!("Expected a dimension"),
		}
	}
}

// ====================================================================================================

/// A piece of a template string, made up of many of these enums concatenated together in alternating order between `String` and `Argument`, where the latter is a value or variable name
#[derive(Debug, Clone, PartialEq)]
pub enum TemplateStringSegment {
	String(String),
	Argument(TypedValueOrVariableName),
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
