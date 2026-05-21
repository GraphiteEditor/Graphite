use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Deserialize)]
struct WorkspaceToml {
	workspace: WorkspaceConfig,
}

#[derive(Debug, Deserialize)]
struct WorkspaceConfig {
	members: Vec<String>,
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
		#[serde(flatten)]
		other: HashMap<String, toml::Value>,
	},
}

struct CrateInfo {
	name: String,
	path: PathBuf,
	dependencies: Vec<String>,
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
	let output_dir = std::env::args_os()
		.nth(1)
		.map(PathBuf::from)
		.ok_or_else(|| anyhow::anyhow!("Usage: crate-hierarchy-viz <output-directory>"))?;
	let output_path = output_dir.join("crate-hierarchy.svg");

	let workspace_root = std::env::current_dir()?;
	let workspace_toml_path = workspace_root.join("Cargo.toml");

	// Parse workspace Cargo.toml
	let workspace_content = fs::read_to_string(&workspace_toml_path).with_context(|| format!("Failed to read {:?}", workspace_toml_path))?;
	let workspace_toml: WorkspaceToml = toml::from_str(&workspace_content).with_context(|| "Failed to parse workspace Cargo.toml")?;

	// Expand glob patterns in workspace members (e.g., "node-graph/libraries/*")
	let mut resolved_members = Vec::new();
	let mut seen_members = HashSet::new();
	let abs_root = workspace_root.canonicalize().unwrap_or_else(|_| workspace_root.clone());
	for member in &workspace_toml.workspace.members {
		if member.contains('*') {
			let pattern = abs_root.join(member).to_string_lossy().to_string();
			let matched: Vec<_> = glob::glob(&pattern)
				.with_context(|| format!("Failed to expand glob pattern: {member}"))?
				.filter_map(|entry| entry.ok())
				.filter_map(|path| path.strip_prefix(&abs_root).ok().map(|p| p.to_string_lossy().to_string()))
				.collect();
			if matched.is_empty() {
				eprintln!("Warning: No matches for glob pattern: {member}");
			}
			for m in matched {
				let normalized = m.replace('\\', "/");
				if seen_members.insert(normalized.clone()) {
					resolved_members.push(normalized);
				}
			}
		} else {
			let normalized = member.replace('\\', "/");
			if seen_members.insert(normalized.clone()) {
				resolved_members.push(normalized);
			}
		}
	}

	// Parse each member crate's Cargo.toml
	let mut parsed_crates = Vec::new();
	for member in &resolved_members {
		let crate_path = workspace_root.join(member);
		let cargo_toml_path = crate_path.join("Cargo.toml");

		if !cargo_toml_path.exists() {
			eprintln!("Warning: Cargo.toml not found for member: {}", member);
			continue;
		}

		let crate_content = fs::read_to_string(&cargo_toml_path).with_context(|| format!("Failed to read {:?}", cargo_toml_path))?;
		let crate_toml: CrateToml = toml::from_str(&crate_content).with_context(|| format!("Failed to parse Cargo.toml for {}", member))?;

		parsed_crates.push((crate_path, crate_toml));
	}

	// Collect all workspace crate names
	let workspace_crate_names: HashSet<String> = parsed_crates.iter().map(|(_, toml)| toml.package.name.clone()).collect();

	// Build dependency graph, keeping only workspace-internal dependencies
	let mut crates: Vec<CrateInfo> = parsed_crates
		.into_iter()
		.map(|(path, crate_toml)| {
			let dependencies = crate_toml
				.dependencies
				.unwrap_or_default()
				.into_iter()
				.filter_map(|(dep_name, dep_config)| {
					// Resolve the actual package name (handles renamed dependencies)
					let actual_name = match &dep_config {
						CrateDependency::Detailed { other, .. } => other.get("package").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or(dep_name),
						CrateDependency::Simple(_) => dep_name,
					};
					// Only keep dependencies that are workspace crates
					workspace_crate_names.contains(&actual_name).then_some(actual_name)
				})
				.collect();

			CrateInfo {
				name: crate_toml.package.name,
				path,
				dependencies,
			}
		})
		.collect();

	remove_transitive_dependencies(&mut crates);

	// Generate DOT format, convert to SVG, and write to output file
	let dot_content = generate_dot(&crates);
	let svg_content = dot_to_svg(&dot_content)?;

	fs::create_dir_all(&output_dir).with_context(|| format!("Failed to create directory {:?}", output_dir))?;
	fs::write(&output_path, &svg_content).with_context(|| format!("Failed to write to {:?}", output_path))?;

	Ok(())
}

/// Convert a DOT graph string to SVG by shelling out to @viz-js/viz via Node.js
fn dot_to_svg(dot: &str) -> Result<String> {
	let temp_dir = std::env::temp_dir().join("crate-hierarchy-viz");
	fs::create_dir_all(&temp_dir).with_context(|| "Failed to create temp directory")?;

	// Install @viz-js/viz into the temp directory if not already present
	let viz_package = temp_dir.join("node_modules").join("@viz-js").join("viz");
	if !viz_package.exists() {
		let npm = if cfg!(target_os = "windows") { "npm.cmd" } else { "npm" };
		let status = Command::new(npm)
			.args(["install", "--prefix", &temp_dir.to_string_lossy(), "@viz-js/viz"])
			.stdout(std::process::Stdio::null())
			.stderr(std::process::Stdio::piped())
			.status()
			.with_context(|| "Failed to run `npm install`. Is Node.js installed?")?;
		if !status.success() {
			anyhow::bail!("Executing `npm install @viz-js/viz` failed");
		}
	}

	// Write a small script that reads DOT from stdin and outputs SVG
	let script_path = temp_dir.join("convert.mjs");
	fs::write(
		&script_path,
		r#"
		import { instance } from "@viz-js/viz";
		let dot = "";
		for await (const chunk of process.stdin) dot += chunk;
		const viz = await instance();
		process.stdout.write(viz.renderString(dot, { format: "svg" }));
		"#
		.trim(),
	)?;

	let mut child = Command::new("node")
		.arg(&script_path)
		.stdin(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.stderr(std::process::Stdio::piped())
		.spawn()
		.with_context(|| "Failed to spawn `node`. Is Node.js installed?")?;

	// Write DOT content to stdin then close the pipe
	child
		.stdin
		.take()
		.context("Failed to get stdin handle for node process")?
		.write_all(dot.as_bytes())
		.with_context(|| "Failed to write DOT content to stdin")?;

	let output = child.wait_with_output().with_context(|| "Failed to wait for `node` process")?;

	// Clean up the temp script (node_modules is intentionally kept as a cache)
	let _ = fs::remove_file(&script_path);

	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr);
		anyhow::bail!("DOT to SVG conversion failed (exit code {:?}):\n{}", output.status.code(), stderr);
	}

	String::from_utf8(output.stdout).with_context(|| "SVG output was not valid UTF-8")
}

fn generate_dot(crates: &[CrateInfo]) -> String {
	let mut out = String::new();
	out.push_str("digraph CrateHierarchy {\n");
	out.push_str("    rankdir=LR;\n");
	out.push_str("    node [shape=box, style=\"rounded,filled\", fillcolor=lightblue];\n");
	out.push_str("    edge [color=gray];\n\n");

	// Define subgraph clusters
	let clusters: &[(&str, &str, &str, Box<dyn Fn(&CrateInfo) -> bool>)] = &[
		(
			"cluster_core",
			"Core Components",
			"lightgray",
			Box::new(|c| (c.name.starts_with("graphite-") || c.name == "editor" || c.name == "graphene-cli") && !c.name.contains("desktop")),
		),
		(
			"cluster_nodegraph",
			"Node Graph System",
			"lightyellow",
			Box::new(|c| c.name == "graph-craft" || c.name == "interpreted-executor" || c.name == "node-macro" || c.name == "preprocessor" || c.name == "graphene-cli"),
		),
		(
			"cluster_node_libraries",
			"Node Graph Libraries",
			"lightcyan",
			Box::new(|c| c.path.to_string_lossy().replace('\\', "/").contains("node-graph/libraries")),
		),
		(
			"cluster_nodes",
			"Nodes",
			"lightblue",
			Box::new(|c| c.path.to_string_lossy().replace('\\', "/").contains("node-graph/nodes")),
		),
		(
			"cluster_desktop",
			"Desktop",
			"lightgreen",
			Box::new(|c| c.path.to_string_lossy().replace('\\', "/").contains("desktop")),
		),
	];

	for (id, label, color, filter) in clusters {
		out.push_str(&format!("    subgraph {id} {{\n"));
		out.push_str(&format!("        label=\"{label}\";\n"));
		out.push_str("        style=filled;\n");
		out.push_str(&format!("        fillcolor={color};\n"));
		for c in crates.iter().filter(|c| filter(c)) {
			out.push_str(&format!("        \"{}\";\n", c.name));
		}
		out.push_str("    }\n\n");
	}

	// Add dependency edges
	for crate_info in crates {
		for dep in &crate_info.dependencies {
			if dep == "dyn-any" || dep == "node-macro" {
				continue;
			}
			out.push_str(&format!("    \"{}\" -> \"{}\";\n", crate_info.name, dep));
		}
	}

	out.push_str("}\n");
	out
}
