use crate::adjustment_steps::AdjustmentSteps;
use core_types::Color;

/// A per-pixel color mapping, run by `map_cpu` on the CPU and on the GPU by its adjustment shader from the step `encode_step` adds.
pub trait MapColor: Sync + 'static {
	fn map_cpu(&self, color: Color) -> Color;

	/// Adds this kernel to a run of steps for the GPU, as its kind and the parameters the shader's function for that kind reads.
	fn encode_step(&self, steps: &mut AdjustmentSteps);
}
