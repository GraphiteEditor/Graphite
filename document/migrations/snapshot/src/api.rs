//! Compares every node's API against a baseline commit, to find changes that need a version bump or a migration.

use crate::lineage::{MigrationStep, find_migrations};
use crate::nodes::{NodeSource, find_nodes};
use crate::scaffold::{MIGRATIONS, NODE_CRATES};
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};

/// Fail if the nodes at the git ref `baseline` differ from the working tree's in a way documents would notice.
pub(crate) fn check_against(workspace: &Path, baseline: &str) -> Result<(), String> {
	let current = find_nodes(&workspace.join(NODE_CRATES))?;
	let migrations = find_migrations(&workspace.join(MIGRATIONS))?;

	let directory = std::env::temp_dir().join(format!("migration-api-baseline-{}", std::process::id()));
	let baseline_nodes = extract_baseline(workspace, baseline, &directory).and_then(|()| find_nodes(&directory.join(NODE_CRATES)));
	// Removal failing only leaves a temporary directory behind
	let _ = std::fs::remove_dir_all(&directory);
	let baseline_nodes = baseline_nodes?;

	let problems = api_problems(&baseline_nodes, &current, &migrations);
	if !problems.is_empty() {
		return Err(problems.join("\n"));
	}

	println!("No node API changed without a version bump since {baseline} ({} nodes)", baseline_nodes.len());
	Ok(())
}

fn extract_baseline(workspace: &Path, baseline: &str, directory: &Path) -> Result<(), String> {
	std::fs::create_dir_all(directory).map_err(|error| format!("{}: {error}", directory.display()))?;

	let mut archive = Command::new("git")
		.args(["archive", baseline, NODE_CRATES])
		.current_dir(workspace)
		.stdout(Stdio::piped())
		.spawn()
		.map_err(|error| format!("running git: {error}"))?;
	let Some(tar_input) = archive.stdout.take() else {
		return Err("git archive has no output".to_string());
	};

	let extracted = Command::new("tar")
		.args(["-x", "-C"])
		.arg(directory)
		.stdin(tar_input)
		.status()
		.map_err(|error| format!("running tar: {error}"))?;
	let archived = archive.wait().map_err(|error| format!("running git: {error}"))?;
	if !archived.success() || !extracted.success() {
		return Err(format!("could not read `{NODE_CRATES}` at the git ref `{baseline}`"));
	}
	Ok(())
}

/// Why `current` needs a version bump or a migration that it doesn't have, given the nodes at the baseline.
fn api_problems(baseline: &[NodeSource], current: &[NodeSource], migrations: &[MigrationStep]) -> Vec<String> {
	let current_by_identifier: HashMap<&str, &NodeSource> = current.iter().map(|node| (node.identifier.as_str(), node)).collect();
	let baseline_identifiers: Vec<&str> = baseline.iter().map(|node| node.identifier.as_str()).collect();

	let mut problems = Vec::new();
	for old in baseline {
		let identifier = &old.identifier;
		let version = old.version;

		let Some(new) = current_by_identifier.get(identifier.as_str()) else {
			if migrations.iter().any(|step| step.source == (identifier.clone(), version)) {
				continue;
			}

			// Moving a module keeps the function and its API, which is what a rename looks like
			let moved = current
				.iter()
				.find(|new| !baseline_identifiers.contains(&new.identifier.as_str()) && new.function == old.function && new.signature == old.signature);
			problems.push(match moved {
				Some(new) => format!(
					"`{identifier}`@{version} disappeared and looks moved to `{}`. Set `version({})` on it and run `cargo run migration rename {identifier}@{version} {}`",
					new.identifier,
					version + 1,
					new.identifier
				),
				None => format!("`{identifier}`@{version} disappeared without a migration. Run `cargo run migration new {identifier}@{version} <replacing node>`"),
			});
			continue;
		};

		if new.version < version {
			problems.push(format!("`{identifier}` went back from version {version} to {}", new.version));
		} else if new.version > version {
			// Stored nodes start at the baseline version, so migrations have to lead from it to the new one, and a migration reaching the new version from another node isn't one
			let mut reached = (identifier.clone(), version);
			for _ in version..new.version {
				if let Some(migration) = migrations.iter().find(|migration| migration.source == reached) {
					reached = migration.target.clone();
				}
			}
			if reached != (identifier.clone(), new.version) {
				problems.push(format!(
					"`{identifier}` went from version {version} to {}, but no migration starts from version {}. Run `cargo run migration new {identifier}`, or `rename` it to itself if nothing changed for stored nodes",
					new.version, reached.1
				));
			}
		} else if new.version == version && new.signature != old.signature {
			problems.push(format!(
				"`{identifier}` changed its API without a version bump. Set `version({})` on it in {} and run `cargo run migration new`\n  was: {}\n  now: {}",
				version + 1,
				new.file.display(),
				old.signature,
				new.signature
			));
		}
	}

	problems
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::PathBuf;

	fn node(identifier: &str, version: u32, signature: &str) -> NodeSource {
		NodeSource {
			identifier: identifier.to_string(),
			version,
			function: identifier.rsplit("::").next().unwrap_or(identifier).to_string(),
			file: PathBuf::from("lib.rs"),
			signature: signature.to_string(),
		}
	}

	#[test]
	fn unchanged_nodes_and_bumped_ones_pass() {
		let baseline = [node("a::b", 1, "(x: f64)"), node("a::c", 1, "(x: f64)")];
		let current = [node("a::b", 1, "(x: f64)"), node("a::c", 2, "(x: f64, y: f64)")];
		let migrations = [MigrationStep {
			source: ("a::c".to_string(), 1),
			target: ("a::c".to_string(), 2),
		}];
		assert!(api_problems(&baseline, &current, &migrations).is_empty());
	}

	#[test]
	fn a_changed_api_needs_a_bump() {
		let problems = api_problems(&[node("a::b", 1, "(x: f64)")], &[node("a::b", 1, "(x: f64, y: f64)")], &[]);
		assert_eq!(problems.len(), 1);
		assert!(problems[0].contains("Set `version(2)`"), "{}", problems[0]);
	}

	#[test]
	fn a_moved_node_suggests_a_rename_until_it_has_a_migration() {
		let baseline = [node("old::b", 3, "(x: f64)")];
		let current = [node("new::b", 3, "(x: f64)")];

		let problems = api_problems(&baseline, &current, &[]);
		assert!(problems[0].contains("rename old::b@3 new::b"), "{}", problems[0]);

		let migrations = [MigrationStep {
			source: ("old::b".to_string(), 3),
			target: ("new::b".to_string(), 4),
		}];
		assert!(api_problems(&baseline, &current, &migrations).is_empty());
	}

	#[test]
	fn a_removed_node_needs_a_migration() {
		let problems = api_problems(&[node("a::gone", 1, "(x: f64)")], &[], &[]);
		assert!(problems[0].contains("disappeared without a migration"), "{}", problems[0]);
	}

	#[test]
	fn a_bump_needs_a_migration_from_the_baseline_version() {
		let bumped = [node("a::b", 3, "(x: f64)")];
		let from_second_only = [MigrationStep {
			source: ("a::b".to_string(), 2),
			target: ("a::b".to_string(), 3),
		}];
		let problems = api_problems(&[node("a::b", 1, "(x: f64)")], &bumped, &from_second_only);
		assert_eq!(problems.len(), 1);
		assert!(problems[0].contains("no migration starts from version 1"), "{}", problems[0]);

		let from_another_node = [MigrationStep {
			source: ("a::old".to_string(), 1),
			target: ("a::b".to_string(), 2),
		}];
		assert!(!api_problems(&[node("a::b", 1, "(x: f64)")], &[node("a::b", 2, "(x: f64)")], &from_another_node).is_empty());

		let renamed_and_back = [
			MigrationStep {
				source: ("a::b".to_string(), 1),
				target: ("a::c".to_string(), 2),
			},
			MigrationStep {
				source: ("a::c".to_string(), 2),
				target: ("a::b".to_string(), 3),
			},
		];
		assert!(api_problems(&[node("a::b", 1, "(x: f64)")], &bumped, &renamed_and_back).is_empty());
	}
}
