use std::fs;
use std::io;
use crate::layout_parsed_node::*;

pub struct ComponentLayout {

}

impl ComponentLayout {
	pub fn new() -> ComponentLayout {
		let parsed_layout_tree = Self::parse_xml_file("gui/window/main.xml").unwrap();
		Self::interpret_abstract_syntax_tree(parsed_layout_tree);
		Self {}
	}
	
	pub fn parse_xml_file(path: &str) -> io::Result<rctree::Node<LayoutParsedNode>> {
		let source = fs::read_to_string(path)?;
		let parsed = xmlparser::Tokenizer::from(&source[..]);

		let mut stack: Vec<rctree::Node<LayoutParsedNode>> = Vec::new();
		let mut current: Option<rctree::Node<LayoutParsedNode>> = None;
		let mut result: Option<rctree::Node<LayoutParsedNode>> = None;
		
		for token in parsed {
			match token.unwrap() {
				xmlparser::Token::ElementStart { prefix, local, .. } => {
					let namespace = String::from(prefix.as_str());
					let tag_name = String::from(local.as_str());

					let new_parsed_layout_node = LayoutParsedNode::new_tag(namespace, tag_name);

					let new_node = rctree::Node::new(new_parsed_layout_node);
					current = Some(new_node);
				}
				xmlparser::Token::Attribute { prefix, local, value, .. } => {
					let colon_prefixed = prefix.start() > 0 && (prefix.start() == prefix.end());
					let key = if colon_prefixed {
						let slice = local.as_str();
						let mut string = String::with_capacity(slice.len() + 1);
						string.push(':');
						string.push_str(slice);
						string
					} else { String::from(local.as_str()) };
					let value = String::from(value.as_str());
					let attribute = (key, value);

					match &mut current {
						Some(current_node) => {
							match &mut *current_node.borrow_mut() {
								LayoutParsedNode::Tag(tag) => {
									// Add this attribute to the current node that has not yet reached its closing angle bracket
									tag.add_attribute(attribute);
								}
								LayoutParsedNode::Text(_) => {
									panic!("Error adding attribute to tag when parsing XML layout in file: {}", path);
								}
							}
						}
						None => {
							panic!("Error adding attribute to tag when parsing XML layout in file: {}", path);
						}
					}
				}
				xmlparser::Token::ElementEnd { end, .. } => {
					match end {
						// After adding any attributes, the opening tag ends
						xmlparser::ElementEnd::Open => {
							// After adding any attributes, we are now a layer deeper which the stack keeps track of
							let node_to_push = current.take().expect(&format!("Invalid syntax when parsing XML layout in file {}", path)[..]);
							stack.push(node_to_push);
						}
						// After adding any attributes, the self-closing tag ends
						xmlparser::ElementEnd::Empty => {
							let parent_node = stack.last_mut().expect(&format!("Invalid syntax when parsing XML layout in file: {}", path)[..]);
							let new_child = current.take().expect(&format!("Invalid syntax when parsing XML layout in file: {}", path)[..]);
							parent_node.append(new_child);
						}
						// The closing tag is reached
						xmlparser::ElementEnd::Close(..) => {
							let popped_node = stack.pop().expect(&format!("Encountered extra closing tag when parsing XML layout in file: {}", path)[..]);
							match stack.last_mut() {
								Some(parent_node) => {
									parent_node.append(popped_node);
								}
								None => {
									match result {
										None => result = Some(popped_node),
										Some(_) => panic!("Encountered multiple root-level tags when parsing XML layout in file: {}", path),
									}
								}
							}
						}
					}
				}
				xmlparser::Token::Text { text } => {
					let parent_node = stack.last_mut().expect(&format!("Encountered text outside the root tag when parsing XML layout in file: {}", path)[..]);
					let text_string = String::from(text.as_str());

					if !text_string.trim().is_empty() {
						let text_node = LayoutParsedNode::new_text(text_string);
						let new_node = rctree::Node::new(text_node);
						parent_node.append(new_node);
					}
				}
				_ => {}
			}
		}
		
		match result {
			None => panic!("Invalid syntax when parsing XML layout in file: {}", path),
			Some(tree) => Ok(tree)
		}
	}

	pub fn interpret_abstract_syntax_tree(root: rctree::Node<LayoutParsedNode>) {
		for node in root.descendants() {
			println!("{:?}", node);
		}
	}
}