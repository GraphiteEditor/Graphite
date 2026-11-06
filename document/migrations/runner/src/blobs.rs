//! Node migrations read and write values, not the blobs a document stores large ones in, so each run sees a state with its blobs decoded
//! and its output moves large values back into blobs. A value the run left alone encodes to the blob it came from, so it diffs as unchanged.

use crate::RunnerError;
use document_graph_storage::blob::{BlobBytes, NewBlobs, decode_blob};
use document_graph_storage::{BlobHash, NodeId, NodeInput, Registry, Value};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;

/// The blobs a document stores, and those migrations moved new values into.
pub struct MigrationBlobs<'a> {
	stored: &'a BlobBytes,
	/// Blobs no stored one already holds.
	added: BlobBytes,
	/// For each added blob, the blob its input slot held before the migration, as the base a store can encode it against.
	bases: HashMap<BlobHash, BlobHash>,
}

/// For a document without blobs.
impl Default for MigrationBlobs<'static> {
	fn default() -> Self {
		static NONE: LazyLock<BlobBytes> = LazyLock::new(BlobBytes::new);
		Self::new(&NONE)
	}
}

impl<'a> MigrationBlobs<'a> {
	/// Resolve through the blobs a document `stored`.
	pub fn new(stored: &'a BlobBytes) -> Self {
		Self {
			stored,
			added: BlobBytes::new(),
			bases: HashMap::new(),
		}
	}

	/// The blobs migrations added.
	pub fn into_added(self) -> NewBlobs {
		NewBlobs { bytes: self.added, bases: self.bases }
	}

	/// `registry` with the blobs in the inputs of `nodes`, or of every node for `None`, decoded back into values.
	pub(crate) fn resolve<'r>(&self, registry: &'r Registry, nodes: Option<&[NodeId]>) -> Result<Cow<'r, Registry>, RunnerError> {
		let ids: Vec<NodeId> = match nodes {
			Some(nodes) => nodes.to_vec(),
			None => registry.node_instances.keys().copied().collect(),
		};
		let holds_blob = |id: &NodeId| registry.node_instances.get(id).is_some_and(|node| node.inputs().iter().any(|slot| slot.input.blob().is_some()));
		if !ids.iter().any(holds_blob) {
			return Ok(Cow::Borrowed(registry));
		}

		let mut resolved = registry.clone();
		for id in ids {
			let Some(node) = resolved.node_instances.get_mut(&id) else { continue };
			for slot in node.inputs_mut() {
				let NodeInput::Value { value, .. } = &mut slot.input else { continue };
				let Value::Blob(hash) = *value else { continue };

				let bytes = self.stored.get(&hash).or_else(|| self.added.get(&hash)).ok_or(RunnerError::MissingBlob(hash))?;
				*value = decode_blob(bytes).map_err(|_| RunnerError::MissingBlob(hash))?;
			}
		}

		Ok(Cow::Owned(resolved))
	}

	/// Move the large values of `migrated` back into blobs, basing each new one on the blob its slot held in `source`.
	pub(crate) fn store(&mut self, migrated: &mut Registry, source: &Registry) {
		let mut bytes = BlobBytes::new();
		migrated.move_values_into_blobs(&mut bytes);
		bytes.retain(|hash, _| !self.stored.contains_key(hash) && !self.added.contains_key(hash));
		if bytes.is_empty() {
			return;
		}

		for (hash, base) in migrated.blob_bases_since(source) {
			if bytes.contains_key(&hash) {
				self.bases.insert(hash, base);
			}
		}
		self.added.extend(bytes);
	}
}
