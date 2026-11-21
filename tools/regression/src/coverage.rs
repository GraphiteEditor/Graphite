//! Which of the nodes this build registers appear somewhere in the corpus's documents.

use crate::corpus::Corpus;
use graph_craft::document::DocumentNodeImplementation;
use graphene_cli::open_document;
use graphene_std::registry::{NODE_METADATA, category_sort_key, is_hidden_category};
use std::collections::HashSet;
use std::error::Error;

/// Print how many of the registered nodes the corpus uses, and with `list_uncovered`, the ones it doesn't.
/// Nodes in hidden categories never appear in the node catalog, so they only count with `include_hidden`.
pub(crate) async fn coverage(corpus: &Corpus, list_uncovered: bool, include_hidden: bool) -> Result<(), Box<dyn Error>> {
	let mut used = HashSet::new();
	for entry in &corpus.manifest.documents {
		let document = open_document(&corpus.document(&entry.name), false).await.map_err(|error| format!("{}: {error}", entry.name))?;
		let identifiers = document.network.recursive_nodes().filter_map(|(_, node, _)| match &node.implementation {
			DocumentNodeImplementation::ProtoNode(identifier) => Some(identifier.as_str().to_string()),
			_ => None,
		});
		used.extend(identifiers);
	}

	let metadata = NODE_METADATA.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
	let counted: Vec<_> = metadata.iter().filter(|(_, metadata)| include_hidden || !is_hidden_category(metadata.category)).collect();
	let mut uncovered: Vec<_> = counted.iter().filter(|(identifier, _)| !used.contains(identifier.as_str())).collect();

	let total = counted.len();
	let covered = total - uncovered.len();
	let percentage = if total == 0 { 0. } else { covered as f64 / total as f64 * 100. };
	println!("The corpus uses {covered} of {total} nodes ({percentage:.1}%)");

	if list_uncovered {
		uncovered.sort_by(|(_, a), (_, b)| category_sort_key(a.category).cmp(&category_sort_key(b.category)).then(a.display_name.cmp(b.display_name)));
		let mut category = None;
		for (identifier, metadata) in uncovered {
			if category != Some(metadata.category) {
				category = Some(metadata.category);
				println!("\n{}", if metadata.category.is_empty() { "Uncategorized" } else { metadata.category });
			}
			println!("  {}  ({})", metadata.display_name, identifier.as_str());
		}
	}
	Ok(())
}
