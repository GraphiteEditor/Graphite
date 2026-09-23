use glam::DVec2;

#[derive(Clone, Copy, Debug, graphene_hash::CacheHash)]
pub struct PaintRenderParams {
	pub paint_target_bounds: [DVec2; 2],
}

impl PaintRenderParams {
	pub const DEFAULT: Self = Self {
		paint_target_bounds: [DVec2::ZERO, DVec2::ONE],
	};
}
