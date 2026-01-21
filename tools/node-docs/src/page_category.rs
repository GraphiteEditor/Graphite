use crate::utility::*;
use convert_case::{Case, Casing};
use graph_craft::concrete;
use graph_craft::proto::NodeMetadata;
use graphene_std::core_types;
use indoc::formatdoc;
use std::collections::HashSet;
use std::io::Write;

pub fn write_category_index_page(index: usize, category: &str, nodes: &[(&core_types::ProtoNodeIdentifier, &NodeMetadata)], category_path: &String) {
	std::fs::create_dir_all(category_path).expect("Failed to create category directory");
	let page_path = format!("{category_path}/_index.md");
	let mut page = std::fs::File::create(&page_path).expect("Failed to create index file");

	write_frontmatter(&mut page, category, index + 1);
	write_description(&mut page, category);
	write_nodes_table_header(&mut page);
	write_nodes_table_rows(&mut page, nodes);
}

fn write_frontmatter(page: &mut std::fs::File, category: &str, order: usize) {
	let content = formatdoc!(
		"
		+++
		title = \"{category}\"
		template = \"book.html\"
		page_template = \"book.html\"
		
		[extra]
		order = {order}
		css = [\"/page/user-manual/node-category.css\"]
		+++
		"
	);
	page.write_all(content.as_bytes()).expect("Failed to write to index file");
}

fn write_description(page: &mut std::fs::File, category: &str) {
	let category_description = category_description(category);
	let content = formatdoc!(
		"

		{category_description}
		"
	);
	page.write_all(content.as_bytes()).expect("Failed to write to index file");
}

fn write_nodes_table_header(page: &mut std::fs::File) {
	let content = formatdoc!(
		"

		## Nodes

		| Node | Details | Possible Types |
		|:-|:-|:-|
		"
	);
	page.write_all(content.as_bytes()).expect("Failed to write to index file");
}

fn write_nodes_table_rows(page: &mut std::fs::File, nodes: &[(&core_types::ProtoNodeIdentifier, &NodeMetadata)]) {
	let content = nodes
		.iter()
		.filter_map(|&(id, metadata)| {
			// Path to page
			let name_url_part = sanitize_path(&metadata.display_name.to_case(Case::Kebab));

			// Name and description
			let name = metadata.display_name;
			let description = node_description(metadata);
			let details = description.split('\n').map(|line| format!("<p>{}</p>", line.trim())).collect::<Vec<_>>().join("");

			// Possible types
			let node_registry = core_types::registry::NODE_REGISTRY.lock().unwrap();
			let implementations = node_registry.get(id)?;
			let valid_primary_inputs_to_outputs = implementations
				.iter()
				.map(|(_, node_io)| {
					let input = node_io
						.inputs
						.first()
						.map(|ty| ty.nested_type())
						.filter(|&ty| ty != &concrete!(()))
						.map(ToString::to_string)
						.unwrap_or_default();
					let output = node_io.return_value.nested_type().to_string();
					format!("`{input} → {output}`")
				})
				.collect::<Vec<_>>();
			let valid_primary_inputs_to_outputs = {
				// Dedupe while preserving order
				let mut found = HashSet::new();
				valid_primary_inputs_to_outputs.into_iter().filter(|s| found.insert(s.clone())).collect::<Vec<_>>()
			};
			let possible_types = valid_primary_inputs_to_outputs.join("<br />");

			// Add table row
			Some(format!("| [{name}]({name_url_part}) | {details} | {possible_types} |"))
		})
		.collect::<Vec<_>>()
		.join("\n");
	page.write_all(content.as_bytes()).expect("Failed to write to index file");
}
