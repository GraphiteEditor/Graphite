use glam::DAffine2;

#[derive(Clone, Copy, Debug, graphene_hash::CacheHash)]
pub struct PaintRenderParams {
	pub fallback_paint_to_target: Option<DAffine2>,
}

impl PaintRenderParams {
	pub const DEFAULT: Self = Self { fallback_paint_to_target: None };
}
