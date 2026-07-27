use core_types::math::bbox::AxisAlignedBbox;
use core_types::transform::Footprint;
use glam::{DAffine2, DVec2, UVec2};

/// Upper bound on either texture dimension, so an oversized request (a large export) can't
/// exceed what the GPU supports.
const MAX_RESOLUTION: u32 = 8192;

/// Texel step crops grow in, so a growing stroke rarely resizes its textures and freed spares
/// keep matching.
const CROP_STEP: u32 = 256;

/// The rendered document region and its texel geometry.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Region {
	pub(crate) min: DVec2,
	/// Texels per document unit.
	pub(crate) scale: f64,
	pub(crate) size: UVec2,
}

impl Region {
	/// The footprint's viewport at the resolution the view needs, expanded by two texels so a
	/// stroke cut at the region edge stays offscreen even if the displayed canvas rounds a texel
	/// past the footprint. Anchored to the texel lattice, so equal footprints derive identical
	/// grids. `None` when empty or degenerate.
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
		// The -2 leaves room for the floor/ceil below to add a texel per side at the cap.
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

/// A content window within a [`Region`]: a whole-texel sub-rectangle on the region's lattice,
/// so placed textures relate by pure integer offsets and cropping never resamples.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Crop {
	pub(crate) origin: UVec2,
	pub(crate) size: UVec2,
}

impl Crop {
	/// The [`CROP_STEP`]-quantized window of `content` within the region; `None` when they don't
	/// intersect (nothing visible to render).
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

	/// Doc-space placement of the window. The texture spans a whole number of texels; map
	/// exactly that span so content isn't compressed by the partial last texel.
	pub(crate) fn transform(&self, region: &Region) -> DAffine2 {
		DAffine2::from_translation(region.min + self.origin.as_dvec2() / region.scale) * DAffine2::from_scale(self.size.as_dvec2() / region.scale)
	}
}
