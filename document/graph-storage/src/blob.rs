//! Large node input values are stored as blobs outside the registry, history and hot log. The input keeps a
//! [`Value::Blob`] with the hash. Blob bytes are the postcard encoding of the value, so equal values share one blob.

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::{BlobHash, NodeId, NodeInput, Registry, RegistryDelta, ResourceHash, Value};

/// Values whose encoding reaches this many bytes are stored as blobs.
pub const BLOB_THRESHOLD: usize = 1024;

/// Blob bytes by hash.
pub type BlobBytes = HashMap<BlobHash, Vec<u8>>;

/// Blobs to store, with the base blob that each one can be encoded against.
#[derive(Clone, Debug, Default)]
pub struct NewBlobs {
	pub bytes: BlobBytes,
	pub bases: HashMap<BlobHash, BlobHash>,
}

impl BlobHash {
	/// The hash of a blob holding `bytes`.
	pub fn of(bytes: &[u8]) -> Self {
		Self(ResourceHash::from(bytes))
	}
}

impl fmt::Display for BlobHash {
	fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		self.0.fmt(formatter)
	}
}

/// Blobs are content addressed like resources, so a peer can fetch a blob through the resource path.
impl From<BlobHash> for ResourceHash {
	fn from(hash: BlobHash) -> Self {
		hash.0
	}
}

/// The value a blob's bytes encode.
pub fn decode_blob(bytes: &[u8]) -> Result<Value, postcard::Error> {
	postcard::from_bytes(bytes)
}

impl NodeInput {
	/// Replace a value of at least [`BLOB_THRESHOLD`] encoded bytes with a blob. The blob bytes are added to `bytes`.
	pub fn move_into_blob(&mut self, bytes: &mut BlobBytes) {
		let NodeInput::Value { value, .. } = self else { return };
		if matches!(value, Value::Blob(_)) {
			return;
		}

		// Encoding into memory can't fail
		let Ok(encoded) = postcard::to_stdvec(value) else { return };
		if encoded.len() < BLOB_THRESHOLD {
			return;
		}

		let hash = BlobHash::of(&encoded);
		*value = Value::Blob(hash);
		bytes.insert(hash, encoded);
	}
}

impl RegistryDelta {
	/// Move the large values this op writes into blobs. See [`NodeInput::move_into_blob`].
	pub fn move_values_into_blobs(&mut self, bytes: &mut BlobBytes) {
		let slots = match self {
			RegistryDelta::ChangeNodeInput { new_input, .. } => {
				new_input.move_into_blob(bytes);
				return;
			}
			RegistryDelta::AddNode { node, .. } => &mut node.inputs,
			RegistryDelta::SetNodeInputs { inputs, .. } => inputs,
			_ => return,
		};

		for slot in slots {
			slot.input.move_into_blob(bytes);
		}
	}
}

impl Registry {
	/// Move the large values of every node input into blobs. See [`NodeInput::move_into_blob`].
	pub fn move_values_into_blobs(&mut self, bytes: &mut BlobBytes) {
		for slot in self.node_instances.values_mut().flat_map(|node| &mut node.inputs) {
			slot.input.move_into_blob(bytes);
		}
	}

	/// Every blob that live nodes and tombstones of removed nodes name.
	pub fn blob_hashes(&self) -> impl Iterator<Item = BlobHash> + '_ {
		let live = self.node_instances.values();
		let removed = self.removed_nodes.values().map(|tombstone| &tombstone.content);
		live.chain(removed).flat_map(|node| node.inputs()).filter_map(|slot| slot.input.blob())
	}

	/// The base of each blob that `ops` write. The base is the blob that the same input slot held before.
	pub fn blob_bases(&self, ops: &[RegistryDelta]) -> HashMap<BlobHash, BlobHash> {
		let mut latest: HashMap<(NodeId, usize), BlobHash> = HashMap::new();
		let mut written = HashSet::new();
		let mut bases = HashMap::new();

		let mut write = |id: NodeId, index: usize, input: &NodeInput| {
			let Some(hash) = input.blob() else { return };

			// Only the first write of a blob takes a base. This keeps bases acyclic
			if written.insert(hash)
				&& let Some(previous) = latest.get(&(id, index)).copied().or_else(|| self.input_blob(id, index))
				&& previous != hash
			{
				bases.insert(hash, previous);
			}
			latest.insert((id, index), hash);
		};

		for op in ops {
			match op {
				RegistryDelta::ChangeNodeInput { id, index, new_input } => write(*id, *index as usize, new_input),
				RegistryDelta::SetNodeInputs { id, inputs } => inputs.iter().enumerate().for_each(|(index, slot)| write(*id, index, &slot.input)),
				_ => {}
			}
		}

		bases
	}

	/// Like [`blob_bases`](Self::blob_bases), but compares the slots of `self` with the same slots in `previous`.
	pub fn blob_bases_since(&self, previous: &Registry) -> HashMap<BlobHash, BlobHash> {
		let mut nodes: Vec<_> = self.node_instances.iter().collect();
		nodes.sort_unstable_by_key(|(id, _)| **id);

		let mut written = HashSet::new();
		let mut bases = HashMap::new();
		for (id, node) in nodes {
			for (index, slot) in node.inputs().iter().enumerate() {
				if let Some(hash) = slot.input.blob()
					&& written.insert(hash)
					&& let Some(base) = previous.input_blob(*id, index)
					&& base != hash
				{
					bases.insert(hash, base);
				}
			}
		}

		bases
	}

	fn input_blob(&self, id: NodeId, index: usize) -> Option<BlobHash> {
		self.node_instances.get(&id)?.inputs().get(index)?.input.blob()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn value_input(value: Value) -> NodeInput {
		NodeInput::Value { value, exposed: false }
	}

	#[test]
	fn only_large_values_become_blobs() {
		let mut bytes = BlobBytes::new();

		let mut small = value_input(Value::Str("small".into()));
		small.move_into_blob(&mut bytes);
		assert_eq!(small, value_input(Value::Str("small".into())));
		assert!(bytes.is_empty());

		let large_value = Value::Str("x".repeat(BLOB_THRESHOLD));
		let mut large = value_input(large_value.clone());
		large.move_into_blob(&mut bytes);
		let hash = large.blob().expect("a large value becomes a blob");
		assert_eq!(decode_blob(&bytes[&hash]).unwrap(), large_value);
	}

	#[test]
	fn moving_twice_keeps_the_blob() {
		let mut bytes = BlobBytes::new();
		let mut input = value_input(Value::Str("x".repeat(BLOB_THRESHOLD)));
		input.move_into_blob(&mut bytes);
		let once = input.clone();

		input.move_into_blob(&mut bytes);
		assert_eq!(input, once);
		assert_eq!(bytes.len(), 1);
	}

	#[test]
	fn bases_stay_acyclic_when_slots_swap_blobs() {
		let [x, a] = [b"x", b"a"].map(|bytes| BlobHash::of(bytes));
		let write = |node: u64, hash: BlobHash| RegistryDelta::ChangeNodeInput {
			id: NodeId(node),
			index: 0,
			new_input: value_input(Value::Blob(hash)),
		};

		let bases = Registry::default().blob_bases(&[write(1, x), write(1, a), write(2, a), write(2, x)]);
		assert_eq!(bases, HashMap::from([(a, x)]), "only first writes take a base");
	}
}
