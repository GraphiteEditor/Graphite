use core_types::Color;

/// A per-pixel color mapping, run by `map_cpu` on the CPU.
pub trait MapColor: Sync + 'static {
	fn map_cpu(&self, color: Color) -> Color;
}
