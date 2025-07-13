mod alignment_snapper;
mod distribution_snapper;
mod grid_snapper;
mod layer_snapper;
mod snap_results;

use crate::consts::{COLOR_OVERLAY_BLACK_75, COLOR_OVERLAY_BLUE, COLOR_OVERLAY_WHITE};
use crate::messages::portfolio::document::overlays::utility_types::{OverlayContext, Pivot};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::misc::{GridSnapTarget, PathSnapTarget, SnapTarget};
use crate::messages::prelude::*;
pub use alignment_snapper::*;
use bezier_rs::TValue;
pub use distribution_snapper::*;
use glam::{DAffine2, DVec2};
use graphene_std::renderer::Quad;
use graphene_std::renderer::Rect;
use graphene_std::vector::NoHashBuilder;
use graphene_std::vector::PointId;
pub use grid_snapper::*;
pub use layer_snapper::*;
pub use snap_results::*;
use std::cmp::Ordering;

/// Configuration for the relevant snap type
#[derive(Debug, Clone, Copy, Default)]
pub struct SnapTypeConfiguration {
	pub only_path: bool,
	pub use_existing_candidates: bool,
	pub accept_distribution: bool,
	pub bbox: Option<Rect>,
}

/// Handles snapping and snap overlays
#[derive(Debug, Clone, Default)]
pub struct SnapManager {
	indicator: Option<SnappedPoint>,
	layer_snapper: LayerSnapper,
	grid_snapper: GridSnapper,
	alignment_snapper: AlignmentSnapper,
	distribution_snapper: DistributionSnapper,
	candidates: Option<Vec<LayerNodeIdentifier>>,
	alignment_candidates: Option<Vec<LayerNodeIdentifier>>,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum SnapConstraint {
	#[default]
	None,
	Line {
		origin: DVec2,
		direction: DVec2,
	},
	Direction(DVec2),
	Circle {
		center: DVec2,
		radius: f64,
	},
}
impl SnapConstraint {
	pub fn projection(&self, point: DVec2) -> DVec2 {
		match *self {
			Self::Line { origin, direction } if direction != DVec2::ZERO => (point - origin).project_onto(direction) + origin,
			Self::Circle { center, radius } => {
				let from_center = point - center;
				let distance = from_center.length();
				if distance > 0. {
					center + radius * from_center / distance
				} else {
					// Point is exactly at the center, so project right
					center + DVec2::new(radius, 0.)
				}
			}
			_ => point,
		}
	}
	pub fn direction(&self) -> DVec2 {
		match *self {
			Self::Line { direction, .. } | Self::Direction(direction) => direction,
			_ => DVec2::ZERO,
		}
	}
}
pub fn snap_tolerance(document: &DocumentMessageHandler) -> f64 {
	document.snapping_state.tolerance / document.document_ptz.zoom()
}

fn compare_points(a: &&SnappedPoint, b: &&SnappedPoint) -> Ordering {
	if (a.target.bounding_box() && !b.target.bounding_box()) || (a.at_intersection && !b.at_intersection) || (a.source.bounding_box() && !b.source.bounding_box()) {
		Ordering::Greater
	} else if (!a.target.bounding_box() && b.target.bounding_box()) || (!a.at_intersection && b.at_intersection) || (!a.source.bounding_box() && b.source.bounding_box()) {
		Ordering::Less
	} else {
		a.distance.partial_cmp(&b.distance).unwrap()
	}
}

fn find_align(a: &SnappedPoint, b: &SnappedPoint) -> Ordering {
	(a.distance, a.distance_to_align_target).partial_cmp(&(b.distance, b.distance_to_align_target)).unwrap()
}

fn get_closest_point(points: Vec<SnappedPoint>) -> Option<SnappedPoint> {
	let mut best_not_align = None;
	let mut best_align = None;
	for point in points {
		if !point.align() && !best_not_align.as_ref().is_some_and(|best| compare_points(&best, &&point).is_ge()) {
			best_not_align = Some(point);
		} else if point.align() && !best_align.as_ref().is_some_and(|best| find_align(best, &point).is_ge()) {
			best_align = Some(point)
		}
	}
	match (best_not_align, best_align) {
		(None, None) => None,
		(Some(result), None) | (None, Some(result)) => Some(result),
		(Some(mut result), Some(align)) => {
			let SnapTarget::DistributeEvenly(distribution) = result.target else { return Some(result) };
			if distribution.is_x() && align.alignment_target_horizontal.is_some() {
				result.snapped_point_document.y = align.snapped_point_document.y;
				result.alignment_target_horizontal = align.alignment_target_horizontal;
			}
			if distribution.is_y() && align.alignment_target_vertical.is_some() {
				result.snapped_point_document.x = align.snapped_point_document.x;
				result.alignment_target_vertical = align.alignment_target_vertical;
			}

			Some(result)
		}
	}
}
fn get_closest_curve(curves: &[SnappedCurve], exclude_paths: bool) -> Option<&SnappedPoint> {
	let keep_curve = |curve: &&SnappedCurve| !exclude_paths || curve.point.target != SnapTarget::Path(PathSnapTarget::AlongPath);
	curves.iter().filter(keep_curve).map(|curve| &curve.point).min_by(compare_points)
}
fn get_closest_line(lines: &[SnappedLine]) -> Option<&SnappedPoint> {
	lines.iter().map(|curve| &curve.point).min_by(compare_points)
}
fn get_closest_intersection(snap_to: DVec2, curves: &[SnappedCurve]) -> Option<SnappedPoint> {
	let mut best = None;
	for curve_i in curves {
		for curve_j in curves {
			if curve_i.start == curve_j.start && curve_i.layer == curve_j.layer {
				continue;
			}
			for curve_i_t in curve_i.document_curve.intersections(&curve_j.document_curve, None, None) {
				let snapped_point_document = curve_i.document_curve.evaluate(TValue::Parametric(curve_i_t));
				let distance = snap_to.distance(snapped_point_document);
				let i_closer = curve_i.point.distance < curve_j.point.distance;
				let close = if i_closer { curve_i } else { curve_j };
				let far = if i_closer { curve_j } else { curve_i };
				if !best.as_ref().is_some_and(|best: &SnappedPoint| best.distance < distance) {
					best = Some(SnappedPoint {
						snapped_point_document,
						distance,
						target: SnapTarget::Path(PathSnapTarget::IntersectionPoint),
						tolerance: close.point.tolerance,
						outline_layers: [Some(close.layer), Some(far.layer)],
						source: close.point.source,
						at_intersection: true,
						constrained: true,
						..Default::default()
					})
				}
			}
		}
	}
	best
}
fn get_grid_intersection(snap_to: DVec2, lines: &[SnappedLine]) -> Option<SnappedPoint> {
	let mut best = None;
	for line_i in lines {
		for line_j in lines {
			if let Some(snapped_point_document) = Quad::intersect_rays(line_i.point.snapped_point_document, line_i.direction, line_j.point.snapped_point_document, line_j.direction) {
				let distance = snap_to.distance(snapped_point_document);
				if !best.as_ref().is_some_and(|best: &SnappedPoint| best.distance < distance) {
					best = Some(SnappedPoint {
						snapped_point_document,
						distance,
						target: SnapTarget::Grid(GridSnapTarget::Intersection),
						tolerance: line_i.point.tolerance,
						source: line_i.point.source,
						at_intersection: true,
						constrained: true,
						..Default::default()
					})
				}
			}
		}
	}
	best
}

#[derive(Default, Clone, Debug)]
pub struct SnapCache {
	pub manipulators: HashMap<LayerNodeIdentifier, HashSet<PointId, NoHashBuilder>, NoHashBuilder>,
	pub unselected: Vec<SnapCandidatePoint>,
}

#[derive(Clone)]
pub struct SnapData<'a> {
	pub document: &'a DocumentMessageHandler,
	pub input: &'a InputPreprocessorMessageHandler,
	pub ignore: &'a [LayerNodeIdentifier],
	pub node_snap_cache: Option<&'a SnapCache>,
	pub candidates: Option<&'a Vec<LayerNodeIdentifier>>,
	pub alignment_candidates: Option<&'a Vec<LayerNodeIdentifier>>,
}
impl<'a> SnapData<'a> {
	pub fn new(document: &'a DocumentMessageHandler, input: &'a InputPreprocessorMessageHandler) -> Self {
		Self::ignore(document, input, &[])
	}
	pub fn ignore(document: &'a DocumentMessageHandler, input: &'a InputPreprocessorMessageHandler, ignore: &'a [LayerNodeIdentifier]) -> Self {
		Self {
			document,
			input,
			ignore,
			candidates: None,
			alignment_candidates: None,
			node_snap_cache: None,
		}
	}
	pub fn new_snap_cache(document: &'a DocumentMessageHandler, input: &'a InputPreprocessorMessageHandler, snap_cache: &'a SnapCache) -> Self {
		Self {
			node_snap_cache: Some(snap_cache),
			..Self::new(document, input)
		}
	}
	fn get_candidates(&self) -> &[LayerNodeIdentifier] {
		self.candidates.map_or([].as_slice(), |candidates| candidates.as_slice())
	}
	fn ignore_bounds(&self, layer: LayerNodeIdentifier) -> bool {
		self.node_snap_cache.is_some_and(|cache| cache.manipulators.contains_key(&layer))
	}
	fn ignore_manipulator(&self, layer: LayerNodeIdentifier, target: PointId) -> bool {
		self.node_snap_cache.and_then(|cache| cache.manipulators.get(&layer)).is_some_and(|points| points.contains(&target))
	}
	fn has_manipulators(&self) -> bool {
		self.node_snap_cache.is_some_and(|cache| !cache.manipulators.is_empty())
	}
}
impl SnapManager {
	pub fn update_indicator(&mut self, snapped_point: SnappedPoint) {
		self.indicator = snapped_point.is_snapped().then_some(snapped_point);
	}
	pub fn clear_indicator(&mut self) {
		self.indicator = None;
	}
	pub fn preview_draw(&mut self, snap_data: &SnapData, mouse: DVec2) {
		let point = SnapCandidatePoint::handle(snap_data.document.metadata().document_to_viewport.inverse().transform_point2(mouse));
		let snapped = self.free_snap(snap_data, &point, SnapTypeConfiguration::default());
		self.update_indicator(snapped);
	}

	pub fn indicator_pos(&self) -> Option<DVec2> {
		self.indicator.as_ref().map(|point| point.snapped_point_document)
	}

	fn find_best_snap(snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: SnapResults, constrained: bool, off_screen: bool, to_path: bool) -> SnappedPoint {
		let mut snapped_points = Vec::new();
		let document = snap_data.document;

		if let Some(closest_point) = get_closest_point(snap_results.points) {
			snapped_points.push(closest_point);
		}
		let exclude_paths = !document.snapping_state.target_enabled(SnapTarget::Path(PathSnapTarget::AlongPath));
		if let Some(closest_curve) = get_closest_curve(&snap_results.curves, exclude_paths) {
			snapped_points.push(closest_curve.clone());
		}

		if document.snapping_state.target_enabled(SnapTarget::Grid(GridSnapTarget::Line)) {
			if let Some(closest_line) = get_closest_line(&snap_results.grid_lines) {
				snapped_points.push(closest_line.clone());
			}
		}

		if !constrained {
			if document.snapping_state.target_enabled(SnapTarget::Path(PathSnapTarget::IntersectionPoint)) {
				if let Some(closest_curves_intersection) = get_closest_intersection(point.document_point, &snap_results.curves) {
					snapped_points.push(closest_curves_intersection);
				}
			}
			if document.snapping_state.target_enabled(SnapTarget::Grid(GridSnapTarget::Intersection)) {
				if let Some(closest_grid_intersection) = get_grid_intersection(point.document_point, &snap_results.grid_lines) {
					snapped_points.push(closest_grid_intersection);
				}
			}
		}

		if to_path {
			snapped_points.retain(|i| matches!(i.target, SnapTarget::Path(_)));
		}

		let mut best_point = None;

		for point in snapped_points {
			let viewport_point = document.metadata().document_to_viewport.transform_point2(point.snapped_point_document);
			let on_screen = viewport_point.cmpgt(DVec2::ZERO).all() && viewport_point.cmplt(snap_data.input.viewport_bounds.size()).all();
			if !on_screen && !off_screen {
				continue;
			}
			if point.distance > point.tolerance {
				continue;
			}
			if best_point.as_ref().is_some_and(|best: &SnappedPoint| point.other_snap_better(best)) {
				continue;
			}
			best_point = Some(point);
		}

		best_point.unwrap_or(SnappedPoint::infinite_snap(point.document_point))
	}

	fn add_candidates(&mut self, layer: LayerNodeIdentifier, snap_data: &SnapData, quad: Quad) {
		let document = snap_data.document;

		if !document.network_interface.is_visible(&layer.to_node(), &[]) {
			return;
		}
		if snap_data.ignore.contains(&layer) {
			return;
		}
		if layer.has_children(document.metadata()) {
			for layer in layer.children(document.metadata()) {
				self.add_candidates(layer, snap_data, quad);
			}
			return;
		}
		let Some(bounds) = document.metadata().bounding_box_with_transform(layer, DAffine2::IDENTITY) else {
			return;
		};
		let layer_bounds = document.metadata().transform_to_document(layer) * Quad::from_box(bounds);
		let screen_bounds = document.metadata().document_to_viewport.inverse() * Quad::from_box([DVec2::ZERO, snap_data.input.viewport_bounds.size()]);
		if screen_bounds.intersects(layer_bounds) {
			if self.alignment_candidates.as_ref().is_none_or(|candidates| candidates.len() <= 100) {
				self.alignment_candidates.get_or_insert_with(Vec::new).push(layer);
			}
			if quad.intersects(layer_bounds) && self.candidates.as_ref().is_none_or(|candidates| candidates.len() <= 10) {
				self.candidates.get_or_insert_with(Vec::new).push(layer);
			}
		}
	}

	fn find_candidates(&mut self, snap_data: &SnapData, point: &SnapCandidatePoint, bbox: Option<Rect>) {
		let document = snap_data.document;
		let offset = snap_tolerance(document);
		let quad = bbox.map_or_else(|| Quad::from_square(point.document_point, offset), |quad| Quad::from_box(quad.0).inflate(offset));

		self.candidates = None;
		self.alignment_candidates = None;
		for layer in LayerNodeIdentifier::ROOT_PARENT.children(document.metadata()) {
			self.add_candidates(layer, snap_data, quad);
		}

		if self.alignment_candidates.as_ref().is_some_and(|candidates| candidates.len() > crate::consts::MAX_ALIGNMENT_CANDIDATES) {
			warn!("Alignment candidate overflow");
		}
		if self.candidates.as_ref().is_some_and(|candidates| candidates.len() > crate::consts::MAX_SNAP_CANDIDATES) {
			warn!("Snap candidate overflow");
		}
	}

	pub fn free_snap(&mut self, snap_data: &SnapData, point: &SnapCandidatePoint, config: SnapTypeConfiguration) -> SnappedPoint {
		if !point.document_point.is_finite() {
			warn!("Snapping non-finite position");
			return SnappedPoint::infinite_snap(DVec2::ZERO);
		}

		let mut snap_results = SnapResults::default();
		if !config.use_existing_candidates {
			self.candidates = None;
		}

		let mut snap_data = snap_data.clone();
		if snap_data.candidates.is_none() {
			self.find_candidates(&snap_data, point, config.bbox);
		}
		snap_data.candidates = self.candidates.as_ref();
		snap_data.alignment_candidates = self.alignment_candidates.as_ref();

		self.layer_snapper.free_snap(&mut snap_data, point, &mut snap_results, config);
		self.grid_snapper.free_snap(&mut snap_data, point, &mut snap_results);
		self.alignment_snapper.free_snap(&mut snap_data, point, &mut snap_results, config);
		self.distribution_snapper.free_snap(&mut snap_data, point, &mut snap_results, config);

		Self::find_best_snap(&mut snap_data, point, snap_results, false, false, config.only_path)
	}

	pub fn constrained_snap(&mut self, snap_data: &SnapData, point: &SnapCandidatePoint, constraint: SnapConstraint, config: SnapTypeConfiguration) -> SnappedPoint {
		if !point.document_point.is_finite() {
			warn!("Snapping non-finite position");
			return SnappedPoint::infinite_snap(DVec2::ZERO);
		}

		let mut snap_results = SnapResults::default();
		if !config.use_existing_candidates {
			self.candidates = None;
		}

		let mut snap_data = snap_data.clone();
		if snap_data.candidates.is_none() {
			self.find_candidates(&snap_data, point, config.bbox);
		}
		snap_data.candidates = self.candidates.as_ref();
		snap_data.alignment_candidates = self.alignment_candidates.as_ref();

		self.layer_snapper.constrained_snap(&mut snap_data, point, &mut snap_results, constraint, config);
		self.grid_snapper.constrained_snap(&mut snap_data, point, &mut snap_results, constraint);
		self.alignment_snapper.constrained_snap(&mut snap_data, point, &mut snap_results, constraint, config);
		self.distribution_snapper.constrained_snap(&mut snap_data, point, &mut snap_results, constraint, config);

		Self::find_best_snap(&mut snap_data, point, snap_results, true, false, config.only_path)
	}

	fn alignment_x_overlay(boxes: &VecDeque<Rect>, transform: DAffine2, overlay_context: &mut OverlayContext) {
		let y_size = transform.inverse().transform_vector2(DVec2::Y * 8.).length();
		for (&first, &second) in boxes.iter().zip(boxes.iter().skip(1)) {
			let bottom = first.center().y < second.center().y + y_size;
			let y = if bottom { first.max() } else { first.min() }.y;
			let start = DVec2::new(first.max().x, y);
			let end = DVec2::new(second.min().x, y);
			let signed_size = if bottom { y_size } else { -y_size };
			overlay_context.line(transform.transform_point2(start), transform.transform_point2(start + DVec2::Y * signed_size), None, None);
			overlay_context.line(transform.transform_point2(end), transform.transform_point2(end + DVec2::Y * signed_size), None, None);
			overlay_context.line(
				transform.transform_point2(start + DVec2::Y * signed_size / 2.),
				transform.transform_point2(end + DVec2::Y * signed_size / 2.),
				None,
				None,
			);
		}
	}

	fn alignment_y_overlay(boxes: &VecDeque<Rect>, transform: DAffine2, overlay_context: &mut OverlayContext) {
		let x_size = transform.inverse().transform_vector2(DVec2::X * 8.).length();
		for (&first, &second) in boxes.iter().zip(boxes.iter().skip(1)) {
			let right = first.center().x < second.center().x + x_size;
			let x = if right { first.max() } else { first.min() }.x;
			let start = DVec2::new(x, first.max().y);
			let end = DVec2::new(x, second.min().y);
			let signed_size = if right { x_size } else { -x_size };
			overlay_context.line(transform.transform_point2(start), transform.transform_point2(start + DVec2::X * signed_size), None, None);
			overlay_context.line(transform.transform_point2(end), transform.transform_point2(end + DVec2::X * signed_size), None, None);
			overlay_context.line(
				transform.transform_point2(start + DVec2::X * signed_size / 2.),
				transform.transform_point2(end + DVec2::X * signed_size / 2.),
				None,
				None,
			);
		}
	}

	pub fn draw_overlays(&mut self, snap_data: SnapData, overlay_context: &mut OverlayContext) {
		let to_viewport = snap_data.document.metadata().document_to_viewport;
		if let Some(ind) = &self.indicator {
			for layer in &ind.outline_layers {
				let &Some(layer) = layer else { continue };
				overlay_context.outline(
					snap_data.document.metadata().layer_with_free_points_outline(layer),
					snap_data.document.metadata().transform_to_viewport(layer),
					None,
				);
			}
			if let Some(quad) = ind.target_bounds {
				overlay_context.quad(to_viewport * quad, None, None);
			}
			let viewport = to_viewport.transform_point2(ind.snapped_point_document);

			Self::alignment_x_overlay(&ind.distribution_boxes_horizontal, to_viewport, overlay_context);
			Self::alignment_y_overlay(&ind.distribution_boxes_vertical, to_viewport, overlay_context);

			let align = [ind.alignment_target_horizontal, ind.alignment_target_vertical].map(|target| target.map(|target| to_viewport.transform_point2(target)));
			let any_align = align.iter().flatten().next().is_some();
			for &target in align.iter().flatten() {
				overlay_context.line(viewport, target, None, None);
			}
			for &target in align.iter().flatten() {
				overlay_context.manipulator_handle(target, false, None);
			}
			if any_align {
				overlay_context.manipulator_handle(viewport, false, None);
			}

			if !any_align && ind.distribution_equal_distance_horizontal.is_none() && ind.distribution_equal_distance_vertical.is_none() {
				let text = format!("[{}] from [{}]", ind.target, ind.source);
				let transform = DAffine2::from_translation(viewport - DVec2::new(0., 4.));
				overlay_context.text(&text, COLOR_OVERLAY_WHITE, Some(COLOR_OVERLAY_BLACK_75), transform, 4., [Pivot::Start, Pivot::End]);
				overlay_context.square(viewport, Some(4.), Some(COLOR_OVERLAY_BLUE), Some(COLOR_OVERLAY_BLUE));
			}
		}
	}

	/// Removes snap target data and overlays. Call this when snapping is done.
	pub fn cleanup(&mut self, responses: &mut VecDeque<Message>) {
		self.candidates = None;
		self.indicator = None;
		responses.add(OverlaysMessage::Draw);
	}
}

/// Converts a bounding box into a set of points for snapping
///
/// Puts a point in the middle of each edge (top, bottom, left, right)
pub fn expand_bounds([bound1, bound2]: [DVec2; 2]) -> [DVec2; 4] {
	[
		DVec2::new((bound1.x + bound2.x) / 2., bound1.y),
		DVec2::new((bound1.x + bound2.x) / 2., bound2.y),
		DVec2::new(bound1.x, (bound1.y + bound2.y) / 2.),
		DVec2::new(bound2.x, (bound1.y + bound2.y) / 2.),
	]
}
