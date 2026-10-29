//! Scaffolds node migrations into `migration-nodes`, one file per step, in a module per node crate.

use crate::nodes::{NodeSource, find_nodes};
use std::path::{Path, PathBuf};

const NODE_CRATES: &str = "node-graph";
const MIGRATIONS: &str = "document/migrations/nodes/src";

/// `new <identifier>` steps a node from its previous version, `new <old>@<version> <identifier>` replaces a node by another,
/// and `rename <old>@<version> <identifier>` registers a finished rename.
pub fn scaffold(workspace: &Path, command: &str, arguments: &[String]) -> Result<(), String> {
	let nodes = find_nodes(&workspace.join(NODE_CRATES))?;
	let migrations = workspace.join(MIGRATIONS);

	match (command, arguments) {
		("new", [identifier]) => {
			let node = find(&nodes, identifier)?;
			if node.version == 1 {
				return Err(format!(
					"`{}` is at version 1, so there is nothing to migrate from. Bump it with `version(2)` on its `#[node_macro::node]` in {}",
					node.identifier,
					node.file.display()
				));
			}

			let from = node.version - 1;
			let source = format!("\"{}\"@{from} => @{}", node.identifier, node.version);
			write_step(&migrations, node, &format!("{}_v{from}", node.function), &source, from)
		}
		("new", [old, identifier]) => {
			let (old_identifier, from) = parse_declaration(old)?;
			let node = find_target(&nodes, identifier, from)?;
			let source = format!("\"{old_identifier}\"@{from} => \"{}\"@{}", node.identifier, node.version);
			write_step(&migrations, node, &format!("{}_from_{}", node.function, last_segment(&old_identifier)), &source, from)
		}
		("rename", [old, identifier]) => {
			let (old_identifier, from) = parse_declaration(old)?;
			let node = find_target(&nodes, identifier, from)?;
			let line = format!("rename!(\"{old_identifier}\"@{from} => \"{}\"@{});", node.identifier, node.version);
			write_rename(&migrations, node, &line)
		}
		_ => Err("expected `new <identifier>`, `new <old identifier>@<version> <identifier>`, or `rename <old identifier>@<version> <identifier>`".to_string()),
	}
}

/// The node `query` names: its full identifier, or an unambiguous function name or identifier suffix.
fn find<'a>(nodes: &'a [NodeSource], query: &str) -> Result<&'a NodeSource, String> {
	if let Some(node) = nodes.iter().find(|node| node.identifier == query) {
		return Ok(node);
	}

	let candidates: Vec<&NodeSource> = nodes.iter().filter(|node| node.function == query || node.identifier.ends_with(&format!("::{query}"))).collect();
	match candidates.as_slice() {
		[node] => Ok(node),
		[] => Err(format!("no `#[node_macro::node]` is identified by `{query}`")),
		_ => {
			let identifiers: Vec<&str> = candidates.iter().map(|node| node.identifier.as_str()).collect();
			Err(format!("`{query}` could be any of: {}", identifiers.join(", ")))
		}
	}
}

/// The node a replacement or rename from version `from` leads to, which must be at the next version.
fn find_target<'a>(nodes: &'a [NodeSource], query: &str, from: u32) -> Result<&'a NodeSource, String> {
	let node = find(nodes, query)?;
	if Some(node.version) != from.checked_add(1) {
		return Err(format!(
			"`{}` is at version {}, but a migration from version {from} leads to version {}. Set `version({})` on its `#[node_macro::node]` in {}",
			node.identifier,
			node.version,
			from + 1,
			from + 1,
			node.file.display()
		));
	}
	Ok(node)
}

fn parse_declaration(declaration: &str) -> Result<(String, u32), String> {
	let parsed = declaration.rsplit_once('@').and_then(|(identifier, version)| Some((identifier.to_string(), version.parse().ok()?)));
	parsed.ok_or_else(|| format!("expected `<identifier>@<version>`, found `{declaration}`"))
}

fn last_segment(identifier: &str) -> String {
	let segment = identifier.rsplit("::").next().unwrap_or(identifier);
	let mut name = String::new();
	for (index, character) in segment.chars().enumerate() {
		if character.is_uppercase() && index > 0 {
			name.push('_');
		}
		name.extend(character.to_lowercase());
	}
	name
}

/// The module holding migrations of nodes from `node`'s crate, declared in the crate root.
fn crate_module(migrations: &Path, node: &NodeSource) -> Result<PathBuf, String> {
	let module = node.identifier.split("::").next().unwrap_or_default().to_string();
	declare_module(&migrations.join("lib.rs"), &module)?;

	let directory = migrations.join(&module);
	std::fs::create_dir_all(&directory).map_err(|error| format!("{}: {error}", directory.display()))?;

	let module_file = directory.join("mod.rs");
	if !module_file.exists() {
		let header = format!("//! Migrations of the nodes in `{module}`.\n");
		std::fs::write(&module_file, header).map_err(|error| format!("{}: {error}", module_file.display()))?;
	}
	Ok(directory)
}

fn write_step(migrations: &Path, node: &NodeSource, name: &str, step: &str, from: u32) -> Result<(), String> {
	if already_migrated(migrations, step) {
		return Err(format!("a migration `{step}` already exists in {}", migrations.display()));
	}

	let directory = crate_module(migrations, node)?;
	let file = directory.join(format!("{name}.rs"));
	if file.exists() {
		return Err(format!("{} already exists", file.display()));
	}

	let source = format!(
		"use migration_core::{{Inputs, migration}};\n\n\
		 /// Describe what changed from version {from}.\n\
		 #[migration({step})]\n\
		 pub fn {name}(inputs: &mut Inputs) {{\n\
		 \tcompile_error!(\"Migrate from version {from} here. Read the source state with `&Node` or `&Graph`, and edit it through `&mut Inputs`, `&mut Upstream`, or `&mut Downstream`\");\n\
		 }}\n"
	);
	std::fs::write(&file, source).map_err(|error| format!("{}: {error}", file.display()))?;
	declare_module(&directory.join("mod.rs"), name)?;

	println!("Scaffolded `{step}` in {}", file.display());
	println!("Write the migration where its `compile_error!` points, and describe the change in its doc comment");
	Ok(())
}

fn write_rename(migrations: &Path, node: &NodeSource, line: &str) -> Result<(), String> {
	let directory = crate_module(migrations, node)?;
	let file = directory.join("renames.rs");

	let existing = std::fs::read_to_string(&file).unwrap_or_else(|_| "//! Nodes that only moved to a new identifier.\n\nuse migration_core::rename;\n\n".to_string());
	if existing.lines().any(|existing_line| existing_line == line) {
		return Err(format!("{} already has `{line}`", file.display()));
	}

	std::fs::write(&file, format!("{existing}{line}\n")).map_err(|error| format!("{}: {error}", file.display()))?;
	declare_module(&directory.join("mod.rs"), "renames")?;

	println!("Registered `{line}` in {}", file.display());
	Ok(())
}

/// Whether some migration file already registers `step`.
fn already_migrated(directory: &Path, step: &str) -> bool {
	let Ok(entries) = std::fs::read_dir(directory) else { return false };
	entries.filter_map(Result::ok).any(|entry| {
		let path = entry.path();
		if path.is_dir() {
			return already_migrated(&path, step);
		}
		std::fs::read_to_string(&path).is_ok_and(|source| source.contains(&format!("#[migration({step})]")))
	})
}

/// Adds `mod <module>;` to `file` unless it's there, keeping the declarations sorted after the header.
fn declare_module(file: &Path, module: &str) -> Result<(), String> {
	let source = std::fs::read_to_string(file).map_err(|error| format!("{}: {error}", file.display()))?;
	let declaration = format!("mod {module};");
	if source.lines().any(|line| line == declaration) {
		return Ok(());
	}

	let mut lines: Vec<String> = source.lines().map(str::to_string).collect();
	let mut declarations: Vec<String> = lines.iter().filter(|line| line.starts_with("mod ")).cloned().chain(std::iter::once(declaration)).collect();
	declarations.sort();
	lines.retain(|line| !line.starts_with("mod "));
	while lines.last().is_some_and(|line| line.is_empty()) {
		lines.pop();
	}

	let updated = format!("{}\n\n{}\n", lines.join("\n"), declarations.join("\n"));
	std::fs::write(file, updated).map_err(|error| format!("{}: {error}", file.display()))
}

#[cfg(test)]
mod tests {
	use super::*;

	/// A workspace with one node crate and an empty `migration-nodes`, removed on drop.
	struct Workspace(PathBuf);

	impl Workspace {
		fn new(name: &str) -> Self {
			let root = std::env::temp_dir().join(format!("migration-scaffold-{name}-{}", std::process::id()));
			let nodes = root.join("node-graph/nodes/blending");
			std::fs::create_dir_all(nodes.join("src")).unwrap();
			std::fs::write(nodes.join("Cargo.toml"), "[package]\nname = \"blending-nodes\"\n").unwrap();
			std::fs::write(
				nodes.join("src/lib.rs"),
				"mod adjust;\n#[node_macro::node(category(\"Blending\"), version(2))]\nfn opacity(value: f64) -> f64 { value }\n#[node_macro::node(category(\"Blending\"))]\nfn blend(value: f64) -> f64 { value }\n",
			)
			.unwrap();
			std::fs::write(
				nodes.join("src/adjust.rs"),
				"mod inner {\n#[node_macro::node(category(\"Blending\"), path(legacy::adjust), version(3))]\nfn brighten(value: f64) -> f64 { value }\n}\n",
			)
			.unwrap();

			std::fs::create_dir_all(root.join(MIGRATIONS)).unwrap();
			std::fs::write(root.join(MIGRATIONS).join("lib.rs"), "//! Node migrations.\n").unwrap();
			Self(root)
		}

		fn read(&self, path: &str) -> String {
			std::fs::read_to_string(self.0.join(MIGRATIONS).join(path)).unwrap()
		}
	}

	impl Drop for Workspace {
		fn drop(&mut self) {
			let _ = std::fs::remove_dir_all(&self.0);
		}
	}

	fn arguments(values: &[&str]) -> Vec<String> {
		values.iter().map(|value| value.to_string()).collect()
	}

	#[test]
	fn finds_identifiers_and_versions_like_the_macro() {
		let workspace = Workspace::new("find");
		let nodes = find_nodes(&workspace.0.join(NODE_CRATES)).unwrap();
		let found: Vec<(&str, u32)> = nodes.iter().map(|node| (node.identifier.as_str(), node.version)).collect();

		assert_eq!(found, [("blending_nodes::BlendNode", 1), ("blending_nodes::OpacityNode", 2), ("legacy::adjust::BrightenNode", 3)]);
	}

	#[test]
	fn new_scaffolds_the_step_from_the_previous_version() {
		let workspace = Workspace::new("new");
		scaffold(&workspace.0, "new", &arguments(&["opacity"])).unwrap();

		assert!(workspace.read("lib.rs").ends_with("\n\nmod blending_nodes;\n"));
		assert!(workspace.read("blending_nodes/mod.rs").contains("mod opacity_v1;"));
		let step = workspace.read("blending_nodes/opacity_v1.rs");
		assert!(step.contains("#[migration(\"blending_nodes::OpacityNode\"@1 => @2)]\npub fn opacity_v1(inputs: &mut Inputs) {"));
		assert!(step.contains("compile_error!"));

		let again = scaffold(&workspace.0, "new", &arguments(&["blending_nodes::OpacityNode"])).unwrap_err();
		assert!(again.contains("already exists"), "{again}");
	}

	#[test]
	fn new_refuses_a_node_at_version_one() {
		let workspace = Workspace::new("first");
		let error = scaffold(&workspace.0, "new", &arguments(&["blend"])).unwrap_err();
		assert!(error.contains("Bump it with `version(2)`"), "{error}");
	}

	#[test]
	fn replacements_and_renames_lead_to_the_next_version() {
		let workspace = Workspace::new("rename");

		scaffold(&workspace.0, "new", &arguments(&["legacy::Brighten@2", "BrightenNode"])).unwrap();
		assert!(
			workspace
				.read("legacy/brighten_from_brighten.rs")
				.contains("#[migration(\"legacy::Brighten\"@2 => \"legacy::adjust::BrightenNode\"@3)]")
		);

		scaffold(&workspace.0, "rename", &arguments(&["graphene_core::Opacity@1", "opacity"])).unwrap();
		assert!(
			workspace
				.read("blending_nodes/renames.rs")
				.contains("rename!(\"graphene_core::Opacity\"@1 => \"blending_nodes::OpacityNode\"@2);")
		);
		assert!(workspace.read("blending_nodes/mod.rs").contains("mod renames;"));

		let skipped = scaffold(&workspace.0, "rename", &arguments(&["graphene_core::Blend@3", "blend"])).unwrap_err();
		assert!(skipped.contains("Set `version(4)`"), "{skipped}");
	}
}
