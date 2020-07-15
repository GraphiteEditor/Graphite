use crate::layout_abstract_types::*;

#[derive(Debug, Clone, PartialEq)]
pub enum LayoutAbstractNode {
	Tag(LayoutAbstractTag),
	Text(String),
}

impl LayoutAbstractNode {
	pub fn new_tag(namespace: String, name: String) -> Self {
		Self::Tag(LayoutAbstractTag::new(namespace, name))
	}

	pub fn new_text(text: String) -> Self {
		Self::Text(text)
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutAbstractTag {
	pub namespace: String,
	pub name: String,
	/// Layout attributes, which are used by the layout engine.
	pub layout_attributes: LayoutAttributes,
	/// The special content attribute, representing the inner elements of this tag.
	pub content: Option<AttributeValue>,
	/// User-defined attributes, which are prefixed with ':'
	pub user_attributes: Vec<Attribute>,
}

impl LayoutAbstractTag {
	pub fn new(namespace: String, name: String) -> Self {
		Self {
			namespace,
			name,
			layout_attributes: Default::default(),
			content: None,
			user_attributes: Vec::new(),
		}
	}

	pub fn add_attribute(&mut self, attribute: Attribute) {
		// User-defined attribute
		if attribute.name.chars().next().unwrap() == ':' {
			self.user_attributes.push(attribute);
		}
		else {
			self.add_builtin_attribute(attribute);
		}
	}

	fn add_builtin_attribute(&mut self, attribute: Attribute) {
		match &attribute.name[..] {
			// The special `content` attribute
			"content" => self.content = Some(attribute.value),
			// Layout attributes, stored separately
			"width" => self.layout_attributes.width = attribute.dimension(),
			"height" => self.layout_attributes.height = attribute.dimension(),
			"x-align" => self.layout_attributes.x_align = attribute.percent(),
			"y-align" => self.layout_attributes.y_align = attribute.percent(),
			"x-padding" => self.layout_attributes.padding.set_horizontal(attribute.dimension()),
			"y-padding" => self.layout_attributes.padding.set_vertical(attribute.dimension()),
			"padding" => self.layout_attributes.padding = attribute.box_dimensions(),
			"x-spacing" => self.layout_attributes.spacing.set_horizontal(attribute.dimension()),
			"y-spacing" => self.layout_attributes.spacing.set_vertical(attribute.dimension()),
			"spacing" => self.layout_attributes.spacing = attribute.box_dimensions(),
			_ => panic!("unknown builtin attribute {}", attribute.name),
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
	pub name: String,
	pub value: AttributeValue,
}

impl Attribute {
	pub fn new(name: String, value: AttributeValue) -> Self {
		Self { name, value }
	}

	/// Extracts this attribute's values as typed values.
	fn values(self) -> Vec<TypeValue> {
		if let AttributeValue::TypeValue(values) = self.value {
			values
				.into_iter()
				.map(|value| {
					if let TypeValueOrArgument::TypeValue(value) = value {
						value
					}
					else {
						todo!("variable arguments are note yet supported")
					}
				})
				.collect()
		}
		else {
			todo!("variable arguments are not yet supported")
		}
	}

	/// Converts this attribute's value into a single dimension.
	fn dimension(self) -> Dimension {
		let values = self.values();
		assert_eq!(values.len(), 1, "expected a single value");
		values[0].expect_dimension()
	}

	/// Extracts a percentage from this attribute's value.
	fn percent(self) -> f32 {
		match self.dimension() {
			Dimension::Percent(value) => value,
			_ => panic!("expected a percentage"),
		}
	}

	/// Converts this attribute's values into box dimensions.
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
			_ => panic!("expected 1, 2 or 4 values"),
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttributeValue {
	VariableParameter(VariableParameter),
	TypeValue(Vec<TypeValueOrArgument>),
}

/// Layout-specific attributes.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutAttributes {
	pub width: Dimension,
	pub height: Dimension,
	pub x_align: f32,
	pub y_align: f32,
	pub spacing: BoxDimensions,
	pub padding: BoxDimensions,
}

impl Default for LayoutAttributes {
	fn default() -> Self {
		let zero_box = BoxDimensions::all(Dimension::AbsolutePx(0.0));
		Self {
			width: Dimension::Inner,
			height: Dimension::Inner,
			x_align: 0.0,
			y_align: 0.0,
			spacing: zero_box,
			padding: zero_box,
		}
	}
}
