use crate::bounds::Bounds;
use crate::candidate::{Candidate, CandidateKey, Highlight, Kind, MatchedGaps};
use crate::find::{Awake, Context};
use crate::frame::Frame;
use crate::locus::Locus;
use crate::scene::{LayerId, LayerKind};
use glam::DVec2;
use std::cmp::Ordering;

/// The index of a box's low side among its sides along an axis.
const LOW: usize = 0;
/// The index of a box's high side among its sides along an axis.
const HIGH: usize = 1;

/// A box being moved or stretched, where it would be with nothing snapping it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpacedBox {
	/// The kind of layer it is, which the boxes it is spaced among must be too.
	pub kind: LayerKind,
	/// Its corners in order around it.
	pub corners: [DVec2; 4],
	/// For each of the frame's first two axes, the feature carrying its low side and the one carrying its high side, with where that feature is. A side that stays put is `None`.
	/// With both sides carried, the box moves whole along that axis, so a stretch about its center names neither.
	pub sides: [[Option<Side>; 2]; 2],
}

/// A feature carrying one side of a spaced box, and where that feature is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Side {
	pub feature: u32,
	pub position: DVec2,
}

/// The relationship a spacing candidate offers, which tells apart candidates drawn from the same neighbors.
#[derive(Clone, Copy, Debug)]
enum Rule {
	ContinueLow,
	ContinueHigh,
	Center,
}

/// How far something reaches along one direction.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Extent {
	low: f64,
	high: f64,
}

impl Extent {
	fn length(&self) -> f64 {
		self.high - self.low
	}

	/// How much two extents overlap, which is negative where there is a gap between them.
	fn overlap(&self, other: &Self) -> f64 {
		self.high.min(other.high) - self.low.max(other.low)
	}

	/// The middle of where two extents overlap, or of the gap between them.
	fn middle_of_overlap(&self, other: &Self) -> f64 {
		(self.low.max(other.low) + self.high.min(other.high)) / 2.
	}
}

/// Measures positions from the frame's origin along one of its axes and along the axis across it.
#[derive(Clone, Copy, Debug)]
struct Ruler {
	origin: DVec2,
	axis: DVec2,
	cross_axis: DVec2,
	/// Whether the axes are the plane's own, so that a strip along this one can be looked up as a box in the plane.
	plane_aligned: bool,
}

impl Ruler {
	/// A ruler along each of the frame's first two axes, with the other across it. Returns `None` unless those axes are at right angles.
	fn pair(frame: &Frame) -> Option<[Self; 2]> {
		let &[first, second, ..] = frame.axes() else { return None };
		if first.dot(second).abs() > 1e-9 {
			return None;
		}

		let plane_aligned = [first, second].iter().all(|axis| axis.x == 0. || axis.y == 0.);
		let ruler = |axis: DVec2, cross_axis: DVec2| Self {
			origin: frame.origin,
			axis,
			cross_axis,
			plane_aligned,
		};
		Some([ruler(first, second), ruler(second, first)])
	}

	/// How far a quad reaches along the axis and across it.
	fn extents(&self, quad: &[DVec2; 4]) -> [Extent; 2] {
		[self.axis, self.cross_axis].map(|direction| {
			let measured = quad.map(|corner| (corner - self.origin).dot(direction));
			Extent {
				low: measured.into_iter().fold(f64::INFINITY, f64::min),
				high: measured.into_iter().fold(f64::NEG_INFINITY, f64::max),
			}
		})
	}

	fn position(&self, along: f64, across: f64) -> DVec2 {
		self.origin + self.axis * along + self.cross_axis * across
	}

	/// A gap from one position along the axis to another, placed halfway across where the boxes on either side of it overlap across the axis.
	fn span(&self, from: f64, to: f64, acrosses: [Extent; 2]) -> [DVec2; 2] {
		let across = acrosses[0].middle_of_overlap(&acrosses[1]);
		[self.position(from, across), self.position(to, across)]
	}

	/// The area a box sweeps moving along the axis without end, as a box in the plane where the axes allow.
	fn strip(&self, quad: &[DVec2; 4]) -> Bounds {
		if !self.plane_aligned {
			return Bounds::EVERYTHING;
		}

		let bounds = Bounds::from_points(*quad);
		let (min, max) = if self.axis.y == 0. {
			(DVec2::new(f64::NEG_INFINITY, bounds.min.y), DVec2::new(f64::INFINITY, bounds.max.y))
		} else {
			(DVec2::new(bounds.min.x, f64::NEG_INFINITY), DVec2::new(bounds.max.x, f64::INFINITY))
		};
		Bounds { min, max }
	}
}

/// A box lying wholly to one side of the spaced box along an axis.
#[derive(Clone, Copy, Debug)]
struct Neighbor {
	layer: LayerId,
	quad: [DVec2; 4],
	along: Extent,
	across: Extent,
}

/// On each side of the spaced box along an axis, the nearest box and then the nearest box beyond that one.
struct Neighbors {
	low: [Option<Neighbor>; 2],
	high: [Option<Neighbor>; 2],
}

/// How much overlap still counts as only touching, allowing for rounding at the size and distance from the origin of the box measured.
fn hair(along: Extent, across: Extent) -> f64 {
	let magnitude = [along.low, along.high, across.low, across.high].into_iter().fold(0., |max: f64, value| max.max(value.abs()));
	1e-9 * (along.length() + across.length()) + 8. * f64::EPSILON * magnitude
}

/// The boxes of the spaced box's kind beside it along the ruler's axis that overlap it across that axis.
fn neighbors(context: &Context, spaced: &SpacedBox, ruler: &Ruler) -> Neighbors {
	let corners = &spaced.corners;
	let [along, across] = ruler.extents(corners);
	let hair = hair(along, across);

	// O(log n + k) for the k boxes in the strip the spaced box sweeps, or O(n) when the frame is turned from the plane's axes
	let (mut low, mut high) = (Vec::new(), Vec::new());
	context.scene.boxes_within(&ruler.strip(corners), context.exclusions, |layer, scene_layer, quad| {
		let [box_along, box_across] = ruler.extents(quad);
		if scene_layer.kind != spaced.kind || box_across.overlap(&across) <= hair {
			return;
		}

		let neighbor = Neighbor {
			layer,
			quad: *quad,
			along: box_along,
			across: box_across,
		};
		if box_along.high <= along.low + hair {
			low.push(neighbor);
		} else if box_along.low >= along.high - hair {
			high.push(neighbor);
		}
	});

	// Nearer boxes sort later, with the narrower and then the lower layer ID settling ties
	let low_order = |a: &Neighbor, b: &Neighbor| a.along.high.total_cmp(&b.along.high).then(a.along.low.total_cmp(&b.along.low)).then(b.layer.cmp(&a.layer));
	let high_order = |a: &Neighbor, b: &Neighbor| b.along.low.total_cmp(&a.along.low).then(b.along.high.total_cmp(&a.along.high)).then(b.layer.cmp(&a.layer));

	Neighbors {
		low: nearest_two(&low, low_order, |beyond, first| beyond.along.high <= first.along.low + hair),
		high: nearest_two(&high, high_order, |beyond, first| beyond.along.low >= first.along.high - hair),
	}
}

/// The nearest of the boxes on one side, where `order` sorts nearer ones later, and the nearest of those that `is_beyond` it.
fn nearest_two(side: &[Neighbor], order: impl Fn(&Neighbor, &Neighbor) -> Ordering, is_beyond: impl Fn(&Neighbor, &Neighbor) -> bool) -> [Option<Neighbor>; 2] {
	let first = side.iter().copied().max_by(&order);
	let second = first.and_then(|first| side.iter().copied().filter(|other| other.layer != first.layer && is_beyond(other, &first)).max_by(&order));
	[first, second]
}

/// How the spaced box moves along one axis.
struct Motion<'a> {
	ruler: &'a Ruler,
	sides: [Option<Side>; 2],
	along: Extent,
	across: Extent,
}

impl<'a> Motion<'a> {
	fn new(ruler: &'a Ruler, corners: &[DVec2; 4], sides: [Option<Side>; 2]) -> Self {
		let [along, across] = ruler.extents(corners);
		Self { ruler, sides, along, across }
	}

	fn rigid(&self) -> bool {
		self.sides[LOW].is_some() && self.sides[HIGH].is_some()
	}

	/// How far the box reaches along the axis once one side is at a target, unless that side stays put or a stretch would turn the box inside out.
	fn moved(&self, side: usize, target: f64) -> Option<Extent> {
		self.sides.get(side).copied().flatten()?;
		let Extent { low, high } = self.along;

		let moved = if self.rigid() {
			let travel = target - if side == LOW { low } else { high };
			Extent {
				low: low + travel,
				high: high + travel,
			}
		} else if side == LOW {
			Extent { low: target, high }
		} else {
			Extent { low, high: target }
		};
		(self.rigid() || moved.low < moved.high).then_some(moved)
	}

	/// A track that brings the feature carrying one side to where that side is in `moved`, if that is within the gathering distance of it.
	fn track(&self, context: &Context, side: usize, moved: Extent, kind: Kind, identity: [u64; 3]) -> Option<Candidate> {
		let carrier = self.sides.get(side).copied().flatten()?;
		let travel = if side == LOW { moved.low - self.along.low } else { moved.high - self.along.high };
		let shifted = carrier.position + self.ruler.axis * travel;
		if context.view.screen_distance(carrier.position, shifted) > context.tunables.gathering_distance() {
			return None;
		}

		let key = CandidateKey {
			kind,
			feature: carrier.feature,
			identity,
		};
		Some(Candidate::track(key, Locus::line(shifted, self.ruler.cross_axis)?))
	}
}

/// Finds where the moving sides of a box would make its gaps to the boxes beside it equal: continuing the spacing of the two boxes on either side, or centering it between its nearest neighbors.
pub fn find_spacing(context: &Context, spaced: &SpacedBox, candidates: &mut Vec<Candidate>) {
	let Some(rulers) = Ruler::pair(context.frame) else { return };

	for (axis_index, (ruler, sides)) in rulers.iter().zip(spaced.sides).enumerate() {
		if sides == [None, None] {
			continue;
		}

		let motion = Motion::new(ruler, &spaced.corners, sides);
		let Neighbors { low, high } = neighbors(context, spaced, ruler);
		let identity = |first: &Neighbor, second: &Neighbor, rule: Rule| [first.layer.0, second.layer.0, rule as u64 * 2 + axis_index as u64];
		let mut offer = |side: usize, moved: Extent, gap: f64, identity: [u64; 3], spans: [[DVec2; 2]; 2]| {
			if gap >= 0.
				&& let Some(track) = motion.track(context, side, moved, Kind::Spacing, identity)
			{
				candidates.push(track.with_gaps(MatchedGaps { gap, spans }));
			}
		};

		// Continue the spacing of the two nearest boxes on the low side
		if let [Some(first), Some(second)] = low {
			let gap = first.along.low - second.along.high;

			if let Some(moved) = motion.moved(LOW, first.along.high + gap) {
				let spans = [
					ruler.span(second.along.high, first.along.low, [second.across, first.across]),
					ruler.span(first.along.high, moved.low, [first.across, motion.across]),
				];
				offer(LOW, moved, gap, identity(&first, &second, Rule::ContinueLow), spans);
			}
		}

		// Continue the spacing of the two nearest boxes on the high side
		if let [Some(first), Some(second)] = high {
			let gap = second.along.low - first.along.high;

			if let Some(moved) = motion.moved(HIGH, first.along.low - gap) {
				let spans = [
					ruler.span(moved.high, first.along.low, [motion.across, first.across]),
					ruler.span(first.along.high, second.along.low, [first.across, second.across]),
				];
				offer(HIGH, moved, gap, identity(&first, &second, Rule::ContinueHigh), spans);
			}
		}

		// Center the box between the nearest box on each side, by moving it whole or by stretching whichever side moves
		if let (Some(before), Some(after)) = (low[0], high[0]) {
			let Extent { low: start, high: end } = motion.along;
			let (side, target) = if motion.rigid() {
				(LOW, (before.along.high + after.along.low - motion.along.length()) / 2.)
			} else if sides[HIGH].is_some() {
				(HIGH, after.along.low - (start - before.along.high))
			} else {
				(LOW, before.along.high + (after.along.low - end))
			};

			if let Some(moved) = motion.moved(side, target) {
				let spans = [
					ruler.span(before.along.high, moved.low, [before.across, motion.across]),
					ruler.span(moved.high, after.along.low, [motion.across, after.across]),
				];
				offer(side, moved, moved.low - before.along.high, identity(&before, &after, Rule::Center), spans);
			}
		}
	}
}

/// Finds where the one moving side of a box being stretched along an axis would make it as long along that axis as a box beside it or an awake box.
pub fn find_equal_sizes(context: &Context, spaced: &SpacedBox, candidates: &mut Vec<Candidate>) {
	let Some(rulers) = Ruler::pair(context.frame) else { return };
	let stretched = |sides: &[Option<Side>; 2]| sides[LOW].is_some() != sides[HIGH].is_some();
	if !spaced.sides.iter().any(stretched) {
		return;
	}

	// The nearest box on either side along both axes, and every awake box, each once
	let mut references: Vec<(LayerId, [DVec2; 4])> = Vec::new();
	for ruler in &rulers {
		let Neighbors { low, high } = neighbors(context, spaced, ruler);
		references.extend([low[0], high[0]].into_iter().flatten().map(|neighbor| (neighbor.layer, neighbor.quad)));
	}
	for awake in context.awake {
		if let Awake::Box { layer } = *awake
			&& !context.exclusions.skips_box(layer)
			&& let Some(quad) = context.scene.layer(layer).and_then(|scene_layer| scene_layer.quad)
		{
			references.push((layer, quad));
		}
	}
	references.sort_by_key(|&(layer, _)| layer);
	references.dedup_by_key(|&mut (layer, _)| layer);

	for (axis_index, (ruler, sides)) in rulers.iter().zip(spaced.sides).enumerate() {
		if !stretched(&sides) {
			continue;
		}

		let motion = Motion::new(ruler, &spaced.corners, sides);
		let side = if sides[LOW].is_some() { LOW } else { HIGH };

		for &(layer, quad) in &references {
			let [reference, _] = ruler.extents(&quad);
			let length = reference.length();
			if length <= 0. {
				continue;
			}

			// The side staying put is where the length is measured from
			let target = if side == LOW { motion.along.high - length } else { motion.along.low + length };
			let Some(moved) = motion.moved(side, target) else { continue };

			if let Some(track) = motion.track(context, side, moved, Kind::EqualSize, [layer.0, axis_index as u64, 0]) {
				candidates.push(track.with_highlight(Highlight::Box { layer }));
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::candidate::Shape;
	use crate::scene::{Exclusions, Scene, SceneLayer};
	use crate::settings::Settings;
	use crate::tunables::Tunables;
	use crate::view::ViewMapping;
	use glam::{DAffine2, DMat2};

	/// Two boxes 200 units apart, between which a box can be centered.
	const APART: [[f64; 4]; 2] = [[0., 0., 100., 60.], [300., 0., 400., 60.]];

	const RIGID: [[bool; 2]; 2] = [[true; 2]; 2];
	const RIGHT_SIDE: [[bool; 2]; 2] = [[false, true], [false; 2]];
	const LEFT_SIDE: [[bool; 2]; 2] = [[true, false], [false; 2]];
	const BOTTOM_SIDE: [[bool; 2]; 2] = [[false; 2], [false, true]];

	/// The corners of a rectangle given as `[min x, min y, max x, max y]`, in order around it from the minimum.
	fn corners(rectangle: [f64; 4]) -> [DVec2; 4] {
		let [min_x, min_y, max_x, max_y] = rectangle;
		[DVec2::new(min_x, min_y), DVec2::new(max_x, min_y), DVec2::new(max_x, max_y), DVec2::new(min_x, max_y)]
	}

	fn layer(rectangle: [f64; 4], kind: LayerKind) -> SceneLayer {
		SceneLayer {
			parent: None,
			kind,
			quad: Some(corners(rectangle)),
			anchors: Vec::new(),
			curves: Vec::new(),
		}
	}

	/// A box with the sides marked in `moving` each carried by a corner: the first carries both low sides, the second the high side along X, and the fourth the high side along Y.
	fn spaced(rectangle: [f64; 4], moving: [[bool; 2]; 2]) -> SpacedBox {
		let corners = corners(rectangle);
		let carriers = [[0, 1], [0, 3]];
		let side = |axis: usize, side: usize| {
			let feature = carriers[axis][side];
			moving[axis][side].then_some(Side {
				feature,
				position: corners[feature as usize],
			})
		};

		SpacedBox {
			kind: LayerKind::Artwork,
			corners,
			sides: [[side(0, LOW), side(0, HIGH)], [side(1, LOW), side(1, HIGH)]],
		}
	}

	/// Where a track takes the corner of the spaced box that it applies to.
	fn landing(candidate: &Candidate, spaced: &SpacedBox) -> DVec2 {
		let Shape::Track(locus) = candidate.shape else { panic!("{candidate:?} should be a track") };
		locus.nearest(spaced.corners[candidate.key.feature as usize])
	}

	/// Whether a candidate makes two gaps of a length equal, with each span given by the coordinates of its ends.
	fn gaps_match(candidate: &Candidate, gap: f64, spans: [[(f64, f64); 2]; 2]) -> bool {
		let Some(gaps) = candidate.gaps else { return false };
		let mut ends = gaps.spans.iter().flatten().zip(spans.iter().flatten());

		(gaps.gap - gap).abs() < 1e-9 && ends.all(|(found, &wanted)| found.abs_diff_eq(DVec2::from(wanted), 1e-9))
	}

	/// A scene of boxes numbered from layer 1, and how it is being looked at.
	struct Setup {
		scene: Scene,
		view: ViewMapping,
		frame: Frame,
		exclusions: Exclusions,
		awake: Vec<Awake>,
	}

	impl Setup {
		fn new(rectangles: &[[f64; 4]]) -> Self {
			let mut scene = Scene::new();
			for (index, &rectangle) in rectangles.iter().enumerate() {
				scene.set_layer(LayerId(index as u64 + 1), layer(rectangle, LayerKind::Artwork));
			}

			Self {
				scene,
				view: ViewMapping::IDENTITY,
				frame: Frame::default(),
				exclusions: Exclusions::default(),
				awake: Vec::new(),
			}
		}

		fn found(&self, spaced: &SpacedBox, finder: fn(&Context, &SpacedBox, &mut Vec<Candidate>)) -> Vec<Candidate> {
			let (tunables, settings) = (Tunables::default(), Settings::default());
			let context = Context {
				scene: &self.scene,
				view: &self.view,
				frame: &self.frame,
				tunables: &tunables,
				settings: &settings,
				exclusions: &self.exclusions,
				awake: &self.awake,
			};

			let mut candidates = Vec::new();
			finder(&context, spaced, &mut candidates);
			candidates
		}

		fn spacing(&self, spaced: &SpacedBox) -> Vec<Candidate> {
			self.found(spaced, find_spacing)
		}

		fn equal_sizes(&self, spaced: &SpacedBox) -> Vec<Candidate> {
			self.found(spaced, find_equal_sizes)
		}
	}

	#[test]
	fn a_box_moved_between_two_others_is_offered_the_place_centered_between_them() {
		let setup = Setup::new(&APART);
		let moved = spaced([170., 10., 220., 50.], RIGID);

		let candidates = setup.spacing(&moved);

		assert_eq!(candidates.len(), 1);
		assert_eq!((candidates[0].key.kind, candidates[0].key.feature), (Kind::Spacing, 0));
		assert!(landing(&candidates[0], &moved).abs_diff_eq(DVec2::new(175., 10.), 1e-9));

		// Each gap is drawn halfway across where the moved box overlaps the box on that side
		assert!(gaps_match(&candidates[0], 75., [[(100., 30.), (175., 30.)], [(225., 30.), (300., 30.)]]));
	}

	#[test]
	fn a_box_moved_past_two_evenly_spaced_others_is_offered_the_place_continuing_their_spacing() {
		let setup = Setup::new(&[[0., 0., 100., 60.], [150., 0., 250., 60.]]);

		// Beyond the second box, the low side lands as far past it as the two boxes are apart
		let beyond = spaced([303., 10., 353., 50.], RIGID);
		let candidates = setup.spacing(&beyond);
		assert_eq!(candidates.len(), 1);
		assert!(landing(&candidates[0], &beyond).abs_diff_eq(DVec2::new(300., 10.), 1e-9));
		assert!(gaps_match(&candidates[0], 50., [[(100., 30.), (150., 30.)], [(250., 30.), (300., 30.)]]));

		// Before the first box, the high side lands as far short of it
		let before = spaced([-103., 10., -53., 50.], RIGID);
		let candidates = setup.spacing(&before);
		assert_eq!(candidates.len(), 1);
		assert_eq!(candidates[0].key.feature, 1);
		assert!(landing(&candidates[0], &before).abs_diff_eq(DVec2::new(-50., 10.), 1e-9));
		assert!(gaps_match(&candidates[0], 50., [[(-50., 30.), (0., 30.)], [(100., 30.), (150., 30.)]]));
	}

	#[test]
	fn stretching_one_side_matches_the_gap_on_the_other_side() {
		let setup = Setup::new(&APART);

		let right = spaced([130., 10., 266., 50.], RIGHT_SIDE);
		let candidates = setup.spacing(&right);
		assert_eq!(candidates.len(), 1);
		assert!(landing(&candidates[0], &right).abs_diff_eq(DVec2::new(270., 10.), 1e-9));
		assert!(gaps_match(&candidates[0], 30., [[(100., 30.), (130., 30.)], [(270., 30.), (300., 30.)]]));

		let left = spaced([134., 10., 270., 50.], LEFT_SIDE);
		let candidates = setup.spacing(&left);
		assert_eq!(candidates.len(), 1);
		assert!(landing(&candidates[0], &left).abs_diff_eq(DVec2::new(130., 10.), 1e-9));
		assert!(gaps_match(&candidates[0], 30., [[(100., 30.), (130., 30.)], [(270., 30.), (300., 30.)]]));
	}

	#[test]
	fn boxes_stacked_along_the_second_axis_are_spaced_the_same_way() {
		let setup = Setup::new(&[[0., 0., 60., 100.], [0., 300., 60., 400.]]);
		let moved = spaced([10., 170., 50., 220.], RIGID);

		let candidates = setup.spacing(&moved);

		assert_eq!(candidates.len(), 1);
		assert!(landing(&candidates[0], &moved).abs_diff_eq(DVec2::new(10., 175.), 1e-9));
		assert!(gaps_match(&candidates[0], 75., [[(30., 100.), (30., 175.)], [(30., 225.), (30., 300.)]]));
	}

	#[test]
	fn each_relationship_keeps_its_key_as_the_box_moves_and_differs_from_the_others() {
		// Pairs of boxes 30 apart on either side, with the place continuing each pair's spacing also the centered one
		let setup = Setup::new(&[[0., 0., 120., 60.], [150., 0., 250., 60.], [350., 0., 450., 60.], [480., 0., 580., 60.]]);
		let keys_at = |low: f64| {
			let mut keys: Vec<CandidateKey> = setup.spacing(&spaced([low, 10., low + 40., 50.], RIGID)).iter().map(|candidate| candidate.key).collect();
			keys.sort();
			keys
		};

		let here = keys_at(283.);
		assert_eq!(here.len(), 3);
		assert!(here.windows(2).all(|pair| pair[0] != pair[1]));
		assert_eq!(keys_at(277.), here);
	}

	#[test]
	fn spacing_is_not_continued_without_a_second_box_beyond_the_first() {
		let setup = Setup::new(&[[150., 0., 250., 60.]]);

		assert!(setup.spacing(&spaced([303., 10., 353., 50.], RIGID)).is_empty());
	}

	#[test]
	fn boxes_overlapping_by_a_hair_have_no_spacing_to_continue() {
		// The boxes count as touching, though the gap between them is a hair below zero
		let setup = Setup::new(&[[0., 0., 100., 60.], [100. - 1e-8, 0., 200., 60.]]);
		assert!(setup.spacing(&spaced([203., 10., 253., 50.], RIGID)).is_empty());

		// Exactly touching, there is a gap of nothing to continue
		let setup = Setup::new(&[[0., 0., 100., 60.], [100., 0., 200., 60.]]);
		assert_eq!(setup.spacing(&spaced([203., 10., 253., 50.], RIGID)).len(), 1);
	}

	#[test]
	fn boxes_that_do_not_overlap_the_moved_box_across_the_axis_are_not_its_neighbors() {
		let setup = Setup::new(&APART);

		// Wholly below the two boxes, and then only touching them across the axis
		assert!(setup.spacing(&spaced([170., 70., 220., 110.], RIGID)).is_empty());
		assert!(setup.spacing(&spaced([170., 60., 220., 100.], RIGID)).is_empty());
	}

	#[test]
	fn a_box_overlapping_the_moved_box_along_the_axis_is_not_its_neighbor() {
		let setup = Setup::new(&[[0., 0., 100., 60.], [151., 0., 251., 60.]]);
		assert_eq!(setup.spacing(&spaced([100.5, 10., 150.5, 50.], RIGID)).len(), 1);

		// A little further along, the moved box overlaps the box on its right
		assert!(setup.spacing(&spaced([102., 10., 152., 50.], RIGID)).is_empty());
	}

	#[test]
	fn excluded_layers_and_artboards_are_not_neighbors() {
		let mut setup = Setup::new(&APART);
		let moved = spaced([170., 10., 220., 50.], RIGID);
		assert_eq!(setup.spacing(&moved).len(), 1);

		setup.exclusions.layers.insert(LayerId(2));
		assert!(setup.spacing(&moved).is_empty());

		setup.exclusions = Exclusions {
			edited: [LayerId(2)].into(),
			..Default::default()
		};
		assert!(setup.spacing(&moved).is_empty());

		setup.exclusions = Exclusions::default();
		setup.scene.set_layer(LayerId(2), layer(APART[1], LayerKind::Artboard));
		assert!(setup.spacing(&moved).is_empty());
	}

	#[test]
	fn an_artboard_is_spaced_among_artboards_and_not_among_artwork() {
		let mut setup = Setup::new(&APART);
		let moved = SpacedBox {
			kind: LayerKind::Artboard,
			..spaced([170., 10., 220., 50.], RIGID)
		};
		assert!(setup.spacing(&moved).is_empty());

		for (index, &rectangle) in APART.iter().enumerate() {
			setup.scene.set_layer(LayerId(index as u64 + 1), layer(rectangle, LayerKind::Artboard));
		}
		assert_eq!(setup.spacing(&moved).len(), 1);
	}

	#[test]
	fn nothing_is_offered_that_would_carry_a_side_beyond_the_gathering_distance_on_screen() {
		let mut setup = Setup::new(&APART);

		// The centered place is at 175, and the gathering distance is just under 17 pixels
		assert_eq!(setup.spacing(&spaced([158.5, 10., 208.5, 50.], RIGID)).len(), 1);
		assert!(setup.spacing(&spaced([157.5, 10., 207.5, 50.], RIGID)).is_empty());

		// Zoomed in to twice the size, each unit of travel appears twice as far
		setup.view = ViewMapping::from_affine(DAffine2::from_scale(DVec2::splat(2.))).unwrap();
		assert_eq!(setup.spacing(&spaced([167., 10., 217., 50.], RIGID)).len(), 1);
		assert!(setup.spacing(&spaced([166., 10., 216., 50.], RIGID)).is_empty());
	}

	#[test]
	fn a_stretch_that_would_turn_the_box_inside_out_is_not_offered() {
		// Matching the gap on the left would bring the right side back past the left side
		let setup = Setup::new(&[[0., 0., 100., 60.], [200., 0., 300., 60.]]);

		assert!(setup.spacing(&spaced([151., 10., 152., 50.], RIGHT_SIDE)).is_empty());
	}

	#[test]
	fn a_frame_turned_from_the_plane_measures_along_its_own_axes() {
		let turn = DMat2::from_angle(0.4);
		let mut setup = Setup::new(&[]);
		for (index, rectangle) in APART.into_iter().enumerate() {
			let turned = SceneLayer {
				quad: Some(corners(rectangle).map(|corner| turn * corner)),
				..layer(rectangle, LayerKind::Artwork)
			};
			setup.scene.set_layer(LayerId(index as u64 + 1), turned);
		}
		setup.frame = Frame::new(DVec2::ZERO, [turn * DVec2::X, turn * DVec2::Y]).unwrap();

		let mut moved = spaced([170., 10., 220., 50.], RIGID);
		moved.corners = moved.corners.map(|corner| turn * corner);
		moved.sides = moved.sides.map(|sides| {
			sides.map(|side| {
				side.map(|side| Side {
					position: turn * side.position,
					..side
				})
			})
		});
		let candidates = setup.spacing(&moved);

		assert_eq!(candidates.len(), 1);
		assert!(landing(&candidates[0], &moved).abs_diff_eq(turn * DVec2::new(175., 10.), 1e-9));
	}

	#[test]
	fn a_frame_with_axes_not_at_right_angles_offers_nothing() {
		let mut setup = Setup::new(&APART);
		setup.frame = Frame::new(DVec2::ZERO, [DVec2::X, DVec2::new(1., 2.)]).unwrap();

		assert!(setup.spacing(&spaced([170., 10., 220., 50.], RIGID)).is_empty());
		assert!(setup.equal_sizes(&spaced([130., 10., 266., 50.], RIGHT_SIDE)).is_empty());
	}

	#[test]
	fn a_stretched_box_is_offered_the_lengths_of_its_neighbors_and_of_awake_boxes() {
		// One box sits above the stretched box and another is far away but awake, 150 and 148 wide
		let mut setup = Setup::new(&[[20., 0., 170., 60.], [500., 500., 648., 520.]]);
		setup.awake = vec![Awake::Box { layer: LayerId(2) }];

		let right = spaced([0., 100., 146., 150.], RIGHT_SIDE);
		let candidates = setup.equal_sizes(&right);
		assert_eq!(candidates.len(), 2);
		for (candidate, (layer, x)) in candidates.iter().zip([(1, 150.), (2, 148.)]) {
			assert_eq!(candidate.key.kind, Kind::EqualSize);
			assert_eq!(candidate.highlight, Some(Highlight::Box { layer: LayerId(layer) }));
			assert!(landing(candidate, &right).abs_diff_eq(DVec2::new(x, 100.), 1e-9));
		}

		// The length is measured from whichever side stays put
		let left = spaced([4., 100., 150., 150.], LEFT_SIDE);
		let landings: Vec<DVec2> = setup.equal_sizes(&left).iter().map(|candidate| landing(candidate, &left)).collect();
		assert_eq!(landings.len(), 2);
		assert!(landings[0].abs_diff_eq(DVec2::new(0., 100.), 1e-9) && landings[1].abs_diff_eq(DVec2::new(2., 100.), 1e-9));

		// The box above is a neighbor along the second axis as well, where its height is offered
		let taller = spaced([0., 100., 146., 157.], BOTTOM_SIDE);
		let candidates = setup.equal_sizes(&taller);
		assert_eq!(candidates.len(), 1);
		assert!(landing(&candidates[0], &taller).abs_diff_eq(DVec2::new(0., 160.), 1e-9));
	}

	#[test]
	fn a_box_reached_both_as_a_neighbor_and_awake_is_offered_once() {
		let mut setup = Setup::new(&[[20., 0., 170., 60.]]);
		setup.awake = vec![Awake::Box { layer: LayerId(1) }, Awake::Box { layer: LayerId(1) }];

		assert_eq!(setup.equal_sizes(&spaced([0., 100., 146., 150.], RIGHT_SIDE)).len(), 1);
	}

	#[test]
	fn sizes_are_offered_only_to_one_side_stretched_within_the_gathering_distance_of_them() {
		let mut setup = Setup::new(&[[20., 0., 170., 60.], [500., 500., 648., 520.]]);
		setup.awake = vec![Awake::Box { layer: LayerId(2) }];

		assert!(setup.equal_sizes(&spaced([0., 100., 146., 150.], RIGID)).is_empty());
		assert!(setup.equal_sizes(&spaced([0., 100., 126., 150.], RIGHT_SIDE)).is_empty());

		// An awake box that the gesture is moving offers no length
		setup.exclusions.layers.insert(LayerId(2));
		assert_eq!(setup.equal_sizes(&spaced([0., 100., 146., 150.], RIGHT_SIDE)).len(), 1);
	}
}
