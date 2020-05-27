use crate::layout_abstract_attributes::*;

#[derive(Debug)]
pub struct LayoutAbstractSyntaxNode {
	pub namespace: Option<String>,
	pub name: String,
	pub attributes: Vec<Attribute>,
}

impl LayoutAbstractSyntaxNode {
	pub fn new(namespace: Option<String>, tag: String, attributes: &Vec<(String, String)>) -> Self {
		for attribute in attributes {
			let parsed = parse_attribute(&attribute.1[..]);
			println!("{} : {:?} -> {:?}", attribute.0, attribute.1, parsed);
		}

		Self {
			namespace,
			name: tag,
			attributes: Vec::new(),
		}
	}
}