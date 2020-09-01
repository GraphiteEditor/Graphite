use crate::layout_abstract_syntax::*;
use crate::layout_abstract_types::*;
use crate::layout_attribute_parser::*;
use crate::resource_cache::ResourceCache;
use crate::window_dom::*;
use std::collections::HashSet;
use std::fs;
use std::io;

pub struct LayoutSystem<'a> {
	windows: Vec<WindowDom<'a>>,
	loaded_components: ResourceCache<FlatComponent>,
	attribute_parser: AttributeParser,
}

impl<'a> LayoutSystem<'a> {
	/// Construct the `LayoutSystem` with zero windows, an empty cache of component XML layouts, and an `AttributeParser` with its regex parsers
	pub fn new() -> Self {
		Self {
			windows: vec![],
			loaded_components: ResourceCache::new(),
			attribute_parser: AttributeParser::new(),
		}
	}

	/// Load and construct a new window from a layout component
	pub fn add_window(&'a mut self, name: (&str, &str)) {
		// Preload the component and its dependencies
		self.preload_component(name)
			.expect(&format!("Failure loading layout component '{}'", Self::component_name(name))[..]);

		// Get the now-loaded component
		let window_root_component_name = Self::component_name(name);
		// let window_root_component = self.loaded_components.get(&window_root_component_name[..]).unwrap();
		// println!("FC: {:#?}", window_root_component);

		// Construct the window and save it
		let new_window = WindowDom::new(&window_root_component_name[..], (1920, 1080), &self.loaded_components);
		self.windows.push(new_window);
	}

	/// Preload and cache a component by its namespace and name, then recursively explore and repeat for its descendants
	pub fn preload_component(&mut self, name: (&str, &str)) -> io::Result<()> {
		// Load and parse the XML file's AST for the visited tag
		let xml_path = Self::layout_xml_path(name);
		let mut component = Self::parse_xml_component(&self.attribute_parser, &xml_path[..], true)?;

		// Keep track of it being loaded to prevent duplicate work during the recursive traversal
		let mut already_loaded_layouts = HashSet::new();
		already_loaded_layouts.insert(Self::component_name(name));

		// Parse and cache components recursively for all tags referenced within this root component
		self.explore_component(&mut component, &mut already_loaded_layouts);

		// Save this loaded root-level component to the cache
		let component_name = Self::component_name(name);
		self.loaded_components.set(&component_name[..], component);

		// Success
		Ok(())
	}

	/// Preload and cache every XML component file referenced by tags within a recursive traversal of descendants in the given flat component
	fn explore_component(&mut self, component: &mut FlatComponent, already_loaded_layouts: &mut HashSet<String>) {
		// Go through each direct child in the list that makes up flat component
		for child_tag in &component.child_components {
			self.explore_component_tag(child_tag, already_loaded_layouts);
		}

		// Go through each parameter attribute and preload any default values of layouts
		for definition in &component.own_info.parameters {
			for default in definition.type_sequence_default.iter() {
				if let TypeValue::Layout(layouts) = default {
					for layout in layouts {
						match &*layout.borrow() {
							LayoutComponentNode::Tag(tag) => self.explore_component_tag(tag, already_loaded_layouts),
							LayoutComponentNode::Text(_) => {},
						}
					}
				}
			}
		}
	}

	/// Preload and cache every XML component file referenced by tags within a recursive traversal of descendants in the given component tag
	fn explore_component_tag(&mut self, tag: &LayoutComponentTag, already_loaded_layouts: &mut HashSet<String>) {
		// Determine the cache key of form "namespace:name"
		let (name, namespace) = &tag.name;
		let key = Self::component_name((&name[..], &namespace[..]));

		// Load the new component if it isn't already preloaded
		if !already_loaded_layouts.contains(&key[..]) && self.loaded_components.get(&key[..]).is_none() {
			// Load and parse the component XML file for the visited tag
			let xml_path = Self::layout_xml_path((&name[..], &namespace[..]));
			let mut component = Self::parse_xml_component(&self.attribute_parser, &xml_path[..], true).unwrap();

			// Keep track of it being loaded to prevent duplicate work
			let key_copy = key.clone();
			already_loaded_layouts.insert(key);

			// Recursively explore the newly loaded component
			self.explore_component(&mut component, already_loaded_layouts);

			// Save the loaded component to the cache
			self.loaded_components.set(&key_copy[..], component);
		}

		// Expore the Layout-type user attribute argument values
		for argument in &tag.user_arguments {
			for value in &argument.value {
				if let TypeValueOrArgument::TypeValue(TypeValue::Layout(layouts)) = value {
					for layout in layouts {
						match &*layout.borrow() {
							LayoutComponentNode::Tag(component_tag) => self.explore_component_tag(component_tag, already_loaded_layouts),
							LayoutComponentNode::Text(_) => {},
						}
					}
				}
			}
		}

		// Explore the tree of `content` children
		if let Some(ref content) = tag.content {
			for child_node in content.iter() {
				for descendant in child_node.descendants() {
					match &*descendant.borrow() {
						LayoutComponentNode::Tag(component_tag) => self.explore_component_tag(component_tag, already_loaded_layouts),
						LayoutComponentNode::Text(_) => {},
					}
				}
			}
		}
	}

	/// Parse an XML component all the way into a flat component structure
	pub fn parse_xml_component(attribute_parser: &AttributeParser, path_or_source: &str, is_path_not_source: bool) -> io::Result<FlatComponent> {
		println!("Parsing XML Component: {}", path_or_source);
		let parsed_tree = &mut Self::parse_xml_tree(attribute_parser, path_or_source, is_path_not_source, true)?;
		let flat_tree = Self::flatten_component_tree(parsed_tree);
		Ok(flat_tree)
	}

	/// Parse a fragment of XML layout syntax with a tree of tags (currently only supports a single root node, should eventually implement returning a vector of them)
	pub fn parse_xml_node(attribute_parser: &AttributeParser, path_or_source: &str, is_path_not_source: bool) -> io::Result<NodeTree> {
		let parsed_tree = Self::parse_xml_tree(attribute_parser, path_or_source, is_path_not_source, false)?;
		Ok(Self::node_tree_from_node_or_def_tree(&parsed_tree))
	}

	/// Flatten a full XML component AST into a vector of the immediate children and put the descendants of those nodes into `content` attributes
	fn flatten_component_tree(tree: &mut NodeOrDefTree) -> FlatComponent {
		let own_info = match &*tree.borrow() {
			LayoutComponentNodeOrDefinition::LayoutComponentDefinition(definition) => definition.clone(),
			LayoutComponentNodeOrDefinition::LayoutComponentNode(LayoutComponentNode::Tag(_)) => panic!("Tag node found in place of component definition"),
			LayoutComponentNodeOrDefinition::LayoutComponentNode(LayoutComponentNode::Text(_)) => panic!("Text node found in place of component definition"),
		};

		// Turn all the tag nodes (but not text nodes) into a list of flat child components (with their descendant trees in their `content` attributes)
		let child_components = tree
			// Get the direct children from this tree node
			.children()
			// Clone each child abstract tag node (ignoring text nodes) with each of their descendants added to their `content` attribute variable
			.filter_map(|child_node| {
				// Filter out text nodes because they make no sense as child components
				let mut cloned_tag = match &*child_node.borrow() {
					LayoutComponentNodeOrDefinition::LayoutComponentNode(LayoutComponentNode::Tag(child_tag)) => child_tag.clone(),
					LayoutComponentNodeOrDefinition::LayoutComponentNode(LayoutComponentNode::Text(_)) => return None,
					LayoutComponentNodeOrDefinition::LayoutComponentDefinition(_) => panic!("Component definition found in place of tag node"),
				};

				// Clone the tree for this child as `LayoutComponentNode`s and turn its children into a vector, then set that vector as the content attribute
				let node_within_root = Self::node_tree_from_node_or_def_tree(&child_node);
				let children = node_within_root.children().map(|mut child| {
					// Child must be detached in order to live on its own in the vector, otherwise it will be cleaned up when its (former) parent is dropped
					child.detach();
					child
				}).collect::<Vec<_>>();
				cloned_tag.set_content(children);

				// Return this `LayoutComponentTag` within the component's root definition tag
				Some(cloned_tag)
			})
			.collect::<Vec<_>>();

		// Build and return the resulting flat component made from the cloned data for its `own_info` and `child_components`
		FlatComponent::new(own_info, child_components)
	}

	/// Get an AST root node representing a parsed XML component file or XML source code
	pub fn parse_xml_tree(attribute_parser: &AttributeParser, path_or_source: &str, is_path_not_source: bool, component_declaration: bool) -> io::Result<NodeOrDefTree> {
		// XML component file markup source code
		let (path, source) = if is_path_not_source {
			(path_or_source, fs::read_to_string(path_or_source)?)
		}
		else {
			("[Inline Attribute XML]", String::from(path_or_source))
		};

		// XML document parser that feeds token-by-token through the file
		let parser = xmlparser::Tokenizer::from(&source[..]);

		// Node stack used to collect descendant nodes while reading deeper into the tree until each reaches its closing tag
		let mut stack: Vec<NodeOrDefTree> = Vec::new();
		// Opening XML tag used to collect the tag name and its various attributes
		let mut current_opening_tag: Option<LayoutComponentNodeOrDefinition> = None;
		// Top-level node that is popped from the stack when the closing tag is reached at the end of the XML document
		let mut final_result: Option<NodeOrDefTree> = None;

		for token_result in parser {
			let token = token_result.expect(&format!("Invalid syntax when parsing XML layout in component: {}", path)[..]);
			match token {
				// Beginning of an opening tag (<NAMESPACE:NAME ...)
				xmlparser::Token::ElementStart { prefix, local, .. } => {
					// Get the supplied namespace and tag name as owned strings
					let name = (String::from(prefix.as_str()), String::from(local.as_str()));

					// If this is the root element and we're parsing a component file, the root tag is the component definition
					if stack.is_empty() && component_declaration {
						// Construct and store the component definition while attributes are added until its opening tag ends
						let definition = LayoutComponentDefinition::new(name);
						current_opening_tag = Some(LayoutComponentNodeOrDefinition::LayoutComponentDefinition(definition));
					}
					// Otherwise, we're parsing a node inside the root or at the root of a fragment of XML layout syntax
					else {
						// Construct and store the component node while attributes are added until the opening (or self-closing) tag ends
						let tag_node = LayoutComponentNode::new_tag(name);
						current_opening_tag = Some(LayoutComponentNodeOrDefinition::LayoutComponentNode(tag_node));
					}
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

					// Add the new attribute to the current yet-to-be-closed element
					match &mut current_opening_tag {
						// Add this attribute as a parameter to the current root-level component definition tag
						Some(LayoutComponentNodeOrDefinition::LayoutComponentDefinition(definition)) => {
							let parsed_parameter = attribute_parser.parse_attribute_parameter_declaration(value);
							definition.add_parameter(parsed_parameter);
						},
						// Add this attribute as an argument to the current tag
						Some(LayoutComponentNodeOrDefinition::LayoutComponentNode(LayoutComponentNode::Tag(tag))) => {
							let parsed_attributes = attribute_parser.parse_attribute_argument_types(value);
							let attribute_argument = AttributeArg::new(name, parsed_attributes);
							tag.add_attribute(attribute_argument);
						},
						// It should be impossible to add an attribute when there is no opening tag in progress
						_ => unreachable!(),
					}
				},
				// Either the end of the opening tag (...>) or the end of a self-closing tag (.../>) or an entire closing tag (</NAMESPACE:NAME>)
				xmlparser::Token::ElementEnd { end, .. } => {
					match end {
						// After adding any attributes, this element's opening tag ends (...>)
						xmlparser::ElementEnd::Open => {
							// After adding any attributes, we are now a layer deeper in the stack of yet-to-be-closed descendants
							let complete_opening_tag = current_opening_tag.take().unwrap();
							let tree_node = rctree::Node::new(complete_opening_tag);
							stack.push(tree_node);
						},
						// After adding any attributes, this element's self-closing tag ends (.../>)
						xmlparser::ElementEnd::Empty => {
							// Because a self-closing element does not go deeper, attach this now-complete node directly to its parent
							let parent_node = stack.last_mut().expect(&format!("Invalid syntax when parsing XML layout in component: {}", path)[..]);
							let complete_self_closing_tag = current_opening_tag.take().unwrap();
							let tree_node = rctree::Node::new(complete_self_closing_tag);
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

						// Construct a text node with the provided text
						let text_template_sequence = attribute_parser.parse_text_template_sequence(&text_string[..]);
						let abstract_text_node = LayoutComponentNodeOrDefinition::LayoutComponentNode(LayoutComponentNode::new_text(text_template_sequence));
						// Put the text node in a new tree node
						let tree_node = rctree::Node::new(abstract_text_node);

						// Attach the new text node on the parent in the tree which contains this text
						parent_node.append(tree_node);
					}
				},
				_ => {},
			}
		}

		// Return the final result or throw an error
		match final_result {
			None => panic!("Invalid syntax when parsing XML layout in component: {}", path),
			Some(tree) => Ok(tree),
		}
	}

	/// Get a string in `namespace:name` format (or just `name` for primitives) given a namespace and component name
	pub fn component_name(name: (&str, &str)) -> String {
		let (namespace, file) = name;
		if namespace.len() > 0 {
			format!("{}:{}", namespace, file)
		}
		else {
			String::from(file)
		}
	}

	/// Get the XML file path given a namespace and component name
	fn layout_xml_path(name: (&str, &str)) -> String {
		let (namespace, file) = name;
		if namespace.len() > 0 {
			format!("gui/{}/{}.xml", namespace, file)
		}
		else {
			format!("gui/{}.xml", file)
		}
	}

	/// Convert every element in the tree of `LayoutComponentNodeOrDefinition` wrapper enums into unwrapped `LayoutComponentNode` structs
	fn node_tree_from_node_or_def_tree(layout_component_node_or_definition: &NodeOrDefTree) -> NodeTree {
		// Unwrap the `LayoutComponentNode` from the root element's value
		let cloned_node_data = match &*layout_component_node_or_definition.borrow() {
			LayoutComponentNodeOrDefinition::LayoutComponentNode(node) => node.clone(),
			LayoutComponentNodeOrDefinition::LayoutComponentDefinition(_) => panic!("Found an unexpected component definition while expecting a node"),
		};

		// Build a new tree of the correct type with the unwrapped data as its root value
		let mut tree_result = rctree::Node::new(cloned_node_data);

		// Go through all the direct children of the old tree and append the new recursively converted trees to match the shape of the old tree
		for tree_node in layout_component_node_or_definition.children() {
			tree_result.append(Self::node_tree_from_node_or_def_tree(&tree_node));
		}

		tree_result
	}
}
