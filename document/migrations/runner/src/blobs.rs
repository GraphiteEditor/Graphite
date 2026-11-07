//! Node migrations work on values, not blobs. Each run sees its blobs decoded, and its output moves large values back into blobs.
//! An unchanged value encodes to the same blob, so it does not show up in the diff.

use crate::RunnerError;
use document_graph_storage::blob::{BlobBytes, NewBlobs, decode_blob};
use document_graph_storage::{BlobHash, NodeId, NodeInput, Registry, Value};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;

/// The blobs of a document, plus the new blobs that migrations create.
pub struct MigrationBlobs<'a> {
	stored: &'a BlobBytes,
	/// New blobs that are not stored yet.
	added: BlobBytes,
	/// The base of each added blob. The base is the blob its input slot held before the migration.
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
	/// Resolve values from the `stored` blobs of a document.
	pub fn new(stored: &'a BlobBytes) -> Self {
		Self {
			stored,
			added: BlobBytes::new(),
			bases: HashMap::new(),
		}
	}

	/// The blobs that migrations added.
	pub fn into_added(self) -> NewBlobs {
		NewBlobs { bytes: self.added, bases: self.bases }
	}

	/// `registry` with the blobs in the inputs of `nodes` decoded back into values. `None` decodes every node.
	pub(crate) fn resolve<'r>(&self, registry: &'r Registry, nodes: Option<&[NodeId]>) -> Result<Cow<'r, Registry>, RunnerError> {
		let ids: Cow<[NodeId]> = match nodes {
			Some(nodes) => Cow::Borrowed(nodes),
			None => Cow::Owned(registry.node_instances.keys().copied().collect()),
		};
		let holds_blob = |id: &NodeId| registry.node_instances.get(id).is_some_and(|node| node.inputs().iter().any(|slot| slot.input.blob().is_some()));
		if !ids.iter().any(holds_blob) {
			return Ok(Cow::Borrowed(registry));
		}

		let mut resolved = registry.clone();
		for id in ids.iter() {
			let Some(node) = resolved.node_instances.get_mut(id) else { continue };
			for slot in node.inputs_mut() {
				let NodeInput::Value { value, .. } = &mut slot.input else { continue };
				let Value::Blob(hash) = *value else { continue };

				let bytes = self.stored.get(&hash).or_else(|| self.added.get(&hash)).ok_or(RunnerError::MissingBlob(hash))?;
				*value = decode_blob(bytes).map_err(|_| RunnerError::MissingBlob(hash))?;
			}
		}

		Ok(Cow::Owned(resolved))
	}

	/// Move the large values of `migrated` back into blobs. A new blob is based on the blob its slot held in `source`.
	pub(crate) fn store(&mut self, migrated: &mut Registry, source: &Registry) {
		let mut bytes = BlobBytes::new();
		migrated.move_values_into_blobs(&mut bytes);
		bytes.retain(|hash, _| !self.stored.contains_key(hash) && !self.added.contains_key(hash));
		if bytes.is_empty() {
			return;
		}

		self.bases.extend(migrated.blob_bases_since(source).into_iter().filter(|(hash, _)| bytes.contains_key(hash)));
		self.added.extend(bytes);
	}
}
