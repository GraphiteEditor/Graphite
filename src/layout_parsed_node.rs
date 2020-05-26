#[derive(Debug)]
pub enum LayoutParsedNode {
	Tag(LayoutParsedTag),
	Text(String),
}

impl LayoutParsedNode {
	pub fn new_tag(namespace: String, tag: String) -> Self {
		Self::Tag(LayoutParsedTag::new(namespace, tag))
	}

	pub fn new_text(text: String) -> Self {
		Self::Text(text)
	}
}

#[derive(Debug)]
pub struct LayoutParsedTag {
	pub namespace: Option<String>,
	pub tag: String,
	pub attributes: Vec<(String, String)>,
}

impl LayoutParsedTag {
	pub fn new(namespace: String, tag: String) -> Self {
		let namespace = if namespace.is_empty() { None } else { Some(namespace) };

		Self {
			namespace,
			tag,
			attributes: Vec::new(),
		}
	}

	pub fn add_attribute(&mut self, attribute: (String, String)) {
		self.attributes.push(attribute);
	}
}