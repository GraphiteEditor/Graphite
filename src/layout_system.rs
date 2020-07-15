use crate::layout_abstract_syntax::*;
use crate::layout_abstract_types::*;
use crate::layout_attribute_parser::*;
use crate::resource_cache::ResourceCache;
use std::collections::HashSet;
use std::fs;
use std::io;

pub struct LayoutSystem {
	loaded_layouts: ResourceCache<Component>,
	attribute_parser: AttributeParser,
}

impl LayoutSystem {
	pub fn new() -> LayoutSystem {
		Self {
			loaded_layouts: ResourceCache::new(),
			attribute_parser: AttributeParser::new(),
		}
	}

	/// Preload and cache a component by its namespace and name, then recursively explore and repeat for its descendants
	pub fn load_layout_component(&mut self, namespace: &str, name: &str) {
		// Load and parse the XML file's AST for the visited tag
		let xml_path = Self::layout_xml_path(namespace, name);
		let xml_parsed = Self::parse_xml_tree(&self.attribute_parser, &xml_path[..], true, true);
		let mut xml_ast = match xml_parsed {
			Ok(result) => result,
			Err(error) => panic!("Error parsing XML layout syntax: {}", error),
		};

		// Keep track of it being loaded to prevent duplicate work
		let mut already_loaded_layouts = HashSet::new();
		already_loaded_layouts.insert(Self::component_name(namespace, name));

		// Turn the entire XML AST into a component
		let component = Self::component_ast_to_component(&mut xml_ast);
		// Self::print_layout_component(&component);

		// Parse and cache components recursively for all tags referenced within this root component
		self.explore_referenced_components(&xml_ast, &mut already_loaded_layouts);

		// Save the loaded component to the cache
		let component_name = Self::component_name(namespace, name);
		self.loaded_layouts.set(&component_name[..], component);
	}

	/// Preload and cache every XML component file referenced by tags within a recursive traversal of descendants in the given component AST
	fn explore_referenced_components(&mut self, layout_tree_root: &ComponentAst, already_loaded_layouts: &mut HashSet<String>) {
		for child_tag in layout_tree_root.descendants() {
			match &*child_tag.borrow() {
				// Tags are references to other XML layouts that should be loaded and cached
				LayoutAbstractNode::Tag(layout_abstract_tag) => {
					// Cache key in form namespace:name
					let key = Self::component_name(&layout_abstract_tag.namespace[..], &layout_abstract_tag.name[..]);

					if !already_loaded_layouts.contains(&key[..]) {
						// Check if the cache has the loaded component and load it if not
						match self.loaded_layouts.get(&key[..]) {
							// Tag has not been loaded, so load it now
							None => {
								// Load and parse the XML file's AST for the visited tag
								let xml_path = Self::layout_xml_path(&layout_abstract_tag.namespace[..], &layout_abstract_tag.name[..]);
								let mut xml_ast = Self::parse_xml_tree(&self.attribute_parser, &xml_path[..], true, true).unwrap();

								// Keep track of it being loaded to prevent duplicate work
								let key_copy = key.clone();
								already_loaded_layouts.insert(key);

								// Turn the entire XML AST into a component
								let component = Self::component_ast_to_component(&mut xml_ast);

								// Recursively explore the newly loaded AST's tags
								self.explore_referenced_components(&xml_ast, already_loaded_layouts);

								// Save the loaded component to the cache
								self.loaded_layouts.set(&key_copy[..], component);
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

	/// Flatten a full XML component AST into a vector of the immediate children and put the descendants of those nodes into `content` attributes
	fn component_ast_to_component(tree: &mut ComponentAst) -> Component {
		println!("====> Flattening the following component AST to a component\n{:#?}\n", tree);
		let result = tree
			.children()
			.map(|mut child| {
				// Clone the abstract syntax node for this child (excluding the tree)
				let mut cloned_child = child.borrow_mut().clone();

				// If this is a node, stick its descendants into a new `content` attribute
				match &mut cloned_child {
					// Deeply clone the children and attach the tree to a new `content` attribute
					LayoutAbstractNode::Tag(ref mut tag) => {
						let ast_vector_in_tag = child.children().map(|mut c| c.make_deep_copy()).collect::<Vec<_>>();
						let layout_type_value = TypeValueOrArgument::TypeValue(TypeValue::Layout(ast_vector_in_tag));
						let type_value_in_vec = AttributeValue::TypeValue(vec![layout_type_value]);
						let content_attribute = Attribute::new(String::from("content"), type_value_in_vec);
						tag.add_attribute(content_attribute);
					},
					// Text nodes have no children
					LayoutAbstractNode::Text(_) => {},
				}
				cloned_child
			})
			.collect::<Vec<_>>();
		Self::print_layout_component(&result);
		result
	}

	/// Get an AST root node representing a parsed XML component file or XML source code
	pub fn parse_xml_tree(attribute_parser: &AttributeParser, path_or_source: &str, is_path_not_source: bool, component_declaration: bool) -> io::Result<ComponentAst> {
		// XML component file markup source code
		let (path, source) = if is_path_not_source {
			(path_or_source, fs::read_to_string(path_or_source)?)
		}
		else {
			(&"[Inline Attribute XML]"[..], String::from(path_or_source))
		};

		// XML document parser that feeds token-by-token through the file
		let parser = xmlparser::Tokenizer::from(&source[..]);

		// Node stack used to collect descendant nodes while reading deeper into the tree until each reaches its closing tag
		let mut stack: Vec<ComponentAst> = Vec::new();
		// Opening XML tag used to collect the tag name and its various attributes
		let mut current_opening_tag: Option<LayoutAbstractNode> = None;
		// Top-level node that is popped from the stack when the closing tag is reached at the end of the XML document
		let mut final_result: Option<ComponentAst> = None;

		for token_result in parser {
			match token_result {
				Ok(token) => {
					match token {
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
							}
							else {
								String::from(local.as_str())
							};
							// Set the value to an ordinary string slice of the given value
							let value = value.as_str();

							// Attributes on the root element are parameter declarations that list the names and types of permitted variables
							let attribute = if stack.is_empty() && component_declaration {
								let parameter_declaration = attribute_parser.parse_attribute_declaration(value);
								Attribute::new(name, parameter_declaration)
							}
							// Attributes on elements inside the root are arguments to the layout engine (no colon prefix) or the child component (colon prefix)
							else {
								let parameter_types = attribute_parser.parse_attribute_types(value);
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
									panic!(
										"Unexpected text attribute {} attemping to be added to tag when parsing XML layout in component: {}",
										text, path
									);
								},
								// Somehow there is no current opening tag to add this attribute to (probably impossible)
								None => {
									panic!("Error adding attribute to tag when parsing XML layout in component: {}", path);
								},
							}
						},
						// Either the end of the opening tag (...>) or the end of a self-closing tag (.../>) or an entire closing tag (</NAMESPACE:NAME>)
						xmlparser::Token::ElementEnd { end, .. } => {
							match end {
								// After adding any attributes, this element's opening tag ends (...>)
								xmlparser::ElementEnd::Open => {
									// After adding any attributes, we are now a layer deeper in the stack of yet-to-be-closed descendants
									let current_abstract_node = current_opening_tag
										.take()
										.expect(&format!("Invalid syntax when parsing XML layout in component {}", path)[..]);
									let tree_node_with_descendants = rctree::Node::new(current_abstract_node);
									stack.push(tree_node_with_descendants);
								},
								// After adding any attributes, this element's self-closing tag ends (.../>)
								xmlparser::ElementEnd::Empty => {
									// Because a self-closing element does not go deeper, attach this now-complete node directly to its parent
									let parent_node = stack.last_mut().expect(&format!("Invalid syntax when parsing XML layout in component: {}", path)[..]);
									let current_abstract_node = current_opening_tag
										.take()
										.expect(&format!("Invalid syntax when parsing XML layout in component: {}", path)[..]);
									let tree_node = rctree::Node::new(current_abstract_node);
									parent_node.append(tree_node);
								},
								// After visiting any descendants inside the opening tag, finally the closing tag is reached (</NAMESPACE:NAME>)
								xmlparser::ElementEnd::Close(..) => {
									// Pop the element now that descendants have been parsed and we make our way back up the tree one level
									let closed_node_with_descendants = stack
										.pop()
										.expect(&format!("Encountered extra closing tag when parsing XML layout in component: {}", path)[..]);

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
												Some(_) => panic!("Encountered multiple root-level tags when parsing XML layout in component: {}", path),
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
								let parent_node = stack
									.last_mut()
									.expect(&format!("Encountered text outside the root tag when parsing XML layout in component: {}", path)[..]);

								// Construct an AST text node with the provided text
								let abstract_text_node = LayoutAbstractNode::new_text(text_string);
								// Put the AST text node in a new tree node
								let new_tree_node = rctree::Node::new(abstract_text_node);

								// Attach the new text node on the parent in the tree which contains this text
								parent_node.append(new_tree_node);
							}
						},
						_ => {},
					}
				},
				Err(error) => {
					panic!("Failed parsing XML syntax with error: {}", error);
				},
			}
		}

		// Return the final result or throw an error
		match final_result {
			None => panic!("Invalid syntax when parsing XML layout in component: {}", path),
			Some(tree) => Ok(tree),
		}
	}

	/// Get a string in `namespace:name` format (or just `name` for primitives) given a namespace and component name
	fn component_name(namespace: &str, name: &str) -> String {
		if namespace.len() > 0 {
			format!("{}:{}", namespace, name)
		}
		else {
			String::from(name)
		}
	}

	/// Get the XML file path given a namespace and component name
	fn layout_xml_path(namespace: &str, name: &str) -> String {
		if namespace.len() > 0 {
			format!("gui/{}/{}.xml", namespace, name)
		}
		else {
			format!("gui/{}.xml", name)
		}
	}

	/// Print a component AST (for debugging)
	fn print_layout_tree(tree_root: &ComponentAst) {
		for node in tree_root.descendants() {
			println!("Printing Component AST:\n{:#?}\n", node);
		}
	}

	/// Print a component (for debugging)
	fn print_layout_component(component: &Component) {
		for node in component {
			println!("Printing Component:\n{:#?}\n\n", node);
			match node {
				LayoutAbstractNode::Tag(tag) => {
					let content = tag.attributes.iter().find(|a| a.name == "content");
					match content {
						Some(attribute) => match attribute.value {
							AttributeValue::TypeValue(ref type_value) => {
								for type_value_or_argument in type_value {
									match type_value_or_argument {
										TypeValueOrArgument::TypeValue(type_value) => match type_value {
											TypeValue::Layout(layout) => {
												for component_ast in layout {
													Self::print_layout_tree(&component_ast);
												}
											},
											_ => {},
										},
										TypeValueOrArgument::VariableArgument(_) => {},
									}
								}
							},
							AttributeValue::VariableParameter(_) => {},
						},
						None => {},
					}
				},
				LayoutAbstractNode::Text(_) => {},
			}
		}
	}
}
