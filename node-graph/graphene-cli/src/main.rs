mod export;

use clap::{Args, Parser, Subcommand};
use convert_case::{Case, Casing};
use fern::colors::{Color, ColoredLevelConfig};
use futures::executor::block_on;
use graph_craft::concrete;
use graph_craft::document::*;
use graph_craft::graphene_compiler::Compiler;
use graph_craft::proto::{NodeMetadata, ProtoNetwork, RegistryValueSource};
use graph_craft::util::load_network;
use graph_craft::wasm_application_io::EditorPreferences;
use graphene_std::application_io::{ApplicationIo, NodeGraphUpdateMessage, NodeGraphUpdateSender};
use graphene_std::text::FontCache;
use graphene_std::wasm_application_io::{WasmApplicationIo, WasmEditorApi};
use graphene_std::{ContextDependencies, core_types};
use indoc::formatdoc;
use interpreted_executor::dynamic_executor::DynamicExecutor;
use interpreted_executor::util::wrap_network_in_scope;
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

struct UpdateLogger {}

impl NodeGraphUpdateSender for UpdateLogger {
	fn send(&self, message: NodeGraphUpdateMessage) {
		println!("{message:?}");
	}
}

#[derive(Debug, Parser)]
#[clap(name = "graphene-cli", version)]
pub struct App {
	#[clap(flatten)]
	global_opts: GlobalOpts,

	#[clap(subcommand)]
	command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
	/// Help message for compile.
	Compile {
		/// Print proto network
		#[clap(long, short = 'p')]
		print_proto: bool,

		/// Path to the .graphite document
		document: PathBuf,
	},
	/// Export a .graphite document to a file (SVG, PNG, or JPG).
	Export {
		/// Path to the .graphite document
		document: PathBuf,

		/// Output file path (extension determines format: .svg, .png, .jpg)
		#[clap(long, short = 'o')]
		output: PathBuf,

		/// Optional input image resource
		#[clap(long)]
		image: Option<PathBuf>,

		/// Scale factor for export (default: 1.0)
		#[clap(long, default_value = "1.0")]
		scale: f64,

		/// Output width in pixels
		#[clap(long)]
		width: Option<u32>,

		/// Output height in pixels
		#[clap(long)]
		height: Option<u32>,

		/// Transparent background for PNG exports
		#[clap(long)]
		transparent: bool,
	},
	ListNodeIdentifiers,
	BuildNodeDocs,
}

#[derive(Debug, Args)]
struct GlobalOpts {
	/// Verbosity level (can be specified multiple times)
	#[clap(long, short, global = true, action = clap::ArgAction::Count)]
	verbose: u8,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let app = App::parse();

	let log_level = app.global_opts.verbose;

	init_logging(log_level);

	let document_path = match app.command {
		Command::Compile { ref document, .. } => document,
		Command::Export { ref document, .. } => document,
		Command::ListNodeIdentifiers => {
			let mut nodes: Vec<_> = graphene_std::registry::NODE_METADATA.lock().unwrap().keys().cloned().collect();
			nodes.sort_by_key(|x| x.as_str().to_string());
			for id in nodes {
				println!("{}", id.as_str());
			}
			return Ok(());
		}
		Command::BuildNodeDocs => {
			// TODO: Also obtain document nodes, not only proto nodes
			let nodes = graphene_std::registry::NODE_METADATA.lock().unwrap();
			let node_registry = core_types::registry::NODE_REGISTRY.lock().unwrap();

			let sanitize_path = |s: &str| {
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
			};

			// =================
			// NODE CATALOG PAGE
			// =================

			// Group nodes by category
			let mut nodes_by_category: HashMap<String, Vec<_>> = HashMap::new();
			for (id, metadata) in nodes.iter() {
				nodes_by_category.entry(metadata.category.to_string()).or_default().push((id, metadata));
			}

			// Sort the categories
			let mut categories: Vec<_> = nodes_by_category.keys().cloned().collect();
			categories.sort();

			// Create _index.md for the node catalog page
			let node_catalog_path = "../../website/content/learn/node-catalog";
			if std::path::Path::new(node_catalog_path).exists() {
				std::fs::remove_dir_all(node_catalog_path).expect("Failed to remove existing node catalog directory");
			}
			std::fs::create_dir_all(node_catalog_path).expect("Failed to create node catalog directory");
			let page_path = format!("{node_catalog_path}/_index.md");
			let mut page = std::fs::File::create(&page_path).expect("Failed to create index file");

			// ===============================
			// NODE CATALOG: WRITE FRONTMATTER
			// ===============================
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

			// ===============================
			// NODE CATALOG: WRITE DESCRIPTION
			// ===============================
			let content = formatdoc!(
				"
				
				The node catalog documents all of the nodes available in Graphite's node graph system, organized by category.

				<p><img src=\"https://static.graphite.art/content/learn/node-catalog/node-terminology.avif\" onerror=\"this.onerror = null; this.src = this.src.replace('.avif', '.png')\" alt=\"Terminology diagram covering how the node system operates\" /></p>
				"
			);
			page.write_all(content.as_bytes()).expect("Failed to write to index file");

			// ==============================
			// NODE CATALOG: WRITE CATEGORIES
			// ==============================
			let content = formatdoc!(
				"

				## Node categories

				| Category | Details |
				|:-|:-|
				"
			);
			page.write_all(content.as_bytes()).expect("Failed to write to index file");

			let content = categories
				.iter()
				// .filter(|c| !c.is_empty())
				.map(|c| if c.is_empty() { "Hidden" } else { c })
				.map(|category| {
					let category_path_part = sanitize_path(&category.to_case(Case::Kebab));
					let details = format!("This is the {category} category of nodes.");
					format!("| [{category}](./{category_path_part}) | {details} |")
				})
				.collect::<Vec<_>>()
				.join("\n");
			page.write_all(content.as_bytes()).expect("Failed to write to index file");

			// ===================
			// NODE CATEGORY PAGES
			// ===================
			for (index, category) in categories.iter().map(|c| if c.is_empty() { "Hidden" } else { c }).filter(|c| !c.is_empty()).enumerate() {
				// Get nodes in this category
				let mut nodes = nodes_by_category.remove(if category == "Hidden" { "" } else { category }).unwrap();
				nodes.sort_by_key(|(_, metadata)| metadata.display_name.to_string());

				// Create _index.md file for category
				let category_path_part = sanitize_path(&category.to_case(Case::Kebab));
				let category_path = format!("{node_catalog_path}/{category_path_part}");
				std::fs::create_dir_all(&category_path).expect("Failed to create category directory");
				let page_path = format!("{category_path}/_index.md");
				let mut page = std::fs::File::create(&page_path).expect("Failed to create index file");

				// ================================
				// NODE CATEGORY: WRITE FRONTMATTER
				// ================================
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

				// ================================
				// NODE CATEGORY: WRITE DESCRIPTION
				// ================================
				let content = formatdoc!(
					"

					This is the {category} category of nodes.
					"
				);
				page.write_all(content.as_bytes()).expect("Failed to write to index file");

				// ================================
				// NODE CATEGORY: WRITE NODES TABLE
				// ================================
				let content = formatdoc!(
					"

					## Nodes

					| Node | Details | Possible Types |
					|:-|:-|:-|
					"
				);
				page.write_all(content.as_bytes()).expect("Failed to write to index file");

				let name_and_description = |metadata: &NodeMetadata| {
					let name = metadata.display_name;
					let mut description = metadata.description.trim();
					if description.is_empty() {
						description = "*Node description coming soon.*";
					}
					(name, description)
				};

				let content = nodes
					.iter()
					.filter_map(|&(id, metadata)| {
						// Path to page
						let name_url_part = sanitize_path(&metadata.display_name.to_case(Case::Kebab));

						// Name and description
						let (name, description) = name_and_description(metadata);
						let details = description.split('\n').map(|line| format!("<p>{}</p>", line.trim())).collect::<Vec<_>>().join("");

						// Possible types
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

				// ==========
				// NODE PAGES
				// ==========
				for (index, (id, metadata)) in nodes.into_iter().enumerate() {
					let Some(implementations) = node_registry.get(id) else { continue };

					// Path to page
					let name_url_part = sanitize_path(&metadata.display_name.to_case(Case::Kebab));
					let page_path = format!("{category_path}/{name_url_part}.md");
					let mut page = std::fs::File::create(&page_path).expect("Failed to create node page file");

					// Name and description
					let (name, description) = name_and_description(metadata);

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

					// =======================
					// NODE: WRITE FRONTMATTER
					// =======================
					let order = index + 1;
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

					// =======================
					// NODE: WRITE DESCRIPTION
					// =======================
					let content = formatdoc!(
						"

						{description}

						## Interface
						"
					);
					page.write_all(content.as_bytes()).expect("Failed to write to node page file");

					// ===================
					// NODE: WRITE CONTEXT
					// ===================
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

					// ==================
					// NODE: WRITE INPUTS
					// ==================
					let rows = metadata
						.fields
						.iter()
						.enumerate()
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
							if let Some(default_value) = match field.value_source {
								RegistryValueSource::None => None,
								RegistryValueSource::Scope(scope_name) => {
									details.push(format!("<p>*Sourced From Scope: `{scope_name}`*</p>"));
									None
								}
								RegistryValueSource::Default(default_value) => Some(default_value.to_string().replace(" :: ", "::")),
							}
							.or_else(|| {
								let ty = field
									.default_type
									.as_ref()
									.or(match possible_types_list.as_slice() {
										[single] => Some(single),
										_ => None,
									})?
									.nested_type();
								Some(match () {
									() if ty == &concrete!(f32) => f32::default().to_string(),
									() if ty == &concrete!(f64) => f64::default().to_string(),
									() if ty == &concrete!(u32) => u32::default().to_string(),
									() if ty == &concrete!(u64) => u64::default().to_string(),
									() if ty == &concrete!(i32) => i32::default().to_string(),
									() if ty == &concrete!(i64) => i64::default().to_string(),
									() if ty == &concrete!(bool) => bool::default().to_string(),
									() if ty == &concrete!(&str) => "\"\"".to_string(),
									() if ty == &concrete!(String) => "\"\"".to_string(),
									() if ty == &concrete!(Vec<f64>) => "[]".to_string(),
									() if ty == &concrete!(value::DVec2) => "(0, 0)".to_string(),
									() if ty == &concrete!(value::DAffine2) => value::DAffine2::default().to_string(),
									() if ty == &concrete!(graphene_std::gradient::GradientStops) => "BLACK_TO_WHITE".to_string(),
									_ => return None,
								})
							}) {
								let default_value = default_value.trim_end_matches('.'); // Remove trailing period on whole-number floats
								let render_color = |color| format!(r#"<span style="padding-right: 100px; border: 2px solid var(--color-fog); background: {color}"></span>"#);
								let default_value = match default_value {
									"Color::BLACK" => render_color("black"),
									"BLACK_TO_WHITE" => render_color("linear-gradient(to right, black, white)"),
									_ => format!("`{default_value}{}`", field.unit.unwrap_or_default()),
								};
								details.push(format!("<p>*Default:&nbsp;{default_value}*</p>"));
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

					// ===================
					// NODE: WRITE OUTPUTS
					// ===================
					let content = formatdoc!(
						"

						### Outputs

						| Product | Details | Possible Types |
						|:-|:-|:-|
						| Result | <p>The value produced by the node operation.</p><p>*Primary Output*</p> | {valid_primary_outputs} |
						"
					);
					page.write_all(content.as_bytes()).expect("Failed to write to node page file");
				}
			}
			return Ok(());
		}
	};

	let document_string = std::fs::read_to_string(document_path).expect("Failed to read document");

	log::info!("Creating GPU context");
	let mut application_io = block_on(WasmApplicationIo::new_offscreen());

	if let Command::Export { image: Some(ref image_path), .. } = app.command {
		application_io.resources.insert("null".to_string(), Arc::from(std::fs::read(image_path).expect("Failed to read image")));
	}

	// Convert application_io to Arc first
	let application_io_arc = Arc::new(application_io);

	// Clone the application_io Arc before borrowing to extract executor
	let application_io_for_api = application_io_arc.clone();

	// Get reference to wgpu executor and clone device handle
	let wgpu_executor_ref = application_io_arc.gpu_executor().unwrap();
	let device = wgpu_executor_ref.context.device.clone();

	let preferences = EditorPreferences { use_vello: true };
	let editor_api = Arc::new(WasmEditorApi {
		font_cache: FontCache::default(),
		application_io: Some(application_io_for_api),
		node_graph_message_sender: Box::new(UpdateLogger {}),
		editor_preferences: Box::new(preferences),
	});

	let proto_graph = compile_graph(document_string, editor_api)?;

	match app.command {
		Command::Compile { print_proto, .. } => {
			if print_proto {
				println!("{proto_graph}");
			}
		}
		Command::Export {
			output,
			scale,
			width,
			height,
			transparent,
			..
		} => {
			// Spawn thread to poll GPU device
			std::thread::spawn(move || {
				loop {
					std::thread::sleep(std::time::Duration::from_nanos(10));
					device.poll(wgpu::PollType::Poll).unwrap();
				}
			});

			// Detect output file type
			let file_type = export::detect_file_type(&output)?;

			// Create executor
			let executor = create_executor(proto_graph)?;

			// Perform export
			export::export_document(&executor, wgpu_executor_ref, output, file_type, scale, (width, height), transparent).await?;
		}
		_ => unreachable!("All other commands should be handled before this match statement is run"),
	}

	Ok(())
}

fn init_logging(log_level: u8) {
	let default_level = match log_level {
		0 => log::LevelFilter::Error,
		1 => log::LevelFilter::Info,
		2 => log::LevelFilter::Debug,
		_ => log::LevelFilter::Trace,
	};
	let colors = ColoredLevelConfig::new().debug(Color::Magenta).info(Color::Green).error(Color::Red);
	fern::Dispatch::new()
		.chain(std::io::stdout())
		.level_for("wgpu", log::LevelFilter::Error)
		.level_for("naga", log::LevelFilter::Error)
		.level_for("wgpu_hal", log::LevelFilter::Error)
		.level_for("wgpu_core", log::LevelFilter::Error)
		.level(default_level)
		.format(move |out, message, record| {
			out.finish(format_args!(
				"[{}]{}{} {}",
				// This will color the log level only, not the whole line. Just a touch.
				colors.color(record.level()),
				chrono::Utc::now().format("[%Y-%m-%d %H:%M:%S]"),
				record.module_path().unwrap_or(""),
				message
			))
		})
		.apply()
		.unwrap();
}

// Migrations are done in the editor which is unfortunately not available here.
// TODO: remove this and share migrations between the editor and the CLI.
fn fix_nodes(network: &mut NodeNetwork) {
	for node in network.nodes.values_mut() {
		match &mut node.implementation {
			// Recursively fix
			DocumentNodeImplementation::Network(network) => fix_nodes(network),
			// This replicates the migration from the editor linked:
			// https://github.com/GraphiteEditor/Graphite/blob/d68f91ccca69e90e6d2df78d544d36cd1aaf348e/editor/src/messages/portfolio/portfolio_message_handler.rs#L535
			// Since the CLI doesn't have the document node definitions, a less robust method of just patching the inputs is used.
			DocumentNodeImplementation::ProtoNode(proto_node_identifier)
				if (proto_node_identifier.as_str().starts_with("graphene_core::ConstructLayerNode") || proto_node_identifier.as_str().starts_with("graphene_core::AddArtboardNode"))
					&& node.inputs.len() < 3 =>
			{
				node.inputs.push(NodeInput::Reflection(DocumentNodeMetadata::DocumentNodePath));
			}
			_ => {}
		}
	}
}
fn compile_graph(document_string: String, editor_api: Arc<WasmEditorApi>) -> Result<ProtoNetwork, Box<dyn Error>> {
	let mut network = load_network(&document_string);
	fix_nodes(&mut network);

	let substitutions = preprocessor::generate_node_substitutions();
	preprocessor::expand_network(&mut network, &substitutions);

	let wrapped_network = wrap_network_in_scope(network.clone(), editor_api);

	let compiler = Compiler {};
	compiler.compile_single(wrapped_network).map_err(|x| x.into())
}

fn create_executor(proto_network: ProtoNetwork) -> Result<DynamicExecutor, Box<dyn Error>> {
	let executor = block_on(DynamicExecutor::new(proto_network)).map_err(|errors| errors.iter().map(|e| format!("{e:?}")).reduce(|acc, e| format!("{acc}\n{e}")).unwrap_or_default())?;
	Ok(executor)
}
