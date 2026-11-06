//! Large node input values move out of the registry, history, and hot log into the resource store, leaving a
//! [`Value::Blob`] naming their bytes. A blob's bytes are the value's postcard encoding.

use std::collections::HashMap;

use crate::{InputSlot, NodeId, NodeInput, Registry, RegistryDelta, ResourceHash, Value};

/// Values whose encoding reaches this many bytes are stored as blobs.
pub const BLOB_THRESHOLD: usize = 1024;

/// Blob contents keyed by hash, for the caller's byte store.
pub type BlobBytes = HashMap<ResourceHash, Vec<u8>>;

/// The value a blob's bytes encode.
pub fn decode_blob(bytes: &[u8]) -> Result<Value, postcard::Error> {
	postcard::from_bytes(bytes)
}

impl NodeInput {
	/// Replace a value of at least [`BLOB_THRESHOLD`] encoded bytes with the blob of those bytes, collecting them into `bytes`.
	pub fn move_into_blob(&mut self, bytes: &mut BlobBytes) {
		let NodeInput::Value { value, .. } = self else { return };
		if matches!(value, Value::Blob(_)) {
			return;
		}

		// Encoding a `Value` into memory has no failure case, so an error just leaves it inline
		let Ok(encoded) = postcard::to_stdvec(value) else { return };
		if encoded.len() < BLOB_THRESHOLD {
			return;
		}

		let hash = ResourceHash::from(encoded.as_slice());
		*value = Value::Blob(hash);
		bytes.insert(hash, encoded);
	}
}

impl RegistryDelta {
	/// Move the large values this op writes into blobs. See [`NodeInput::move_into_blob`].
	pub fn move_values_into_blobs(&mut self, bytes: &mut BlobBytes) {
		let slots = match self {
			RegistryDelta::ChangeNodeInput { new_input, .. } => return new_input.move_into_blob(bytes),
			RegistryDelta::AddNode { node, .. } => &mut node.inputs,
			RegistryDelta::SetNodeInputs { inputs, .. } => inputs,
			_ => return,
		};
		slots.iter_mut().for_each(|slot| slot.input.move_into_blob(bytes));
	}
}

impl Registry {
	/// Move the large values of every node input into blobs. See [`NodeInput::move_into_blob`].
	pub fn move_values_into_blobs(&mut self, bytes: &mut BlobBytes) {
		let slots = self.node_instances.values_mut().flat_map(|node| node.inputs.iter_mut());
		slots.for_each(|slot: &mut InputSlot| slot.input.move_into_blob(bytes));
	}

	/// For each blob `ops` write, the blob its input slot held before, as the base a store can encode it against.
	/// A slot written twice in `ops` bases the second blob on the first.
	pub fn blob_bases(&self, ops: &[RegistryDelta]) -> HashMap<ResourceHash, ResourceHash> {
		let mut latest: HashMap<(NodeId, usize), ResourceHash> = HashMap::new();
		let mut bases = HashMap::new();

		let mut write = |id: NodeId, index: usize, input: &NodeInput| {
			let Some(hash) = input.blob() else { return };

			let previous = latest.get(&(id, index)).copied().or_else(|| self.node_instances.get(&id)?.inputs().get(index)?.input.blob());
			if let Some(previous) = previous
				&& previous != hash
			{
				bases.entry(hash).or_insert(previous);
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
}
