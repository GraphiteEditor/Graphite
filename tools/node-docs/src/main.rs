mod page_catalog;
mod page_category;
mod page_node;
mod utility;

use crate::utility::*;
use convert_case::{Case, Casing};
use std::collections::HashMap;

fn main() {
	// TODO: Also obtain document nodes, not only proto nodes
	let nodes = graphene_std::registry::NODE_METADATA.lock().unwrap();

	// Group nodes by category
	let mut nodes_by_category: HashMap<_, Vec<_>> = HashMap::new();
	for (id, metadata) in nodes.iter() {
		nodes_by_category.entry(metadata.category.to_string()).or_default().push((id, metadata));
	}

	// Sort the categories
	let mut categories = nodes_by_category.keys().cloned().collect::<Vec<_>>();
	categories.sort();

	// Create _index.md for the node catalog page
	page_catalog::write_catalog_index_page(&categories);

	// Create node category pages and individual node pages
	for (index, category) in categories.iter().map(|c| if !OMIT_HIDDEN && c.is_empty() { "Hidden" } else { c }).filter(|c| !c.is_empty()).enumerate() {
		// Get nodes in this category
		let mut nodes = nodes_by_category.remove(if !OMIT_HIDDEN && category == "Hidden" { "" } else { category }).unwrap();
		nodes.sort_by_key(|(_, metadata)| metadata.display_name.to_string());

		// Create _index.md file for category
		let category_path_part = sanitize_path(&category.to_case(Case::Kebab));
		let category_path = format!("{NODE_CATALOG_PATH}/{category_path_part}");
		page_category::write_category_index_page(index, category, &nodes, &category_path);

		// Create individual node pages
		for (index, (id, metadata)) in nodes.into_iter().enumerate() {
			page_node::write_node_page(index, id, metadata, &category_path);
		}
	}
}
