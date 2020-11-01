use crate::layout_abstract_syntax::*;
use crate::layout_abstract_types::*;
use crate::{layout_system::*, resource_cache::ResourceCache};
use std::collections::HashMap;

pub struct WindowDom<'a> {
	pub dom: rctree::Node<DomNode>,
	loaded_components: &'a ResourceCache<FlatComponent>,
}

impl<'a> WindowDom<'a> {
	pub fn new(root_component: &str, window_size: (u32, u32), loaded_components: &'a ResourceCache<FlatComponent>) -> WindowDom<'a> {
		let mut layout_attributes = LayoutAttributes::default();
		layout_attributes.width = Dimension::AbsolutePx(window_size.0 as f64);
		layout_attributes.height = Dimension::AbsolutePx(window_size.1 as f64);

		let dom = Self::build_dom_from_component(root_component, &layout_attributes, &vec![], loaded_components);
		Self { dom, loaded_components }
	}

	fn build_dom_from_component(
		root_component: &str,
		layout_attributes: &LayoutAttributes,
		props: &Vec<Prop>,
		loaded_components: &'a ResourceCache<FlatComponent>,
	) -> rctree::Node<DomNode> {
		// Instantiate the DOM node and put it in a tree node
		let component = loaded_components.get(root_component).unwrap();
		let dom_node = DomNode::from_component(component, layout_attributes, props);
		let mut tree = rctree::Node::new(dom_node);

		// Recursively build the child `DomNode` tree node instances
		let child_nodes = component
			.child_components
			.iter()
			.map(|child| {
				// Get the child name used as the component cache key
				let (namespace, name) = &child.name;
				let component_name = LayoutSystem::component_name((namespace, name));

				// Recursively build the child `DomNode` component instance
				Self::build_dom_from_component(&component_name[..], &child.layout, &child.props, loaded_components)
			})
			.collect::<Vec<_>>();

		// Append each child `DomNode` tree node
		for child in child_nodes {
			tree.append(child);
		}

		// Return the tree that has been recursively built with sibling and child components
		tree
	}
}

pub struct DomNode {
	pub cache_name: String,
	pub layout_attributes: LayoutAttributes,
	pub variable_bindings: HashMap<String, Vec<TypedValueOrVariableName>>,
}

impl DomNode {
	pub fn new(cache_name: String, layout_attributes: LayoutAttributes, variable_bindings: HashMap<String, Vec<TypedValueOrVariableName>>) -> Self {
		Self {
			cache_name,
			layout_attributes,
			variable_bindings,
		}
	}

	pub fn from_component(component: &FlatComponent, layout_attributes: &LayoutAttributes, props: &Vec<Prop>) -> Self {
		// Cached name of the loaded component
		let (namespace, name) = &component.own_info.name;
		let cache_name = LayoutSystem::component_name((&namespace[..], &name[..]));

		// Every VARIABLE_NAME binding defined in the prop definitions on this component
		let mut variable_bindings = component
			.own_info
			.prop_definitions
			.iter()
			.map(|prop_definition| {
				(
					// HashMap key is the prop name
					prop_definition.variable_name.clone(),
					// HashMap value is the prop definition's default value
					prop_definition
						.type_sequence_default
						.iter()
						.map(|value| TypedValueOrVariableName::TypedValue(value.clone()))
						.collect::<Vec<_>>(),
				)
			})
			.collect::<HashMap<_, _>>();
		// Overwrite the default values for the provided props
		for prop in props {
			if !variable_bindings.contains_key(&prop.name[..]) {
				panic!("Invalid argument {} given to the {} component", prop.name, cache_name);
			}

			variable_bindings.insert(prop.name.clone(), prop.value_sequence.clone());
		}

		Self::new(cache_name, layout_attributes.clone(), variable_bindings)
	}
}
