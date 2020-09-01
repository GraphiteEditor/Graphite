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
		parameters: &Vec<AttributeArg>,
		loaded_components: &'a ResourceCache<FlatComponent>,
	) -> rctree::Node<DomNode> {
		// Instantiate the DOM node and put it in a tree node
		let component = loaded_components.get(root_component).unwrap();
		let dom_node = DomNode::from_component(component, layout_attributes, parameters);
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
				Self::build_dom_from_component(&component_name[..], &child.layout_arguments, &child.user_arguments, loaded_components)
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
	pub variable_bindings: HashMap<String, Vec<TypeValueOrArgument>>,
}

impl DomNode {
	pub fn new(cache_name: String, layout_attributes: LayoutAttributes, variable_bindings: HashMap<String, Vec<TypeValueOrArgument>>) -> Self {
		Self {
			cache_name,
			layout_attributes,
			variable_bindings,
		}
	}

	pub fn from_component(component: &FlatComponent, layout_attributes: &LayoutAttributes, parameters: &Vec<AttributeArg>) -> Self {
		// Cached name of the loaded component
		let (namespace, name) = &component.own_info.name;
		let cache_name = LayoutSystem::component_name((&namespace[..], &name[..]));

		// Every VARIABLE_NAME binding defined as a parameter on this component
		let mut variable_bindings = component
			.own_info
			.parameters
			.iter()
			.map(|parameter| {
				(
					// HashMap key is the parameter name
					parameter.name.clone(),
					// HashMap value is the parameter's defined default value
					parameter
						.type_sequence_default
						.iter()
						.map(|value| TypeValueOrArgument::TypeValue(value.clone()))
						.collect::<Vec<_>>(),
				)
			})
			.collect::<HashMap<_, _>>();
		// Overwrite the defaults for given parameters
		for parameter in parameters {
			if !variable_bindings.contains_key(&parameter.name[..]) {
				panic!("Invalid argument {} given to the {} component", parameter.name, cache_name);
			}

			variable_bindings.insert(parameter.name.clone(), parameter.value.clone());
		}

		Self::new(cache_name, layout_attributes.clone(), variable_bindings)
	}
}
