//! The typing pass: selects each node's implementation from its input types and records the result per node.

use super::ir::{Ir, NewNode};
use super::registry::{Conversion, Promotion, Registry};
use super::types::{TypeId, TypeInterner};

/// The typing of a node about to be built, or the conversions its inputs need first.
pub enum Inferred {
	Typed(TypeInfo),
	NeedsConversions(Box<[Conversion]>),
}

#[derive(Clone, Copy, Debug)]
pub struct TypeInfo {
	pub return_type: TypeId,
	pub fn_type: TypeId,
	pub implementation: u32,
	pub promotions: (u32, u32),
}

pub struct Typing {
	pub types: TypeInterner,
	pub registry: Registry,
	/// Indexed by IR node ID.
	pub info: Vec<TypeInfo>,
	pub promotions: Vec<Promotion>,
	context: TypeId,
	scratch: Vec<TypeId>,
}

impl Typing {
	pub fn new(types: TypeInterner, registry: Registry, context: TypeId) -> Self {
		Self {
			types,
			registry,
			info: Vec::new(),
			promotions: Vec::new(),
			context,
			scratch: Vec::new(),
		}
	}

	pub fn clear(&mut self) {
		self.info.clear();
		self.promotions.clear();
	}

	pub fn promotions(&self, id: u32) -> &[Promotion] {
		let (start, end) = self.info[id as usize].promotions;
		&self.promotions[start as usize..end as usize]
	}

	/// Selects the implementation for a node with these inputs, returning the selection's index for `infer`.
	pub fn select(&mut self, node: &NewNode, inputs: &[u32]) -> u32 {
		self.scratch.clear();
		self.scratch.extend(inputs.iter().map(|&input| self.info[input as usize].fn_type));
		self.registry.select_index(&mut self.types, node.identifier, node.call_argument, &self.scratch)
	}

	/// The conversions a selection needs inserted on its inputs before the node can be built.
	pub fn conversions(&self, selection: u32) -> Result<&[Conversion], String> {
		self.registry.selection(selection).map(|selection| selection.conversions.as_slice())
	}

	/// Types a node, using `selected` when typing already selected its implementation for inputs of the same types.
	pub fn infer(&mut self, ir: &Ir, node: &NewNode, inputs: &[u32], selected: Option<u32>) -> Result<Inferred, String> {
		if let Some(value) = node.value {
			let fn_type = self.types.function(self.context, value.ty);
			return Ok(Inferred::Typed(TypeInfo {
				return_type: value.ty,
				fn_type,
				implementation: 0,
				promotions: (0, 0),
			}));
		}

		debug_assert!(inputs.iter().all(|&input| (input as usize) < ir.len()));
		let selection = selected.unwrap_or_else(|| self.select(node, inputs));
		let selection = self.registry.selection(selection)?;
		if !selection.conversions.is_empty() {
			return Ok(Inferred::NeedsConversions(selection.conversions.as_slice().into()));
		}

		let start = self.promotions.len() as u32;
		self.promotions.extend_from_slice(&selection.promotions);
		Ok(Inferred::Typed(TypeInfo {
			return_type: selection.return_type,
			fn_type: selection.fn_type,
			implementation: selection.implementation,
			promotions: (start, self.promotions.len() as u32),
		}))
	}
}
