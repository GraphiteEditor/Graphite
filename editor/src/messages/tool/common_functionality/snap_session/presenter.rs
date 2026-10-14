use super::{Shown, SnapSession};
use crate::consts::{COLOR_OVERLAY_BLACK_75, COLOR_OVERLAY_BLUE, COLOR_OVERLAY_BLUE_50, COLOR_OVERLAY_WHITE};
use crate::messages::portfolio::document::overlays::utility_types::{OverlayContext, Pivot};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::shapes::shape_utility::format_rounded;
use glam::{DAffine2, DVec2};
use graphene_std::renderer::Quad;
use snapping::{Awake, Candidate, Highlight, Kind, Locus, Shape};

/// How far the cap at each end of a drawn gap reaches to either side of it, in viewport pixels.
const GAP_CAP_REACH: f64 = 4.;

impl SnapSession {
	/// Draws the woken targets, and then what the pointer is snapped to: the piece of the scene it found, any line it is held on, and a mark at the snapped position with a word for the relationship.
	pub fn draw_overlays(&self, document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
		let to_viewport = document.metadata().document_to_viewport;
		for &target in &self.awake {
			self.draw_awake(target, to_viewport, overlay_context);
		}

		let Some(Shown { outcome, features, .. }) = &self.shown else { return };

		let engaged: Vec<(&Candidate, DVec2)> = outcome
			.engaged_candidates()
			.filter_map(|candidate| Some((candidate, *features.get(candidate.key.feature as usize)?)))
			.collect();
		let Some(&(_, marked)) = engaged.first() else { return };

		for &(candidate, snapped) in &engaged {
			if let Some(highlight) = candidate.highlight {
				self.draw_highlight(highlight, to_viewport, overlay_context);
			}

			// A line the feature is held on is drawn from what the line was measured from to where the feature now is
			if let (Shape::Track(Locus::Line { .. }), Some(reference)) = (candidate.shape, candidate.reference) {
				let [from, to] = [reference, snapped].map(|position| to_viewport.transform_point2(position));
				overlay_context.line(from, to, None, None);
				overlay_context.manipulator_handle(from, false, None);
			}

			if let Some(gaps) = candidate.gaps {
				for span in gaps.spans {
					draw_gap(span, gaps.gap, to_viewport, overlay_context);
				}
			}
		}

		let marked = to_viewport.transform_point2(marked);
		let above = DAffine2::from_translation(marked - DVec2::new(0., 4.));
		overlay_context.text(&marked_label(&engaged), COLOR_OVERLAY_WHITE, Some(COLOR_OVERLAY_BLACK_75), above, 4., [Pivot::Start, Pivot::End]);
		overlay_context.square(marked, Some(4.), Some(COLOR_OVERLAY_BLUE), Some(COLOR_OVERLAY_BLUE));
	}

	/// Marks a woken target softly, which stays marked wherever the pointer goes.
	fn draw_awake(&self, target: Awake, to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
		match target {
			Awake::Anchor { layer, index } => {
				if let Some(anchor) = self.scene.layer(layer).and_then(|layer| layer.anchors.get(index as usize)) {
					overlay_context.circle(to_viewport.transform_point2(anchor.position), 5., None, Some(COLOR_OVERLAY_BLUE_50));
				}
			}
			Awake::Curve { layer, index } => {
				if let Some(curve) = self.scene.layer(layer).and_then(|layer| layer.curves.get(index as usize)) {
					overlay_context.outline_bezier(curve.curve.segment(), to_viewport);
				}
			}
			Awake::Box { layer } => {
				if let Some(corners) = self.scene.layer(layer).and_then(|layer| layer.quad) {
					overlay_context.dashed_quad(to_viewport * Quad(corners), Some(COLOR_OVERLAY_BLUE_50), None, Some(4.), Some(4.), None);
				}
			}
			Awake::Position { position, .. } => {
				overlay_context.circle(to_viewport.transform_point2(position), 5., None, Some(COLOR_OVERLAY_BLUE_50));
			}
		}
	}

	/// Outlines the piece of the scene a snap was found on. An anchor needs none, since the mark at the snapped position sits on it.
	fn draw_highlight(&self, highlight: Highlight, to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
		match highlight {
			Highlight::Anchor { .. } => {}
			Highlight::Curve { layer, index } => {
				if let Some(curve) = self.scene.layer(layer).and_then(|layer| layer.curves.get(index as usize)) {
					overlay_context.outline_bezier(curve.curve.segment(), to_viewport);
				}
			}
			Highlight::Box { layer } => {
				if let Some(corners) = self.scene.layer(layer).and_then(|layer| layer.quad) {
					overlay_context.quad(to_viewport * Quad(corners), None, None);
				}
			}
		}
	}
}

/// The label for the mark on the first engaged feature, naming only what holds that feature, since other features may be held by unrelated things.
fn marked_label(engaged: &[(&Candidate, DVec2)]) -> String {
	let Some((first, _)) = engaged.first() else { return String::new() };
	let kinds: Vec<Kind> = engaged
		.iter()
		.filter(|(candidate, _)| candidate.key.feature == first.key.feature)
		.map(|(candidate, _)| candidate.key.kind)
		.collect();

	label(&kinds)
}

/// Draws one of the gaps a snap has made equal: a line across it with a cap at each end, and its length in document units.
fn draw_gap(span: [DVec2; 2], gap: f64, to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	let [from, to] = span.map(|position| to_viewport.transform_point2(position));
	let Some(cap) = (to - from).try_normalize().map(|along| along.perp() * GAP_CAP_REACH) else {
		return;
	};

	overlay_context.line(from, to, None, None);
	overlay_context.line(from - cap, from + cap, None, None);
	overlay_context.line(to - cap, to + cap, None, None);

	// The length goes above a gap running across and to the right of one running up and down, clear of its line either way
	let runs_across = (to - from).x.abs() >= (to - from).y.abs();
	let pivot = if runs_across { [Pivot::Middle, Pivot::End] } else { [Pivot::Start, Pivot::Middle] };
	let beside = DAffine2::from_translation((from + to) / 2.);
	overlay_context.text(&format_rounded(gap, 2), COLOR_OVERLAY_BLUE, None, beside, 5., pivot);
}

/// The relationship a snap gives, in a word or two.
fn label(kinds: &[Kind]) -> String {
	let follows_geometry = |kind: &Kind| matches!(kind, Kind::OnCurve | Kind::OnBoxEdge | Kind::OnArtboardEdge);

	match kinds {
		[kind] => name(*kind).to_string(),
		[first, second] if follows_geometry(first) && follows_geometry(second) => "Intersection".to_string(),
		[Kind::Alignment, Kind::Alignment] => "Aligned Both Ways".to_string(),
		[first, second] if first == second => name(*first).to_string(),
		[first, second] => format!("{} and {}", name(*first), name(*second)),
		_ => String::new(),
	}
}

fn name(kind: Kind) -> &'static str {
	match kind {
		Kind::Anchor => "Anchor",
		Kind::Midpoint => "Midpoint",
		Kind::Between => "Halfway",
		Kind::TangentPoint | Kind::TangentLine => "Tangent",
		Kind::PerpendicularFoot | Kind::PerpendicularLine => "Perpendicular",
		Kind::BoxCorner => "Corner",
		Kind::BoxEdgeMidpoint => "Edge Midpoint",
		Kind::BoxCenter => "Center",
		Kind::ArtboardCorner => "Artboard Corner",
		Kind::ArtboardCenter => "Artboard Center",
		Kind::OnCurve => "On Path",
		Kind::OnBoxEdge => "On Edge",
		Kind::OnArtboardEdge => "On Artboard Edge",
		Kind::Extension => "Extension",
		Kind::Parallel => "Parallel",
		Kind::RightAngle => "Right Angle",
		Kind::Through => "In Line",
		Kind::CommonTangent => "Tangent to Both",
		Kind::Alignment => "Aligned",
		Kind::EqualLength => "Equal Length",
		Kind::AngleStep => "Angle",
		Kind::Spacing => "Evenly Spaced",
		Kind::EqualSize => "Equal Size",
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_snap_is_named_for_its_relationship() {
		assert_eq!(label(&[Kind::Anchor]), "Anchor");
		assert_eq!(label(&[Kind::OnCurve, Kind::OnBoxEdge]), "Intersection");
		assert_eq!(label(&[Kind::Alignment, Kind::Alignment]), "Aligned Both Ways");
		assert_eq!(label(&[Kind::Extension, Kind::Extension]), "Extension");
		assert_eq!(label(&[Kind::OnCurve, Kind::Alignment]), "On Path and Aligned");
		assert_eq!(label(&[Kind::TangentLine, Kind::Extension]), "Tangent and Extension");
	}

	#[test]
	fn the_mark_is_named_for_what_holds_its_own_feature() {
		let on = |kind: Kind, feature: u32, identity: u64| {
			Candidate::spot(
				snapping::CandidateKey {
					kind,
					feature,
					identity: [identity, 0, 0],
				},
				DVec2::ZERO,
			)
		};
		let [path, edge, other_path] = [on(Kind::OnCurve, 0, 1), on(Kind::OnBoxEdge, 0, 2), on(Kind::OnCurve, 1, 3)];

		// Two paths holding the same feature meet there
		assert_eq!(marked_label(&[(&path, DVec2::ZERO), (&edge, DVec2::ZERO)]), "Intersection");

		// Each end of a line drawn from its middle on a path of its own isn't at a crossing
		assert_eq!(marked_label(&[(&path, DVec2::ZERO), (&other_path, DVec2::X)]), "On Path");
	}
}
