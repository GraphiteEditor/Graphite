//! Checks that every node version bump has a migration leading to it, and that every migration leads somewhere.

use std::collections::BTreeSet;
use std::path::Path;
use syn::parse::{Parse, ParseStream};

/// `identifier@version`.
pub(crate) type Declaration = (String, u32);

/// The source and target of one migration in `migration-nodes`.
pub(crate) struct MigrationStep {
	pub source: Declaration,
	pub target: Declaration,
}

/// `"id"@1 => @2` or `"id"@1 => "other"@2`, as `#[migration]` and `rename!` take it.
impl Parse for MigrationStep {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let identifier: syn::LitStr = input.parse()?;
		input.parse::<syn::Token![@]>()?;
		let version: syn::LitInt = input.parse()?;
		input.parse::<syn::Token![=>]>()?;

		let target_identifier = if input.peek(syn::LitStr) { input.parse::<syn::LitStr>()?.value() } else { identifier.value() };
		input.parse::<syn::Token![@]>()?;
		let target_version: syn::LitInt = input.parse()?;

		Ok(Self {
			source: (identifier.value(), version.base10_parse()?),
			target: (target_identifier, target_version.base10_parse()?),
		})
	}
}

/// Every migration step the crate at `directory` compiles, following its modules from `lib.rs` so a file no `mod` declares doesn't count.
pub(crate) fn find_migrations(directory: &Path) -> Result<Vec<MigrationStep>, String> {
	let mut steps = Vec::new();
	read_module(&directory.join("lib.rs"), directory, &mut steps)?;
	Ok(steps)
}

/// `file` is a module whose `mod child;` declarations live in `children`.
fn read_module(file: &Path, children: &Path, steps: &mut Vec<MigrationStep>) -> Result<(), String> {
	let source = std::fs::read_to_string(file).map_err(|error| format!("{}: {error}", file.display()))?;
	let parsed = syn::parse_file(&source).map_err(|error| format!("{}: {error}", file.display()))?;
	read_items(&parsed.items, file, children, steps)
}

fn read_items(items: &[syn::Item], file: &Path, children: &Path, steps: &mut Vec<MigrationStep>) -> Result<(), String> {
	for item in items {
		let step = match item {
			syn::Item::Fn(function) => match function.attrs.iter().find(|attribute| attribute.path().is_ident("migration")) {
				Some(attribute) => attribute.parse_args::<MigrationStep>(),
				None => continue,
			},
			syn::Item::Macro(call) if call.mac.path.is_ident("rename") => call.mac.parse_body::<MigrationStep>(),
			syn::Item::Mod(module) => {
				let name = module.ident.to_string();
				let directory = children.join(&name);

				match &module.content {
					Some((_, items)) => read_items(items, file, &directory, steps)?,
					None => {
						let flat = children.join(format!("{name}.rs"));
						let module_file = if flat.exists() { flat } else { directory.join("mod.rs") };
						read_module(&module_file, &directory, steps)?;
					}
				}
				continue;
			}
			_ => continue,
		};
		steps.push(step.map_err(|error| format!("{}: {error}", file.display()))?);
	}

	Ok(())
}

/// Problems with how `nodes` (current declarations) and `migrations` connect, empty when every lineage is whole.
pub(crate) fn lineage_problems(nodes: &[Declaration], migrations: &[MigrationStep]) -> Vec<String> {
	let targets: BTreeSet<&Declaration> = migrations.iter().map(|step| &step.target).collect();
	let sources: BTreeSet<&Declaration> = migrations.iter().map(|step| &step.source).collect();
	let current: BTreeSet<&Declaration> = nodes.iter().collect();

	let mut problems = Vec::new();
	for node in nodes {
		let (identifier, version) = node;
		if *version > 1 && !targets.contains(node) {
			problems.push(format!("`{identifier}` is at version {version}, but no migration leads to it. Run `cargo run migration new`"));
		}
	}

	let mut seen_sources = BTreeSet::new();
	for step in migrations {
		if !seen_sources.insert(&step.source) {
			let (source, from) = &step.source;
			problems.push(format!("two migrations start from `{source}`@{from}, which fails every document with a pending migration"));
		}
	}

	// A target is a current declaration, or a step on the way to one
	for step in migrations {
		if !current.contains(&step.target) && !sources.contains(&step.target) {
			let ((source, from), (target, to)) = (&step.source, &step.target);
			problems.push(format!(
				"the migration from `{source}`@{from} leads to `{target}`@{to}, which no node declares and no migration continues from"
			));
		}
	}

	problems
}

#[cfg(test)]
mod tests {
	use super::*;

	fn declaration(identifier: &str, version: u32) -> Declaration {
		(identifier.to_string(), version)
	}

	fn step(source: Declaration, target: Declaration) -> MigrationStep {
		MigrationStep { source, target }
	}

	#[test]
	fn parses_steps_and_replacements() {
		let version_step: MigrationStep = syn::parse_str(r#""a::B"@1 => @2"#).unwrap();
		assert_eq!((version_step.source, version_step.target), (declaration("a::B", 1), declaration("a::B", 2)));

		let replacement: MigrationStep = syn::parse_str(r#""a::B"@2 => "c::D"@3"#).unwrap();
		assert_eq!((replacement.source, replacement.target), (declaration("a::B", 2), declaration("c::D", 3)));
	}

	#[test]
	fn whole_lineages_pass() {
		let nodes = [declaration("a", 1), declaration("b", 3)];
		let migrations = [step(declaration("old", 1), declaration("b", 2)), step(declaration("b", 2), declaration("b", 3))];
		assert!(lineage_problems(&nodes, &migrations).is_empty());
	}

	#[test]
	fn bumps_need_a_migration_and_migrations_need_a_destination() {
		let nodes = [declaration("bumped", 2)];
		let migrations = [step(declaration("gone", 1), declaration("nowhere", 2))];
		let problems = lineage_problems(&nodes, &migrations);

		assert_eq!(problems.len(), 2);
		assert!(problems[0].contains("`bumped` is at version 2"));
		assert!(problems[1].contains("leads to `nowhere`@2"));
	}

	#[test]
	fn two_migrations_from_one_declaration_are_a_problem() {
		let nodes = [declaration("a", 2), declaration("b", 2)];
		let migrations = [step(declaration("a", 1), declaration("a", 2)), step(declaration("a", 1), declaration("b", 2))];
		let problems = lineage_problems(&nodes, &migrations);

		assert_eq!(problems.len(), 1);
		assert!(problems[0].contains("two migrations start from"), "{}", problems[0]);
	}
}
