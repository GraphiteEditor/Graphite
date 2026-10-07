use crate::application::generate_uuid;

/// Identifies one guide line. The editor generates the id when the line is created, so the frontend never mints one.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(from_wasm_abi))]
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct GuideLineId(pub u64);

impl GuideLineId {
	pub fn new() -> Self {
		Self(generate_uuid())
	}
}

impl Default for GuideLineId {
	fn default() -> Self {
		Self::new()
	}
}

/// Whether a guide line runs across the canvas horizontally or vertically.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify), tsify(from_wasm_abi))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum GuideLineDirection {
	Horizontal,
	Vertical,
}

/// One guide line as a ruler needs it: where the line sits along that ruler, and which line it is.
///
/// The offset is a viewport-space distance from the viewport's corner rather than a document coordinate, because that is
/// the space the ruler measures in. Sharing it means the two sides agree on where a line is without the ruler knowing
/// the document transform. The id travels with it so a drag names the guide it grabbed, rather than its place in a list.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GuideRulerEntry {
	/// The guide's position along this ruler, in viewport pixels measured from the viewport's corner.
	pub offset: f64,
	/// The id of the guide this entry describes, which the ruler hands back to name the line a drag grabbed.
	pub id: GuideLineId,
}

/// One guide line, spanning the whole canvas at a fixed position in document space.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GuideLine {
	pub id: GuideLineId,
	pub direction: GuideLineDirection,
	/// Position in document space (the Y coordinate for a horizontal guide, the X coordinate for a vertical one).
	pub position: f64,
}

impl GuideLine {
	pub fn new(direction: GuideLineDirection, position: f64) -> Self {
		Self {
			id: GuideLineId::new(),
			direction,
			position,
		}
	}

	/// The point on this line that a snapping or overlay routine anchors to, in document space.
	pub fn anchor(&self) -> glam::DVec2 {
		match self.direction {
			GuideLineDirection::Horizontal => glam::DVec2::new(0., self.position),
			GuideLineDirection::Vertical => glam::DVec2::new(self.position, 0.),
		}
	}

	/// The direction the line runs in.
	pub fn direction_vector(&self) -> glam::DVec2 {
		match self.direction {
			GuideLineDirection::Horizontal => glam::DVec2::X,
			GuideLineDirection::Vertical => glam::DVec2::Y,
		}
	}

	/// This guide as a viewport-space line, as a point on it and its direction, so a ruler can meet it where it crosses.
	pub fn viewport_line(&self, document_to_viewport: glam::DAffine2) -> (glam::DVec2, glam::DVec2) {
		match self.direction {
			GuideLineDirection::Vertical => (
				document_to_viewport.transform_point2(glam::DVec2::new(self.position, 0.)),
				document_to_viewport.transform_vector2(glam::DVec2::Y),
			),
			GuideLineDirection::Horizontal => (
				document_to_viewport.transform_point2(glam::DVec2::new(0., self.position)),
				document_to_viewport.transform_vector2(glam::DVec2::X),
			),
		}
	}

	/// The rulers this guide can be grabbed from, each with where along it the guide sits in viewport pixels.
	///
	/// A guide is listed on every ruler its own line reaches, so a tilted canvas routes it to the rulers it visibly
	/// crosses rather than the ones its document direction happens to name. A line nearly parallel to a ruler has no
	/// useful crossing with it, which the length-relative bound keeps out without tripping on a zoomed-out canvas.
	pub fn ruler_crossings(&self, document_to_viewport: glam::DAffine2) -> impl Iterator<Item = (bool, GuideRulerEntry)> {
		let (anchor, direction) = self.viewport_line(document_to_viewport);
		let length = direction.length().max(f64::MIN_POSITIVE);

		// The top ruler runs along the viewport's top edge and the left one along its left edge.
		let top = (direction.y.abs() > length * 1e-9).then(|| {
			(
				true,
				GuideRulerEntry {
					offset: anchor.x - anchor.y * direction.x / direction.y,
					id: self.id,
				},
			)
		});
		let left = (direction.x.abs() > length * 1e-9).then(|| {
			(
				false,
				GuideRulerEntry {
					offset: anchor.y - anchor.x * direction.y / direction.x,
					id: self.id,
				},
			)
		});
		top.into_iter().chain(left)
	}
}

#[cfg(test)]
mod test {
	use super::*;

	/// The crossings of one guide, as `(on the top ruler, offset there)`, in creation order.
	fn crossings(guide: &GuideLine, document_to_viewport: glam::DAffine2) -> Vec<(bool, f64)> {
		guide.ruler_crossings(document_to_viewport).map(|(is_top, entry)| (is_top, entry.offset)).collect()
	}

	#[test]
	fn test_upright_canvas_lists_each_guide_on_its_own_ruler() {
		let upright = glam::DAffine2::from_scale_angle_translation(glam::DVec2::splat(1.5), 0., glam::DVec2::new(40., 60.));

		let vertical = crossings(&GuideLine::new(GuideLineDirection::Vertical, 10.), upright);
		assert_eq!(vertical.len(), 1, "an upright vertical guide crosses only the top ruler");
		assert!(vertical[0].0, "that crossing belongs to the top ruler");
		assert!((vertical[0].1 - (10. * 1.5 + 40.)).abs() < 1e-9, "the top offset is the guide's viewport X");

		let horizontal = crossings(&GuideLine::new(GuideLineDirection::Horizontal, 20.), upright);
		assert_eq!(horizontal.len(), 1, "an upright horizontal guide crosses only the left ruler");
		assert!(!horizontal[0].0, "that crossing belongs to the left ruler");
		assert!((horizontal[0].1 - (20. * 1.5 + 60.)).abs() < 1e-9, "the left offset is the guide's viewport Y");
	}

	#[test]
	fn test_tilted_canvas_lists_a_guide_on_every_ruler_it_crosses() {
		let tilted = glam::DAffine2::from_angle_translation(std::f64::consts::FRAC_PI_4, glam::DVec2::new(0., 0.));

		// A 45-degree canvas slants every guide across both edges.
		let vertical = crossings(&GuideLine::new(GuideLineDirection::Vertical, 0.), tilted);
		assert_eq!(vertical.len(), 2, "a slanted guide reaches both rulers");

		// A quarter turn lays a vertical guide along the left ruler, so only that ruler gets it.
		let quarter_turned = glam::DAffine2::from_angle_translation(std::f64::consts::FRAC_PI_2, glam::DVec2::new(200., 250.));
		let laid_flat = crossings(&GuideLine::new(GuideLineDirection::Vertical, 50.), quarter_turned);
		assert_eq!(laid_flat.len(), 1, "a guide parallel to a ruler has no crossing with it");
		assert!(!laid_flat[0].0, "the remaining crossing belongs to the left ruler");
		assert!((laid_flat[0].1 - 300.).abs() < 1e-9, "it sits where the guide's line meets that edge");
	}
}
