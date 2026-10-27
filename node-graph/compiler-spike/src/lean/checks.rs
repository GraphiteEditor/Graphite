//! Node-local compile-time checks: inputs required to be constant, and footprint invariance.

use super::ir::NewNode;

const FOOTPRINT: u32 = 1 << 0;

#[derive(Default)]
pub struct Checks {
	/// Indexed by IR node ID: whether the node's output is a value known at compile time.
	pub resolves_to_value: Vec<bool>,
	pub footprint_warnings: Vec<u32>,
}

impl Checks {
	pub fn clear(&mut self) {
		self.resolves_to_value.clear();
		self.footprint_warnings.clear();
	}

	/// Checks a node about to be built and returns whether its output resolves to a compile-time value.
	pub fn check(&self, node: &NewNode, inputs: &[u32]) -> Result<bool, String> {
		let metadata = node.compile_metadata;
		for (index, &input) in inputs.iter().enumerate() {
			let constant = index < 64 && metadata.constant_inputs & (1 << index) != 0;
			if constant && !self.resolves_to_value[input as usize] {
				return Err("An input requiring a constant is not fed by a value".to_string());
			}
		}
		Ok(node.value.is_some() || metadata.forwards_input && inputs.first().is_some_and(|&first| self.resolves_to_value[first as usize]))
	}

	/// Records a footprint warning for a footprint-invariant node whose upstream reads the footprint.
	pub fn check_footprint(&mut self, id: u32, node: &NewNode, upstream_features: u32) {
		if node.compile_metadata.footprint_invariant && upstream_features & FOOTPRINT != 0 {
			self.footprint_warnings.push(id);
		}
	}
}
