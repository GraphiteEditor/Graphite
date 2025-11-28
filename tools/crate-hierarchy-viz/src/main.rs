use anyhow::{Context, Result, anyhow};
use clap::{Parser, ValueEnum};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, ValueEnum)]
enum OutputFormat {
	/// Output DOT format (GraphViz)
	Dot,
	/// Output PNG image (requires GraphViz)
	Png,
	/// Output SVG image (requires GraphViz)
	Svg,
}

#[derive(Parser)]
#[command(name = "crate-hierarchy-viz")]
#[command(about = "Visualize the crate hierarchy in the Graphite workspace")]
struct Args {
	/// Workspace root directory (defaults to current directory)
	#[arg(short, long)]
	workspace: Option<PathBuf>,

	/// Output file (defaults to stdout for DOT format, required for PNG/SVG)
	#[arg(short, long)]
	output: Option<PathBuf>,

	/// Output format
	#[arg(short, long, value_enum, default_value = "dot")]
	format: OutputFormat,
}

#[derive(Debug, Deserialize)]
struct WorkspaceToml {
	workspace: WorkspaceConfig,
}

#[derive(Debug, Deserialize)]
struct WorkspaceConfig {
	members: Vec<String>,
	dependencies: Option<HashMap<String, WorkspaceDependency>>,
}

/// Represents a workspace-level dependency in Cargo.toml
/// The Simple variant's String is needed for serde deserialization but never read directly
#[derive(Debug, Deserialize)]
#[serde(untagged)]
#[allow(dead_code)]
enum WorkspaceDependency {
	Simple(String),
	Detailed {
		#[serde(flatten)]
		_other: HashMap<String, toml::Value>,
	},
}

#[derive(Debug, Deserialize)]
struct CrateToml {
	package: PackageConfig,
	dependencies: Option<HashMap<String, CrateDependency>>,
}

#[derive(Debug, Deserialize)]
struct PackageConfig {
	name: String,
}

/// Represents a crate-level dependency in Cargo.toml
/// The Simple variant's String is needed for serde deserialization but never read directly
#[derive(Debug, Deserialize)]
#[serde(untagged)]
#[allow(dead_code)]
enum CrateDependency {
	Simple(String),
	Detailed {
		path: Option<String>,
		workspace: Option<bool>,
		#[serde(flatten)]
		other: HashMap<String, toml::Value>,
	},
}

#[derive(Debug, Clone, PartialEq)]
struct CrateInfo {
	name: String,
	path: PathBuf,
	dependencies: Vec<String>,
	external_dependencies: Vec<String>,
}

/// Remove transitive dependencies from the crate list.
/// If A depends on B and C, and B depends on C, then A->C is removed.
fn remove_transitive_dependencies(crates: &mut [CrateInfo]) {
	// Build a map from crate name to its dependencies for quick lookup
	let dep_map: HashMap<String, HashSet<String>> = crates.iter().map(|c| (c.name.clone(), c.dependencies.iter().cloned().collect())).collect();

	// For each crate, compute which dependencies are reachable through other dependencies
	for crate_info in crates.iter_mut() {
		let mut transitive_deps = HashSet::new();

		// For each direct dependency, find all its transitive dependencies
		for direct_dep in &crate_info.dependencies {
			// Recursively collect all transitive dependencies of this direct dependency
			let mut visited = HashSet::new();
			collect_all_dependencies(direct_dep, &dep_map, &mut visited);
			// Remove the direct dependency itself from the visited set
			visited.remove(direct_dep);
			transitive_deps.extend(visited);
		}

		// Remove dependencies that are transitive
		crate_info.dependencies.retain(|dep| !transitive_deps.contains(dep));
	}
}

/// Recursively collect all dependencies of a crate
fn collect_all_dependencies(crate_name: &str, dep_map: &HashMap<String, HashSet<String>>, visited: &mut HashSet<String>) {
	if !visited.insert(crate_name.to_string()) {
		return; // Already visited, avoid cycles
	}

	if let Some(deps) = dep_map.get(crate_name) {
		for dep in deps {
			collect_all_dependencies(dep, dep_map, visited);
		}
	}
}

fn main() -> Result<()> {
	let args = Args::parse();

	let workspace_root = args.workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
	let workspace_toml_path = workspace_root.join("Cargo.toml");

	// Parse workspace Cargo.toml
	let workspace_content = fs::read_to_string(&workspace_toml_path).with_context(|| format!("Failed to read {:?}", workspace_toml_path))?;
	let workspace_toml: WorkspaceToml = toml::from_str(&workspace_content).with_context(|| "Failed to parse workspace Cargo.toml")?;

	// Get workspace dependencies (external crates defined at workspace level)
	let workspace_deps: HashSet<String> = workspace_toml.workspace.dependencies.unwrap_or_default().keys().cloned().collect();

	// Parse each member crate and build name mapping
	let mut crates = Vec::new();
	let mut workspace_crate_names = HashSet::new();

	// First pass: collect all workspace crate names
	for member in &workspace_toml.workspace.members {
		let crate_path = workspace_root.join(member);
		let cargo_toml_path = crate_path.join("Cargo.toml");

		if !cargo_toml_path.exists() {
			eprintln!("Warning: Cargo.toml not found for member: {}", member);
			continue;
		}

		let crate_content = fs::read_to_string(&cargo_toml_path).with_context(|| format!("Failed to read {:?}", cargo_toml_path))?;
		let crate_toml: CrateToml = toml::from_str(&crate_content).with_context(|| format!("Failed to parse Cargo.toml for {}", member))?;

		workspace_crate_names.insert(crate_toml.package.name.clone());
	}

	// Second pass: parse dependencies now that we know all workspace crate names
	for member in &workspace_toml.workspace.members {
		let crate_path = workspace_root.join(member);
		let cargo_toml_path = crate_path.join("Cargo.toml");

		if !cargo_toml_path.exists() {
			continue;
		}

		let crate_content = fs::read_to_string(&cargo_toml_path).with_context(|| format!("Failed to read {:?}", cargo_toml_path))?;
		let crate_toml: CrateToml = toml::from_str(&crate_content).with_context(|| format!("Failed to parse Cargo.toml for {}", member))?;

		let mut dependencies = Vec::new();
		let mut external_dependencies = Vec::new();

		if let Some(deps) = &crate_toml.dependencies {
			for (dep_name, dep_config) in deps {
				let is_workspace_crate = workspace_crate_names.contains(dep_name);
				let is_workspace_dep = workspace_deps.contains(dep_name);

				let is_local_dep = match dep_config {
					CrateDependency::Detailed { workspace: Some(true), .. } => is_workspace_dep,
					CrateDependency::Detailed { path: Some(_), .. } => true,
					CrateDependency::Simple(_) => is_workspace_dep,
					_ => false,
				};

				// Check if this dependency has a different package name
				let actual_dep_name = match dep_config {
					CrateDependency::Detailed { other, .. } => {
						// Check if there's a "package" field that renames the dependency
						if let Some(toml::Value::String(package_name)) = other.get("package") {
							package_name.clone()
						} else {
							dep_name.clone()
						}
					}
					_ => dep_name.clone(),
				};

				let is_actual_workspace_crate = workspace_crate_names.contains(&actual_dep_name);

				if is_workspace_crate || is_actual_workspace_crate || is_local_dep {
					dependencies.push(actual_dep_name);
				} else {
					external_dependencies.push(actual_dep_name);
				}
			}
		}

		crates.push(CrateInfo {
			name: crate_toml.package.name.clone(),
			path: crate_path,
			dependencies,
			external_dependencies,
		});
	}

	// Filter dependencies to only include workspace crates
	for crate_info in &mut crates {
		crate_info.dependencies.retain(|dep| workspace_crate_names.contains(dep));
	}

	// Remove transitive dependencies
	remove_transitive_dependencies(&mut crates);

	// Generate DOT format
	let dot_content = generate_dot_format(&crates)?;

	// Handle output based on format
	match args.format {
		OutputFormat::Dot => {
			// Write DOT output
			if let Some(output_path) = args.output {
				fs::write(&output_path, &dot_content).with_context(|| format!("Failed to write to {:?}", output_path))?;
				println!("DOT output written to: {:?}", output_path);
			} else {
				print!("{}", dot_content);
			}
		}
		OutputFormat::Png | OutputFormat::Svg => {
			// Require output file for PNG/SVG
			let output_path = args.output.ok_or_else(|| anyhow!("Output file (-o/--output) is required for PNG/SVG formats"))?;

			// Check if dot command is available
			let dot_check = Command::new("dot").arg("-V").output();
			if dot_check.is_err() || !dot_check.as_ref().unwrap().status.success() {
				return Err(anyhow!(
					"GraphViz 'dot' command not found. Please install GraphViz to generate PNG/SVG output.\n\
					 On Ubuntu/Debian: sudo apt-get install graphviz\n\
					 On macOS: brew install graphviz\n\
					 On Windows: Download from https://graphviz.org/download/"
				));
			}

			// Determine the format argument for dot
			let format_arg = match args.format {
				OutputFormat::Png => "png",
				OutputFormat::Svg => "svg",
				_ => unreachable!(),
			};

			// Run dot command to generate the output
			let mut dot_process = Command::new("dot")
				.arg(format!("-T{}", format_arg))
				.arg("-o")
				.arg(&output_path)
				.stdin(std::process::Stdio::piped())
				.spawn()
				.with_context(|| "Failed to spawn 'dot' command")?;

			// Write DOT content to stdin
			use std::io::Write;
			if let Some(mut stdin) = dot_process.stdin.take() {
				stdin.write_all(dot_content.as_bytes()).with_context(|| "Failed to write DOT content to 'dot' command")?;
				// Close stdin to signal EOF
				drop(stdin);
			}

			// Wait for the command to complete
			let status = dot_process.wait().with_context(|| "Failed to wait for 'dot' command")?;
			if !status.success() {
				return Err(anyhow!("'dot' command failed with exit code: {:?}", status.code()));
			}

			println!("{} output written to: {:?}", format_arg.to_uppercase(), output_path);
		}
	}

	Ok(())
}

fn generate_dot_format(crates: &[CrateInfo]) -> Result<String> {
	let mut output = String::new();
	output.push_str("digraph CrateHierarchy {\n");
	output.push_str("    rankdir=LR;\n");
	output.push_str("    node [shape=box, style=\"rounded,filled\", fillcolor=lightblue];\n");
	output.push_str("    edge [color=gray];\n\n");

	// Add subgraphs for different categories
	output.push_str("    subgraph cluster_core {\n");
	output.push_str("        label=\"Core Components\";\n");
	output.push_str("        style=filled;\n");
	output.push_str("        fillcolor=lightgray;\n");

	let core_crates: Vec<_> = crates
		.iter()
		.filter(|c| (c.name.starts_with("graphite-") || c.name == "editor" || c.name == "graphene-cli") && !c.name.contains("desktop"))
		.collect();

	for crate_info in &core_crates {
		output.push_str(&format!("        \"{}\";\n", crate_info.name));
	}
	output.push_str("    }\n\n");

	output.push_str("    subgraph cluster_nodegraph {\n");
	output.push_str("        label=\"Node Graph System\";\n");
	output.push_str("        style=filled;\n");
	output.push_str("        fillcolor=lightyellow;\n");

	let nodegraph_crates: Vec<_> = crates
		.iter()
		.filter(|c| c.name == "graph-craft" || c.name == "interpreted-executor" || c.name == "node-macro" || c.name == "preprocessor" || c.name == "graphene-cli")
		.collect();

	for crate_info in &nodegraph_crates {
		output.push_str(&format!("        \"{}\";\n", crate_info.name));
	}
	output.push_str("    }\n\n");

	output.push_str("    subgraph cluster_node_libraries {\n");
	output.push_str("        label=\"Node Graph Libraries\";\n");
	output.push_str("        style=filled;\n");
	output.push_str("        fillcolor=lightcyan;\n");

	let node_library_crates: Vec<_> = crates
		.iter()
		.filter(|c| {
			let path_str = c.path.to_string_lossy();
			path_str.contains("node-graph/libraries")
		})
		.collect();

	for crate_info in &node_library_crates {
		output.push_str(&format!("        \"{}\";\n", crate_info.name));
	}
	output.push_str("    }\n\n");

	output.push_str("    subgraph cluster_nodes {\n");
	output.push_str("        label=\"Nodes\";\n");
	output.push_str("        style=filled;\n");
	output.push_str("        fillcolor=lightblue;\n");

	let node_crates: Vec<_> = crates
		.iter()
		.filter(|c| {
			let path_str = c.path.to_string_lossy();
			path_str.contains("node-graph/nodes")
		})
		.collect();

	for crate_info in &node_crates {
		output.push_str(&format!("        \"{}\";\n", crate_info.name));
	}
	output.push_str("    }\n\n");

	output.push_str("    subgraph cluster_desktop{\n");
	output.push_str("        label=\"Desktop\";\n");
	output.push_str("        style=filled;\n");
	output.push_str("        fillcolor=lightgreen;\n");

	let desktop_crates: Vec<_> = crates
		.iter()
		.filter(|c| {
			let path_str = c.path.to_string_lossy();
			path_str.contains("desktop")
		})
		.collect();

	for crate_info in &desktop_crates {
		output.push_str(&format!("        \"{}\";\n", crate_info.name));
	}
	output.push_str("    }\n\n");

	// Add dependencies as edges
	for crate_info in crates {
		for dep in &crate_info.dependencies {
			if dep == "dyn-any" || dep == "node-macro" {
				continue;
			}
			output.push_str(&format!("    \"{}\" -> \"{}\";\n", crate_info.name, dep));
		}
	}

	output.push_str("}\n");
	Ok(output)
}
