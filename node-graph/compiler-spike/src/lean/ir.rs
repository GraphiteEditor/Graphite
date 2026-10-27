//! The IR store: hash-consed nodes in topological order. Pass results live in side tables indexed by node ID.

use super::document::IdentifierId;
use super::types::TypeId;
use crate::workload::CompileMetadata;
use rustc_hash::{FxHashMap, FxHasher};
use std::hash::{Hash, Hasher};

const NONE: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IrValue {
	pub ty: TypeId,
	pub hash: u64,
	pub source: Option<u64>,
}

/// A node about to be built, before its inputs are stored.
#[derive(Clone, Copy, Debug)]
pub struct NewNode {
	pub identifier: IdentifierId,
	pub call_argument: TypeId,
	pub value: Option<IrValue>,
	/// Path hash of nodes exempt from deduplication, which keeps each instance distinct.
	pub skip_deduplication: Option<u64>,
	pub compile_metadata: CompileMetadata,
	/// The hash of the containing instance's path and the document node's ID.
	pub origin: (u64, u64),
}

#[derive(Clone, Debug)]
pub struct IrNode {
	pub identifier: IdentifierId,
	pub call_argument: TypeId,
	pub inputs: (u32, u32),
	pub value: Option<IrValue>,
	pub skip_deduplication: Option<u64>,
	pub compile_metadata: CompileMetadata,
	pub origin: (u64, u64),
}

/// Compares element by element, since inputs are a handful of IDs and an out-of-line `memcmp` call costs more than the loop.
#[inline]
fn same_inputs(a: &[u32], b: &[u32]) -> bool {
	a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a == b)
}

#[derive(Default)]
pub struct Ir {
	pub nodes: Vec<IrNode>,
	pub inputs: Vec<u32>,
	consed: FxHashMap<u64, u32>,
	collisions: Vec<u32>,
}

impl Ir {
	pub fn clear(&mut self) {
		self.nodes.clear();
		self.inputs.clear();
		self.consed.clear();
		self.collisions.clear();
	}

	pub fn len(&self) -> usize {
		self.nodes.len()
	}

	pub fn is_empty(&self) -> bool {
		self.nodes.is_empty()
	}

	pub fn inputs(&self, id: u32) -> &[u32] {
		let (start, end) = self.nodes[id as usize].inputs;
		&self.inputs[start as usize..end as usize]
	}

	fn hash(node: &NewNode, inputs: &[u32]) -> u64 {
		let mut hasher = FxHasher::default();
		node.identifier.hash(&mut hasher);
		node.call_argument.hash(&mut hasher);
		node.value.map(|value| (value.ty, value.hash)).hash(&mut hasher);
		node.skip_deduplication.hash(&mut hasher);
		inputs.hash(&mut hasher);
		hasher.finish()
	}

	/// The existing node with this content, if any, and the content hash for inserting it otherwise.
	pub fn find(&self, node: &NewNode, inputs: &[u32]) -> (u64, Option<u32>) {
		let hash = Self::hash(node, inputs);
		let mut candidate = self.consed.get(&hash).copied().unwrap_or(NONE);
		while candidate != NONE {
			let existing = &self.nodes[candidate as usize];
			let same = existing.identifier == node.identifier
				&& existing.call_argument == node.call_argument
				&& existing.value.map(|value| (value.ty, value.hash)) == node.value.map(|value| (value.ty, value.hash))
				&& existing.skip_deduplication == node.skip_deduplication
				&& same_inputs(self.inputs(candidate), inputs);
			if same {
				return (hash, Some(candidate));
			}
			candidate = self.collisions[candidate as usize];
		}
		(hash, None)
	}

	pub fn push(&mut self, hash: u64, node: NewNode, inputs: &[u32]) -> u32 {
		let start = self.inputs.len() as u32;
		self.inputs.extend_from_slice(inputs);
		let id = self.nodes.len() as u32;
		self.nodes.push(IrNode {
			identifier: node.identifier,
			call_argument: node.call_argument,
			inputs: (start, self.inputs.len() as u32),
			value: node.value,
			skip_deduplication: node.skip_deduplication,
			compile_metadata: node.compile_metadata,
			origin: node.origin,
		});
		self.collisions.push(self.consed.insert(hash, id).unwrap_or(NONE));
		id
	}

	/// The nodes reachable from `output`, which is the compiled graph once earlier compiles have left unreferenced nodes behind.
	pub fn reachable(&self, output: u32) -> Vec<u32> {
		let mut reached = vec![false; self.nodes.len()];
		let mut stack = vec![output];
		while let Some(id) = stack.pop() {
			if std::mem::replace(&mut reached[id as usize], true) {
				continue;
			}
			stack.extend_from_slice(self.inputs(id));
		}
		(0..self.nodes.len() as u32).filter(|&id| reached[id as usize]).collect()
	}
}
