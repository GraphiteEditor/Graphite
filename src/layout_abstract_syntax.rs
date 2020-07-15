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
	pub attributes: Vec<Attribute>,
}

impl LayoutAbstractTag {
	pub fn new(namespace: String, name: String) -> Self {
		Self {
			namespace,
			name,
			attributes: Vec::new(),
		}
	}

	pub fn add_attribute(&mut self, attribute: Attribute) {
		self.attributes.push(attribute);
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
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttributeValue {
	VariableParameter(VariableParameter),
	TypeValue(Vec<TypeValueOrArgument>),
}
