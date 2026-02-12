use crate::utility::*;
use convert_case::{Case, Casing};
use indoc::formatdoc;
use std::io::Write;

pub fn write_catalog_index_page(categories: &[String]) {
	if std::path::Path::new(NODE_CATALOG_PATH).exists() {
		std::fs::remove_dir_all(NODE_CATALOG_PATH).expect("Failed to remove existing node catalog directory");
	}
	std::fs::create_dir_all(NODE_CATALOG_PATH).expect("Failed to create node catalog directory");
	let page_path = format!("{NODE_CATALOG_PATH}/_index.md");
	let mut page = std::fs::File::create(&page_path).expect("Failed to create index file");

	write_frontmatter(&mut page);
	write_description(&mut page);
	write_categories_table_header(&mut page);
	write_categories_table_rows(&mut page, categories);
}

fn write_frontmatter(page: &mut std::fs::File) {
	let content = formatdoc!(
		"
		+++
		title = \"Node catalog\"
		template = \"book.html\"
		page_template = \"book.html\"

		[extra]
		order = 3
		css = [\"/page/user-manual/node-catalog.css\"]
		+++
		"
	);
	page.write_all(content.as_bytes()).expect("Failed to write to index file");
}

fn write_description(page: &mut std::fs::File) {
	let content = formatdoc!(
		"

		The node catalog documents all of the nodes available in Graphite's node graph system, organized by category.

		<p><img src=\"https://static.graphite.art/content/learn/node-catalog/node-terminology.avif\" onerror=\"this.onerror = null; this.src = this.src.replace('.avif', '.png')\" alt=\"Terminology diagram covering how the node system operates\" /></p>
		"
	);
	page.write_all(content.as_bytes()).expect("Failed to write to index file");
}

fn write_categories_table_header(page: &mut std::fs::File) {
	let content = formatdoc!(
		"
	
		## Node categories

		| Category | Details |
		|:-|:-|
		"
	);
	page.write_all(content.as_bytes()).expect("Failed to write to index file");
}

fn write_categories_table_rows(page: &mut std::fs::File, categories: &[String]) {
	let content = categories
		.iter()
		.filter_map(|c| if c.is_empty() { if OMIT_HIDDEN { None } else { Some("Hidden") } } else { Some(c) })
		.map(|category| {
			let category_path_part = sanitize_path(&category.to_case(Case::Kebab));
			let details = category_description(category).replace("\n\n", "</p><p>").replace('\n', "<br />");
			format!("| [{category}](./{category_path_part}) | <p>{details}</p> |")
		})
		.collect::<Vec<_>>()
		.join("\n");
	page.write_all(content.as_bytes()).expect("Failed to write to index file");
}
