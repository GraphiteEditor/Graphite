use core_types::math::bbox::AxisAlignedBbox;
use core_types::transform::Footprint;
use glam::{DAffine2, DVec2, UVec2};

const MAX_RESOLUTION: u32 = 8192;

const CROP_STEP: u32 = 256;

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Region {
	pub(crate) min: DVec2,
	pub(crate) scale: f64,
	pub(crate) size: UVec2,
}

impl Region {
	pub(crate) fn new(footprint: &Footprint) -> Option<Self> {
		let margin = DVec2::splat(2. / footprint.scale().max_element());
		let viewport = footprint.viewport_bounds_in_local_space();
		let bounds = AxisAlignedBbox {
			start: viewport.start - margin,
			end: viewport.end + margin,
		};
		if !bounds.size().cmpgt(DVec2::ZERO).all() {
			return None;
		}
		// -2 leaves room for the floor/ceil below to add a texel per side at the cap.
		let scale = footprint.scale().max_element().min((MAX_RESOLUTION as f64 - 2.) / bounds.size().max_element());
		if !scale.is_finite() || scale <= 0. {
			return None;
		}
		let start = (bounds.start * scale).floor();
		let end = (bounds.end * scale).ceil();
		let size = (end - start).as_uvec2().max(UVec2::ONE);
		Some(Self { min: start / scale, scale, size })
	}
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Crop {
	pub(crate) origin: UVec2,
	pub(crate) size: UVec2,
}

impl Crop {
	pub(crate) fn new(content: AxisAlignedBbox, region: &Region) -> Option<Self> {
		let start = ((content.start - region.min) * region.scale).floor().max(DVec2::ZERO);
		let end = ((content.end - region.min) * region.scale).ceil().min(region.size.as_dvec2());
		if !(end - start).cmpgt(DVec2::ZERO).all() {
			return None;
		}
		let origin = start.as_uvec2() / CROP_STEP * CROP_STEP;
		let end = ((end.as_uvec2() + UVec2::splat(CROP_STEP - 1)) / CROP_STEP * CROP_STEP).min(region.size);
		Some(Self { origin, size: end - origin })
	}

	pub(crate) fn transform(&self, region: &Region) -> DAffine2 {
		DAffine2::from_translation(region.min + self.origin.as_dvec2() / region.scale) * DAffine2::from_scale(self.size.as_dvec2() / region.scale)
	}

	pub(crate) fn scissor(&self, region: &Region, bounds: AxisAlignedBbox) -> (UVec2, UVec2) {
		let clamp = |texels: UVec2| texels.max(self.origin).min(self.origin + self.size) - self.origin;
		let min = ((bounds.start - region.min) * region.scale).floor().max(DVec2::ZERO).as_uvec2().min(region.size);
		let max = ((bounds.end - region.min) * region.scale).ceil().max(DVec2::ZERO).as_uvec2().min(region.size);
		(clamp(min), clamp(max) - clamp(min))
	}
}
