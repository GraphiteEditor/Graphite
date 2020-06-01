use std::fs;
use std::io;
use std::collections::HashSet;
use crate::layout_abstract_syntax::*;
use crate::layout_attribute_parser::*;
use crate::resource_cache::ResourceCache;

pub struct GuiLayout {
	pub loaded_layouts: ResourceCache<rctree::Node<LayoutAbstractNode>>,
	attribute_parser: AttributeParser,
}

impl GuiLayout {
	pub fn new() -> GuiLayout {
		Self {
			loaded_layouts: ResourceCache::new(),
			attribute_parser: AttributeParser::new(),
		}
	}

	pub fn load_layout(&mut self, namespace: &str, name: &str) {
		// Load and parse the XML layout
		let xml_path = self.layout_xml_path(namespace, name);
		let window_main = self.parse_xml_file(&xml_path[..]).unwrap();

		// Keep track of it being loaded to prevent duplicate work
		let mut already_loaded_layouts = HashSet::new();
		already_loaded_layouts.insert(format!("{}:{}", namespace, name));

		// Load XML files recursively for all tags referenced in window:main and within those layouts
		self.explore_referenced_layouts(&window_main, &mut already_loaded_layouts);
		let tag_name = self.layout_name(namespace, name);
		self.loaded_layouts.set(&tag_name[..], window_main);
	}

	fn explore_referenced_layouts(&mut self, layout_tree_root: &rctree::Node<LayoutAbstractNode>, already_loaded_layouts: &mut HashSet<String>) {
		for child_tag in layout_tree_root.descendants() {
			match & *child_tag.borrow() {
				// Tags are references to other XML layouts that should be loaded and cached
				LayoutAbstractNode::Tag(layout_abstract_tag) => {
					// Cache key in form namespace:name
					let key = self.layout_name(&layout_abstract_tag.namespace[..], &layout_abstract_tag.name[..]);

					if !already_loaded_layouts.contains(&key[..]) {
						// Check if the cache has the loaded layout and load it if not
						match self.loaded_layouts.get(&key[..]) {
							// Tag has not been loaded, so load it now
							None => {
								// Load the layout for the visited tag
								let xml_path = self.layout_xml_path(&layout_abstract_tag.namespace[..], &layout_abstract_tag.name[..]);
								let new_loaded_layout = self.parse_xml_file(&xml_path[..]).unwrap();

								// Keep track of it being loaded to prevent duplicate work
								let key_copy = key.clone();
								already_loaded_layouts.insert(key);
								
								// Recursively explore the newly loaded layout's tags
								self.explore_referenced_layouts(&new_loaded_layout, already_loaded_layouts);

								self.loaded_layouts.set(&key_copy[..], new_loaded_layout);
							}
							// Tag has already been loaded
							Some(_) => {}
						}
					}
				}
				// Text nodes don't need to be loaded
				LayoutAbstractNode::Text(_) => {}
			};
		}
	}

	fn layout_name(&self, namespace: &str, name: &str) -> String {
		if namespace.len() > 0 {
			format!("{}:{}", namespace, name)
		}
		else {
			String::from(name)
		}
	}

	fn layout_xml_path(&self, namespace: &str, name: &str) -> String {
		if namespace.len() > 0 {
			format!("gui/{}/{}.xml", namespace, name)
		}
		else {
			format!("gui/{}.xml", name)
		}
	}

	fn parse_xml_file(&self, path: &str) -> io::Result<rctree::Node<LayoutAbstractNode>> {
		let source = fs::read_to_string(path)?;
		let parsed = xmlparser::Tokenizer::from(&source[..]);

		let mut stack: Vec<rctree::Node<LayoutAbstractNode>> = Vec::new();
		let mut current: Option<rctree::Node<LayoutAbstractNode>> = None;
		let mut result: Option<rctree::Node<LayoutAbstractNode>> = None;
		
		let mut parsing_root_tag_with_declarations = true;

		for token in parsed {
			match token.unwrap() {
				xmlparser::Token::ElementStart { prefix, local, .. } => {
					let namespace = String::from(prefix.as_str());
					let tag_name = String::from(local.as_str());

					let new_parsed_layout_node = LayoutAbstractNode::new_tag(namespace, tag_name);

					let new_node = rctree::Node::new(new_parsed_layout_node);
					current = Some(new_node);
				}
				xmlparser::Token::Attribute { prefix, local, value, .. } => {
					let colon_prefixed = prefix.start() > 0 && (prefix.start() == prefix.end());
					let name = if colon_prefixed {
						let slice = local.as_str();
						let mut string = String::with_capacity(slice.len() + 1);
						string.push(':');
						string.push_str(slice);
						string
					} else { String::from(local.as_str()) };
					let value = value.as_str();

					let attribute = if parsing_root_tag_with_declarations {
						let parameter_declaration = self.attribute_parser.parse_attribute_declaration(value);
						Attribute::new(name, parameter_declaration)
					}
					else {
						let parameter_types = self.attribute_parser.parse_attribute_types(value);
						Attribute::new(name, parameter_types)
					};

					match &mut current {
						Some(current_node) => {
							match &mut *current_node.borrow_mut() {
								LayoutAbstractNode::Tag(tag) => {
									// Add this attribute to the current node that has not yet reached its closing angle bracket
									tag.add_attribute(attribute);
								}
								LayoutAbstractNode::Text(text) => {
									panic!("Unexpected text attribute {} attemping to be added to tag when parsing XML layout in file: {}", text, path);
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
							parsing_root_tag_with_declarations = false;
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
						let text_node = LayoutAbstractNode::new_text(text_string);
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
}