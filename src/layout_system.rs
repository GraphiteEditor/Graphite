use std::fs;
use std::io;
use std::collections::HashSet;
use crate::layout_abstract_syntax::*;
use crate::layout_attribute_parser::*;
use crate::resource_cache::ResourceCache;

pub struct LayoutSystem {
	// pub dom_tree: rctree::Node<
	pub loaded_layouts: ResourceCache<rctree::Node<LayoutAbstractNode>>,
	attribute_parser: AttributeParser,
}

impl LayoutSystem {
	pub fn new() -> LayoutSystem {
		Self {
			loaded_layouts: ResourceCache::new(),
			attribute_parser: AttributeParser::new(),
		}
	}

	pub fn load_layout(&mut self, namespace: &str, name: &str) {
		// Load and parse the requested XML layout
		let xml_path = self.layout_xml_path(namespace, name);
		let window_main = self.parse_xml_file(&xml_path[..]).unwrap();

		Self::print_layout_tree(&window_main);

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

								// Save the loaded layout to the cache
								self.loaded_layouts.set(&key_copy[..], new_loaded_layout);
							},
							// Tag has already been loaded
							Some(_) => {},
						}
					}
				},
				// Text nodes don't need to be loaded
				LayoutAbstractNode::Text(_) => {},
			};
		}
	}

	// Get the "namespace:name" format of string given a namespace and layout name
	fn layout_name(&self, namespace: &str, name: &str) -> String {
		if namespace.len() > 0 {
			format!("{}:{}", namespace, name)
		}
		else {
			String::from(name)
		}
	}

	// Get the XML file path given a namespace and layout name
	fn layout_xml_path(&self, namespace: &str, name: &str) -> String {
		if namespace.len() > 0 {
			format!("gui/{}/{}.xml", namespace, name)
		}
		else {
			format!("gui/{}.xml", name)
		}
	}

	// Get an abstract syntax tree root node representing a parsed XML layout file
	fn parse_xml_file(&self, path: &str) -> io::Result<rctree::Node<LayoutAbstractNode>> {
		// XML layout file markup source code
		let source = fs::read_to_string(path)?;
		// XML document parser that feeds token-by-token through the file
		let parser = xmlparser::Tokenizer::from(&source[..]);

		// Node stack used to collect descendant nodes while reading deeper into the tree until each reaches its closing tag
		let mut stack: Vec<rctree::Node<LayoutAbstractNode>> = Vec::new();
		// Opening XML tag used to collect the tag name and its various attributes
		let mut current_opening_tag: Option<LayoutAbstractNode> = None;
		// Top-level node that is popped from the stack when the closing tag is reached at the end of the XML document
		let mut final_result: Option<rctree::Node<LayoutAbstractNode>> = None;
		
		for token in parser {
			match token.unwrap() {
				// Beginning of an opening tag (<NAMESPACE:NAME ...)
				xmlparser::Token::ElementStart { prefix, local, .. } => {
					// Get the supplied namespace and tag name as owned strings
					let namespace = String::from(prefix.as_str());
					let tag_name = String::from(local.as_str());

					// Construct an AST tag node with the namespace and tag name
					let abstract_tag_node = LayoutAbstractNode::new_tag(namespace, tag_name);

					// Store the AST node while attributes are added until the opening (or self-closing) tag ends
					current_opening_tag = Some(abstract_tag_node);
				},
				// Any attributes within the current opening tag (... ATTRIBUTE="VALUE" ...)
				xmlparser::Token::Attribute { prefix, local, value, .. } => {
					// Check if the attribute has an empty prefix (thus, only a colon)
					let colon_prefixed = prefix.start() > 0 && (prefix.start() == prefix.end());
					// Set the name to the given name, possibly with a prepended colon
					let name = if colon_prefixed {
						let slice = local.as_str();
						let mut string = String::with_capacity(slice.len() + 1);
						string.push(':');
						string.push_str(slice);
						string
					} else {
						String::from(local.as_str())
					};
					// Set the value to an ordinary string slice of the given value
					let value = value.as_str();

					// Attributes on the root element are parameter declarations that list the names and types of permitted variables
					let attribute = if stack.is_empty() {
						let parameter_declaration = self.attribute_parser.parse_attribute_declaration(value);
						Attribute::new(name, parameter_declaration)
					}
					// Attributes on elements inside the root are arguments to the layout engine (no colon prefix) or the child layout (colon prefix)
					else {
						let parameter_types = self.attribute_parser.parse_attribute_types(value);
						Attribute::new(name, parameter_types)
					};

					// Add the new attribute to the current yet-to-be-closed element
					match &mut current_opening_tag {
						// The opening tag is indeed a tag AST node
						Some(LayoutAbstractNode::Tag(tag)) => {
							tag.add_attribute(attribute);
						},
						// Somehow the current opening tag is actually a text node (probably impossible)
						Some(LayoutAbstractNode::Text(text)) => {
							panic!("Unexpected text attribute {} attemping to be added to tag when parsing XML layout in file: {}", text, path);
						},
						// Somehow there is no current opening tag to add this attribute to (probably impossible)
						None => {
							panic!("Error adding attribute to tag when parsing XML layout in file: {}", path);
						},
					}
				},
				// Either the end of the opening tag (...>) or the end of a self-closing tag (.../>) or an entire closing tag (</NAMESPACE:NAME>)
				xmlparser::Token::ElementEnd { end, .. } => {
					match end {
						// After adding any attributes, this element's opening tag ends (...>)
						xmlparser::ElementEnd::Open => {
							// After adding any attributes, we are now a layer deeper in the stack of yet-to-be-closed descendants
							let current_abstract_node = current_opening_tag.take().expect(&format!("Invalid syntax when parsing XML layout in file {}", path)[..]);
							let tree_node_with_descendants = rctree::Node::new(current_abstract_node);
							stack.push(tree_node_with_descendants);
						},
						// After adding any attributes, this element's self-closing tag ends (.../>)
						xmlparser::ElementEnd::Empty => {
							// Because a self-closing element does not go deeper, attach this now-complete node directly to its parent
							let parent_node = stack.last_mut().expect(&format!("Invalid syntax when parsing XML layout in file: {}", path)[..]);
							let current_abstract_node = current_opening_tag.take().expect(&format!("Invalid syntax when parsing XML layout in file: {}", path)[..]);
							let tree_node = rctree::Node::new(current_abstract_node);
							parent_node.append(tree_node);
						},
						// After visiting any descendants inside the opening tag, finally the closing tag is reached (</NAMESPACE:NAME>)
						xmlparser::ElementEnd::Close(..) => {
							// Pop the element now that descendants have been parsed and we make our way back up the tree one level
							let closed_node_with_descendants = stack.pop().expect(&format!("Encountered extra closing tag when parsing XML layout in file: {}", path)[..]);

							// Append this now-complete node to its parent, unless there is no parent, in which case we save this root node as the final result
							match stack.last_mut() {
								// If a parent node exists
								Some(parent_node) => {
									parent_node.append(closed_node_with_descendants);
								},
								// If this is the root node
								None => {
									match final_result {
										// Save the root element as the final result
										None => final_result = Some(closed_node_with_descendants),
										// There can only be one root element in the XML document, but this isn't the first one encountered
										Some(_) => panic!("Encountered multiple root-level tags when parsing XML layout in file: {}", path),
									}
								},
							}
						},
					}
				},
				// A text node in the space between sibling elements (... SOME TEXT ...)
				xmlparser::Token::Text { text } => {
					// Trim any whitespace from around the string
					let text_string = String::from(text.as_str().trim());
					
					// If the string isn't all whitespace, append a new text node to the parent
					if !text_string.is_empty() {
						// Get the tree node which contains this text
						let parent_node = stack.last_mut().expect(&format!("Encountered text outside the root tag when parsing XML layout in file: {}", path)[..]);

						// Construct an AST text node with the provided text
						let abstract_text_node = LayoutAbstractNode::new_text(text_string);
						// Put the AST text node in a new tree node
						let new_tree_node = rctree::Node::new(abstract_text_node);

						// Attach the new text node on the parent in the tree which contains this text
						parent_node.append(new_tree_node);
					}
				},
				_ => {}
			}
		}
		
		match final_result {
			None => panic!("Invalid syntax when parsing XML layout in file: {}", path),
			Some(tree) => Ok(tree),
		}
	}

	fn print_layout_tree(tree_root: &rctree::Node<LayoutAbstractNode>) {
		for node in tree_root.descendants() {
			println!("{:?}", node);
		}
	}
}