use convert_case::{Case, Casing};
use graph_craft::concrete;
use graph_craft::document::value;
use graph_craft::proto::{NodeMetadata, RegistryValueSource};
use graphene_std::{ContextDependencies, core_types};
use indoc::{formatdoc, indoc};
use std::collections::{HashMap, HashSet};
use std::io::Write;

const NODE_CATALOG_PATH: &str = "../../website/content/learn/node-catalog";
const OMIT_HIDDEN: bool = true;

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
	write_catalog_index_page(&categories);

	// Create node category pages and individual node pages
	for (index, category) in categories.iter().map(|c| if !OMIT_HIDDEN && c.is_empty() { "Hidden" } else { c }).filter(|c| !c.is_empty()).enumerate() {
		// Get nodes in this category
		let mut nodes = nodes_by_category.remove(if !OMIT_HIDDEN && category == "Hidden" { "" } else { category }).unwrap();
		nodes.sort_by_key(|(_, metadata)| metadata.display_name.to_string());

		// Create _index.md file for category
		let category_path_part = sanitize_path(&category.to_case(Case::Kebab));
		let category_path = format!("{NODE_CATALOG_PATH}/{category_path_part}");
		write_category_index_page(index, category, &nodes, &category_path);

		// Create individual node pages
		for (index, (id, metadata)) in nodes.into_iter().enumerate() {
			write_node_page(index, id, metadata, &category_path);
		}
	}
}

fn write_node_page(index: usize, id: &core_types::ProtoNodeIdentifier, metadata: &NodeMetadata, category_path: &String) {
	let node_registry = core_types::registry::NODE_REGISTRY.lock().unwrap();
	let Some(implementations) = node_registry.get(id) else { return };

	// Path to page
	let name_url_part = sanitize_path(&metadata.display_name.to_case(Case::Kebab));
	let page_path = format!("{category_path}/{name_url_part}.md");
	let mut page = std::fs::File::create(&page_path).expect("Failed to create node page file");

	// Context features
	let context_features = &metadata.context_features;
	let context_dependencies: ContextDependencies = context_features.as_slice().into();

	// Input types
	let mut valid_input_types = vec![Vec::new(); metadata.fields.len()];
	for (_, node_io) in implementations.iter() {
		for (i, ty) in node_io.inputs.iter().enumerate() {
			valid_input_types[i].push(ty.clone());
		}
	}
	for item in valid_input_types.iter_mut() {
		// Dedupe while preserving order
		let mut found = HashSet::new();
		*item = item.clone().into_iter().filter(|s| found.insert(s.clone())).collect::<Vec<_>>()
	}

	// Primary output types
	let valid_primary_outputs = implementations.iter().map(|(_, node_io)| node_io.return_value.nested_type().clone()).collect::<Vec<_>>();
	let valid_primary_outputs = {
		// Dedupe while preserving order
		let mut found = HashSet::new();
		valid_primary_outputs.into_iter().filter(|s| found.insert(s.clone())).collect::<Vec<_>>()
	};
	let valid_primary_outputs = valid_primary_outputs.iter().map(|ty| format!("`{ty}`")).collect::<Vec<_>>();
	let valid_primary_outputs = {
		// Dedupe while preserving order
		let mut found = HashSet::new();
		valid_primary_outputs.into_iter().filter(|s| found.insert(s.clone())).collect::<Vec<_>>()
	};
	let valid_primary_outputs = valid_primary_outputs.join("<br />");

	// Write sections to the file
	write_node_frontmatter(&mut page, metadata, index + 1);
	write_node_description(&mut page, metadata);
	write_node_interface_header(&mut page);
	node_write_context(&mut page, context_dependencies);
	node_write_inputs(&mut page, valid_input_types, metadata);
	node_write_outputs(page, valid_primary_outputs);
}

fn write_node_frontmatter(page: &mut std::fs::File, metadata: &NodeMetadata, order: usize) {
	let name = metadata.display_name;

	let content = formatdoc!(
		"
		+++
		title = \"{name}\"

		[extra]
		order = {order}
		css = [\"/page/user-manual/node.css\"]
		+++
		"
	);
	page.write_all(content.as_bytes()).expect("Failed to write to node page file");
}

fn write_node_description(page: &mut std::fs::File, metadata: &NodeMetadata) {
	let description = node_description(metadata);

	let content = formatdoc!(
		"

		{description}
		"
	);
	page.write_all(content.as_bytes()).expect("Failed to write to node page file");
}

fn write_node_interface_header(page: &mut std::fs::File) {
	let content = formatdoc!(
		"

		## Interface
		"
	);
	page.write_all(content.as_bytes()).expect("Failed to write to node page file");
}

fn node_write_context(page: &mut std::fs::File, context_dependencies: ContextDependencies) {
	let extract = context_dependencies.extract;
	let inject = context_dependencies.inject;
	if !extract.is_empty() || !inject.is_empty() {
		let mut context_features = "| | |\n|:-|:-|".to_string();
		if !extract.is_empty() {
			let names = extract.iter().map(|ty| format!("`{}`", ty.name())).collect::<Vec<_>>().join("<br />");
			context_features.push_str(&format!("\n| **Reads** | {names} |"));
		}
		if !inject.is_empty() {
			let names = inject.iter().map(|ty| format!("`{}`", ty.name())).collect::<Vec<_>>().join("<br />");
			context_features.push_str(&format!("\n| **Sets** | {names} |"));
		}

		let content = formatdoc!(
			"

			### Context

			{context_features}
			"
		);
		page.write_all(content.as_bytes()).expect("Failed to write to node page file");
	};
}

fn node_write_inputs(page: &mut std::fs::File, valid_input_types: Vec<Vec<core_types::Type>>, metadata: &NodeMetadata) {
	let rows = metadata
		.fields
		.iter()
		.enumerate()
		.filter(|&(index, field)| !field.hidden || index == 0)
		.map(|(index, field)| {
			// Parameter
			let parameter = field.name;

			// Possible types
			let mut possible_types_list = valid_input_types.get(index).unwrap_or(&Vec::new()).iter().map(|ty| ty.nested_type()).cloned().collect::<Vec<_>>();
			possible_types_list.sort_by_key(|ty| ty.to_string());
			possible_types_list.dedup();
			let mut possible_types = possible_types_list.iter().map(|ty| format!("`{ty}`")).collect::<Vec<_>>().join("<br />");
			if possible_types.is_empty() {
				possible_types = "*Any Type*".to_string();
			}

			// Details
			let mut details = field
				.description
				.trim()
				.split('\n')
				.filter(|line| !line.is_empty())
				.map(|line| format!("<p>{}</p>", line.trim()))
				.collect::<Vec<_>>();
			if index == 0 {
				details.push("<p>*Primary Input*</p>".to_string());
			}
			if field.exposed {
				details.push("<p>*Exposed to the Graph by Default*</p>".to_string());
			}
			if let RegistryValueSource::Scope(scope_name) = &field.value_source {
				details.push(format!("<p>*Sourced From Scope: `{scope_name}`*</p>"));
			}
			let default_value = match field.value_source {
				RegistryValueSource::Default(default_value) => Some(default_value.to_string().replace(" :: ", "::")),
				_ => field
					.default_type
					.as_ref()
					.or(match possible_types_list.as_slice() {
						[single] => Some(single),
						_ => None,
					})
					.and_then(|ty| value::TaggedValue::from_type(ty.nested_type()))
					.map(|ty| ty.to_debug_string()),
			};
			if index > 0
				&& !field.exposed
				&& let Some(default_value) = default_value
			{
				let default_value = default_value.trim_end_matches('.').trim_end_matches(".0"); // Display whole-number floats as integers

				let render_color = |color| format!(r#"<span style="padding-right: 100px; border: 2px solid var(--color-fog); background: {color}"></span>"#);
				let default_value = match default_value {
					"Color::BLACK" => render_color("black"),
					"GradientStops([(0.0, Color { red: 0.0, green: 0.0, blue: 0.0, alpha: 1.0 }), (1.0, Color { red: 1.0, green: 1.0, blue: 1.0, alpha: 1.0 })])" => {
						render_color("linear-gradient(to right, black, white)")
					}
					_ => format!("`{default_value}{}`", field.unit.unwrap_or_default()),
				};

				details.push(format!("<p>*Default:*&nbsp;{default_value}</p>"));
			}
			let details = details.join("");

			if index == 0 && possible_types_list.as_slice() == [concrete!(())] {
				"| - | *No Primary Input* | - |".to_string()
			} else {
				format!("| {parameter} | {details} | {possible_types} |")
			}
		})
		.collect::<Vec<_>>();
	if !rows.is_empty() {
		let rows = rows.join("\n");
		let content = formatdoc!(
			"

			### Inputs

			| Parameter | Details | Possible Types |
			|:-|:-|:-|
			{rows}
			"
		);
		page.write_all(content.as_bytes()).expect("Failed to write to node page file");
	}
}

fn node_write_outputs(mut page: std::fs::File, valid_primary_outputs: String) {
	let product = "Result";
	let details = "The value produced by the node operation.";

	let mut details = format!("<p>{details}</p>");
	details.push_str("<p>*Primary Output*</p>");

	let content = formatdoc!(
		"

		### Outputs

		| Product | Details | Possible Types |
		|:-|:-|:-|
		| {product} | {details} | {valid_primary_outputs} |
		"
	);
	page.write_all(content.as_bytes()).expect("Failed to write to node page file");
}

fn write_category_index_page(index: usize, category: &str, nodes: &[(&core_types::ProtoNodeIdentifier, &NodeMetadata)], category_path: &String) {
	std::fs::create_dir_all(category_path).expect("Failed to create category directory");
	let page_path = format!("{category_path}/_index.md");
	let mut page = std::fs::File::create(&page_path).expect("Failed to create index file");

	// Write frontmatter
	let order = index + 1;
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

	// Write description
	let category_description = category_description(category);
	let content = formatdoc!(
		"

		{category_description}
		"
	);
	page.write_all(content.as_bytes()).expect("Failed to write to index file");

	// Write nodes table header
	let content = formatdoc!(
		"

		## Nodes

		| Node | Details | Possible Types |
		|:-|:-|:-|
		"
	);
	page.write_all(content.as_bytes()).expect("Failed to write to index file");

	// Write nodes table rows
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
					format!(
						"`{} → {}`",
						node_io
							.inputs
							.first()
							.map(|t| t.nested_type())
							.filter(|&t| t != &concrete!(()))
							.map(ToString::to_string)
							.unwrap_or_default(),
						node_io.return_value.nested_type()
					)
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

fn sanitize_path(s: &str) -> String {
	// Replace disallowed characters with a dash
	let allowed_characters = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~[]@!$&'()*+,;=";
	let filtered = s.chars().map(|c| if allowed_characters.contains(c) { c } else { '-' }).collect::<String>();

	// Fix letter-number type names
	let mut filtered = format!("-{filtered}-");
	filtered = filtered.replace("-vec-2-", "-vec2-");
	filtered = filtered.replace("-f-32-", "-f32-");
	filtered = filtered.replace("-f-64-", "-f64-");
	filtered = filtered.replace("-u-32-", "-u32-");
	filtered = filtered.replace("-u-64-", "-u64-");
	filtered = filtered.replace("-i-32-", "-i32-");
	filtered = filtered.replace("-i-64-", "-i64-");

	// Remove consecutive dashes
	while filtered.contains("--") {
		filtered = filtered.replace("--", "-");
	}

	// Trim leading and trailing dashes
	filtered.trim_matches('-').to_string()
}

fn node_description(metadata: &NodeMetadata) -> &str {
	let mut description = metadata.description.trim();
	if description.is_empty() {
		description = "*Node description coming soon.*";
	}
	description
}

fn category_description(category: &str) -> &str {
	match category {
		"Animation" => indoc!(
			"
			Nodes in this category enable the creation of animated, real-time, and interactive motion graphics involving paramters that change over time.

			These nodes require that playback is activated by pressing the play button above the viewport.
			"
		),
		"Blending" => "Nodes in this category control how overlapping graphical content is composited together, considering blend modes, opacity, and clipping.",
		"Color" => "Nodes in this category deal with selecting and manipulating colors, gradients, and palettes.",
		"Debug" => indoc!(
			"
			Nodes in this category are temporarily included for debugging purposes by Graphite's developers. They may have rare potential uses for advanced users, but are not intended for general use and will be removed in future releases.
			"
		),
		"General" => "Nodes in this category deal with general data handling, such as merging and flattening graphical elements.",
		"Instancing" => "Nodes in this category enable the duplication, arrangement, and looped generation of graphical elements.",
		"Math: Arithmetic" => "Nodes in this category perform common arithmetic operations on numerical values (and where applicable, `vec2` values).",
		"Math: Logic" => "Nodes in this category perform boolean logic operations such as comparisons, conditionals, logic gates, and switching.",
		"Math: Numeric" => "Nodes in this category perform discontinuous numeric operations such as rounding, clamping, mapping, and randomization.",
		"Math: Transform" => "Nodes in this category perform transformations on graphical elements and calculations involving transformation matrices.",
		"Math: Trig" => "Nodes in this category perform trigonometric operations such as sine, cosine, tangent, and their inverses.",
		"Math: Vector" => "Nodes in this category perform operations involving `vec2` values (points or arrows in 2D space) such as the dot product, normalization, and distance calculations.",
		"Raster: Adjustment" => "Nodes in this category perform per-pixel color adjustments on raster graphics, such as brightness and contrast modifications.",
		"Raster: Channels" => "Nodes in this category enable channel-specific manipulation of the RGB and alpha channels of raster graphics.",
		"Raster: Filter" => "Nodes in this category apply filtering effects to raster graphics such as blurs and sharpening.",
		"Raster: Pattern" => "Nodes in this category generate procedural raster patterns, fractals, textures, and noise.",
		"Raster" => "Nodes in this category deal with fundamental raster image operations.",
		"Text" => "Nodes in this category support the manipulation, formatting, and rendering of text strings.",
		"Value" => "Nodes in this category supply data values of common types such as numbers, colors, booleans, and strings.",
		"Vector: Measure" => "Nodes in this category perform measurements and analysis on vector graphics, such as length/area calculations, path traversal, and hit testing.",
		"Vector: Modifier" => "Nodes in this category modify the geometry of vector graphics, such as boolean operations, smoothing, and morphing.",
		"Vector: Shape" => "Nodes in this category generate parametrically-described primitive vector shapes such as rectangles, grids, stars, and spirals.",
		"Vector: Style" => "Nodes in this category apply fill and stroke styles to alter the appearance of vector graphics.",
		"Vector" => "Nodes in this category deal with fundamental vector graphics data handling and operations.",
		"Web Request" => "Nodes in this category facilitate fetching and handling resources from HTTP endpoints and sending webhook requests to external services.",
		_ => panic!("Category '{category}' is missing a description"),
	}.trim()
}

fn write_catalog_index_page(categories: &[String]) {
	if std::path::Path::new(NODE_CATALOG_PATH).exists() {
		std::fs::remove_dir_all(NODE_CATALOG_PATH).expect("Failed to remove existing node catalog directory");
	}
	std::fs::create_dir_all(NODE_CATALOG_PATH).expect("Failed to create node catalog directory");
	let page_path = format!("{NODE_CATALOG_PATH}/_index.md");
	let mut page = std::fs::File::create(&page_path).expect("Failed to create index file");

	// Write frontmatter
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

	// Write description
	let content = formatdoc!(
		"

		The node catalog documents all of the nodes available in Graphite's node graph system, organized by category.

		<p><img src=\"https://static.graphite.art/content/learn/node-catalog/node-terminology.avif\" onerror=\"this.onerror = null; this.src = this.src.replace('.avif', '.png')\" alt=\"Terminology diagram covering how the node system operates\" /></p>
		"
	);
	page.write_all(content.as_bytes()).expect("Failed to write to index file");

	// Write node categories table header
	let content = formatdoc!(
		"
	
		## Node categories

		| Category | Details |
		|:-|:-|
		"
	);
	page.write_all(content.as_bytes()).expect("Failed to write to index file");

	// Write node categories table rows
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
