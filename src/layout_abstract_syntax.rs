use crate::layout_abstract_types::*;

// AST for a component with info on its definition (from the root element of the XML layout) and a vector of direct child component tags
pub struct FlatComponent {
	// The abstract definition of the root node of the component with attribute parameters
	pub own_info: LayoutComponentDefinition,
	// Only stores tags, text elements are disposed of (they'd be meaningless in a tag list)
	pub child_components: Vec<LayoutComponentTag>,
}

/// A component in its final processed form (after parsing its XML file), with information on its definition with a list of child components with their own children in their `content` attributes
impl FlatComponent {
	// Construct a layout component which stores its own root-level component definition (with attribute parameters, etc.) and a flat list of its direct child tags, each with an AST in their `content` attribute
	pub fn new(own_info: LayoutComponentDefinition, child_components: Vec<LayoutComponentTag>) -> FlatComponent {
		Self { own_info, child_components }
	}

	/// Print the component (for debugging)
	#[allow(dead_code)]
	pub fn debug_print(&self) {
		println!("Flat Component: {:#?}", self.own_info);
		for tag in &self.child_components {
			tag.debug_print();
		}
	}
}

// ====================================================================================================

/// Wrapper for either a `LayoutComponentNode` enum or `LayoutComponentDefinition` struct
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutComponentNodeOrDefinition {
	LayoutComponentNode(LayoutComponentNode),
	LayoutComponentDefinition(LayoutComponentDefinition),
}

// ====================================================================================================

/// AST of `LayoutComponentNode`s which hold either a tag or text node
pub type NodeTree = rctree::Node<LayoutComponentNode>;

/// AST similar to `NodeTree` (a tree of `LayoutComponentNode`s) but this holds the wrapped values `LayoutComponentNodeOrDefinition` (unwrap them with `LayoutSystem::node_tree_from_node_or_def_tree()`)
pub type NodeOrDefTree = rctree::Node<LayoutComponentNodeOrDefinition>;

// ====================================================================================================

/// Representation of an XML node with either another XML tag (`LayoutComponentTag`) or a text node (just a `String`)
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutComponentNode {
	Tag(LayoutComponentTag),
	Text(String),
}

impl LayoutComponentNode {
	/// Given a tag name in "namespace:name" format, construct a `LayoutComponentNode` that wraps a newly constructed `LayoutComponentTag` struct based on the provided name
	pub fn new_tag(name: (String, String)) -> Self {
		Self::Tag(LayoutComponentTag::new(name))
	}

	/// Given some text hanging out in the XML between tags, construct a `LayoutComponentNode` with that text which simply stores the provided `String`
	pub fn new_text(text: String) -> Self {
		Self::Text(text)
	}

	/// Print the component node (for debugging)
	#[allow(dead_code)]
	pub fn debug_print(&self) {
		match self {
			LayoutComponentNode::Tag(tag) => tag.debug_print(),
			LayoutComponentNode::Text(text) => println!("================> Text Node: {}", text),
		}
	}
}

// ====================================================================================================

/// Abstract representation of a component based on the parameters defined by the root tag of a component XML layout
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutComponentDefinition {
	/// Name of the component in "namespace:name" format
	pub name: (String, String),
	/// User-defined attribute parameters, which are prefixed with ':'
	pub user_attributes: Vec<VariableParameter>,
}

impl LayoutComponentDefinition {
	/// Construct a definition for a layout component given its name in "namespace:name" format with an empty set of parameters
	pub fn new(name: (String, String)) -> Self {
		let user_attributes = vec![];
		Self { name, user_attributes }
	}

	/// Add a parameter definition (with its name, types, and default value) to this component definition
	pub fn add_parameter(&mut self, parameter: VariableParameter) {
		self.user_attributes.push(parameter);
	}
}

// ====================================================================================================

/// Abstract representation of a tag inside an abstract component with attributes and descendant content
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutComponentTag {
	/// Name of the tag's referenced component in "namespace:name" format
	pub name: (String, String),
	/// Layout engine attribute arguments, which are used by the layout engine
	pub layout_arguments: LayoutAttributes,
	/// User-defined attribute arguments, which are prefixed with ':'
	pub user_arguments: Vec<AttributeArg>,
	/// The special content attribute, representing the inner elements of this tag
	pub content: Option<Vec<NodeTree>>,
}

impl LayoutComponentTag {
	/// Construct a tag in an XML layout component based on its referenced component name (in "namespace:name" format) and empty defaults
	pub fn new(name: (String, String)) -> Self {
		Self {
			name,
			layout_arguments: Default::default(),
			content: None,
			user_arguments: Vec::new(),
		}
	}

	/// Provide a sequence of ASTs for this component's content attribute
	pub fn set_content(&mut self, content: Vec<NodeTree>) {
		self.content = Some(content);
	}

	/// Add an XML tag attribute to this component (either a layout engine attribute, user-defined custom attribute, or event handler attribute)
	pub fn add_attribute(&mut self, attribute: AttributeArg) {
		// User-defined attribute (for reactive data system)
		if attribute.name.len() > 1 && &attribute.name[..1] == ":" {
			self.add_user_attribute(attribute);
		}
		// Event handler attribute (for event system)
		else if attribute.name.len() > 3 && &attribute.name[..3] == "on:" {
			todo!("Event attributes not implemented yet");
		}
		// Layout attribute (for layout engine)
		else {
			self.add_layout_attribute(attribute);
		}
	}

	/// Add an XML tag attribute to this component for a colon-prefixed custom user-defined variable value
	fn add_user_attribute(&mut self, attribute: AttributeArg) {
		self.user_arguments.push(attribute);
	}

	/// Add an XML tag attribute to this component for a non-prefixed layout engine value
	fn add_layout_attribute(&mut self, attribute: AttributeArg) {
		match &attribute.name[..] {
			// Layout attributes, stored separately
			"width" => self.layout_arguments.width = attribute.dimension(),
			"height" => self.layout_arguments.height = attribute.dimension(),
			"x-align" => self.layout_arguments.x_align = attribute.percent(),
			"y-align" => self.layout_arguments.y_align = attribute.percent(),
			"x-padding" => self.layout_arguments.padding.set_horizontal(attribute.dimension()),
			"y-padding" => self.layout_arguments.padding.set_vertical(attribute.dimension()),
			"padding" => self.layout_arguments.padding = attribute.box_dimensions(),
			"x-gap" => self.layout_arguments.gap.set_horizontal(attribute.dimension()),
			"y-gap" => self.layout_arguments.gap.set_vertical(attribute.dimension()),
			"gap" => self.layout_arguments.gap = attribute.box_dimensions(),
			_ => panic!("Unknown builtin attribute `{}`", attribute.name),
		}
	}

	/// Print the layout tag (for debugging)
	pub fn debug_print(&self) {
		println!("Tag Node: {:#?}", self);
		if let Some(ref content) = self.content {
			for child in content {
				for node in child.descendants() {
					println!("> Descendant Node: {:#?}", node);
				}
			}
		}
	}
}

// ====================================================================================================

/// Name-value pair for an argument used in the attribute variable system, where the name is a `String` and the value sequence is a vector of `TypeValueOrArgument`s
#[derive(Debug, Clone, PartialEq)]
pub struct AttributeArg {
	pub name: String,
	pub value: Vec<TypeValueOrArgument>,
}

impl AttributeArg {
	/// Construct a name-value pair representing an argument on a layout tag given its name and sequence of values
	pub fn new(name: String, value: Vec<TypeValueOrArgument>) -> Self {
		Self { name, value }
	}

	/// Extract this attribute's values as typed values
	fn values(self) -> Vec<TypeValue> {
		self.value
			.into_iter()
			.map(|value| {
				if let TypeValueOrArgument::TypeValue(value) = value {
					value
				}
				else {
					todo!("Variable arguments are note yet supported")
				}
			})
			.collect()
	}

	/// Convert this attribute's value into a single dimension
	fn dimension(self) -> Dimension {
		let values = self.values();
		assert_eq!(values.len(), 1, "Expected a single value");
		values[0].expect_dimension()
	}

	/// Extract a percentage from this attribute's value
	fn percent(self) -> f64 {
		match self.dimension() {
			Dimension::Percent(value) => value,
			_ => panic!("Expected a percentage"),
		}
	}

	/// Convert this attribute's values into box dimensions
	fn box_dimensions(self) -> BoxDimensions {
		let values = self.values();
		match values.len() {
			1 => {
				let value = values[0].expect_dimension();
				BoxDimensions::all(value)
			},
			2 => {
				let vertical = values[0].expect_dimension();
				let horizontal = values[1].expect_dimension();
				BoxDimensions::symmetric(vertical, horizontal)
			},
			4 => {
				let top = values[0].expect_dimension();
				let right = values[1].expect_dimension();
				let bottom = values[2].expect_dimension();
				let left = values[3].expect_dimension();
				BoxDimensions::new(top, right, bottom, left)
			},
			_ => panic!("Expected 1, 2 or 4 values"),
		}
	}
}

// ====================================================================================================

/// Attributes used by the layout engine to calculate sizing and placement
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutAttributes {
	pub width: Dimension,
	pub height: Dimension,
	pub x_align: f64,
	pub y_align: f64,
	pub gap: BoxDimensions,
	pub padding: BoxDimensions,
}

impl Default for LayoutAttributes {
	/// Provide default values for dimensions, alignment, and outside spacing
	fn default() -> Self {
		let zero_box = BoxDimensions::all(Dimension::AbsolutePx(0.0));
		Self {
			width: Dimension::Inner,
			height: Dimension::Inner,
			x_align: 0.0,
			y_align: 0.0,
			gap: zero_box,
			padding: zero_box,
		}
	}
}
