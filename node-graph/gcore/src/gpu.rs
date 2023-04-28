use crate::{raster::Sample, Color};

use bytemuck::{Pod, Zeroable};
use spirv_std::image::{Image2d, SampledImage};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
pub struct PushConstants {
	pub n: u32,
	pub node: u32,
}

impl Sample for SampledImage<Image2d> {
	type Pixel = Color;

	fn sample(&self, pos: glam::DVec2, _area: glam::DVec2) -> Option<Self::Pixel> {
		let color = self.sample(pos);
		Color::from_rgbaf32(color.x, color.y, color.z, color.w)
	}
}
