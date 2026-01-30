use crate::utility::*;
use convert_case::{Case, Casing};
use graph_craft::concrete;
use graph_craft::document::value;
use graph_craft::proto::{NodeMetadata, RegistryValueSource};
use graphene_std::{ContextDependencies, core_types};
use indoc::formatdoc;
use std::collections::HashSet;
use std::io::Write;

pub fn write_node_page(index: usize, id: &core_types::ProtoNodeIdentifier, metadata: &NodeMetadata, category_path: &String) {
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
			valid_input_types[i].push(ty.nested_type().clone());
		}
	}
	for item in valid_input_types.iter_mut() {
		// Dedupe while preserving order
		let mut found = HashSet::new();
		*item = item.clone().into_iter().filter(|s| found.insert(s.clone())).collect::<Vec<_>>()
	}

	// Primary output types
	let valid_primary_outputs = implementations.iter().map(|(_, node_io)| node_io.return_value.nested_type().clone()).collect::<Vec<_>>();

	// Write sections to the file
	write_frontmatter(&mut page, metadata, index + 1);
	write_description(&mut page, metadata);
	write_interface_header(&mut page);
	write_context(&mut page, context_dependencies);
	write_inputs(&mut page, &valid_input_types, metadata);
	write_outputs(&mut page, &valid_primary_outputs);
}

fn write_frontmatter(page: &mut std::fs::File, metadata: &NodeMetadata, order: usize) {
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

fn write_description(page: &mut std::fs::File, metadata: &NodeMetadata) {
	let description = node_description(metadata);

	let content = formatdoc!(
		"

		{description}
		"
	);
	page.write_all(content.as_bytes()).expect("Failed to write to node page file");
}

fn write_interface_header(page: &mut std::fs::File) {
	let content = formatdoc!(
		"

		## Interface
		"
	);
	page.write_all(content.as_bytes()).expect("Failed to write to node page file");
}

fn write_context(page: &mut std::fs::File, context_dependencies: ContextDependencies) {
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

fn write_inputs(page: &mut std::fs::File, valid_input_types: &[Vec<core_types::Type>], metadata: &NodeMetadata) {
	let rows = metadata
		.fields
		.iter()
		.enumerate()
		.filter(|&(index, field)| !field.hidden || index == 0)
		.map(|(index, field)| {
			// Parameter
			let parameter = field.name;

			// Possible types
			let possible_types_list = valid_input_types.get(index).cloned().unwrap_or_default();
			if index == 0 && possible_types_list.as_slice() == [concrete!(())] {
				return "| - | *No Primary Input* | - |".to_string();
			}
			let mut possible_types = possible_types_list.iter().map(|ty| format!("`{ty}`")).collect::<Vec<_>>();
			possible_types.sort();
			possible_types.dedup();
			let mut possible_types = possible_types.join("<br />");
			if possible_types.is_empty() {
				possible_types = "*Any Type*".to_string();
			}

			// Details: description
			let mut details = field
				.description
				.trim()
				.split('\n')
				.filter(|line| !line.is_empty())
				.map(|line| format!("<p>{}</p>", line.trim()))
				.collect::<Vec<_>>();

			// Details: primary input
			if index == 0 {
				details.push("<p>*Primary Input*</p>".to_string());
			}

			// Details: exposed by default
			if field.exposed {
				details.push("<p>*Exposed to the Graph by Default*</p>".to_string());
			}

			// Details: sourced from scope
			if let RegistryValueSource::Scope(scope_name) = &field.value_source {
				details.push(format!("<p>*Sourced From Scope: `{scope_name}`*</p>"));
			}

			// Details: default value
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

			// Construct the table row
			let details = details.join("");
			format!("| {parameter} | {details} | {possible_types} |")
		})
		.collect::<Vec<_>>()
		.join("\n");
	if !rows.is_empty() {
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

fn write_outputs(page: &mut std::fs::File, valid_primary_outputs: &[core_types::Type]) {
	// Product
	let product = "Result";

	// Details: description
	let details = "The value produced by the node operation.";
	let mut details = format!("<p>{details}</p>");

	// Details: primary output
	details.push_str("<p>*Primary Output*</p>");

	// Possible types
	let valid_primary_outputs = valid_primary_outputs.iter().map(|ty| format!("`{ty}`")).collect::<Vec<_>>();
	let valid_primary_outputs = {
		// Dedupe while preserving order
		let mut found = HashSet::new();
		valid_primary_outputs.into_iter().filter(|s| found.insert(s.clone())).collect::<Vec<_>>()
	};
	let valid_primary_outputs = {
		// Dedupe while preserving order
		let mut found = HashSet::new();
		valid_primary_outputs.into_iter().filter(|s| found.insert(s.clone())).collect::<Vec<_>>()
	};
	let valid_primary_outputs = valid_primary_outputs.join("<br />");

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
