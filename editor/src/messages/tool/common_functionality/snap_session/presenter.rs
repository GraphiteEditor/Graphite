use super::{Shown, SnapSession};
use crate::consts::{COLOR_OVERLAY_BLACK_75, COLOR_OVERLAY_BLUE, COLOR_OVERLAY_BLUE_50, COLOR_OVERLAY_WHITE};
use crate::messages::portfolio::document::overlays::utility_types::{OverlayContext, Pivot};
use crate::messages::prelude::*;
use glam::{DAffine2, DVec2};
use graphene_std::renderer::Quad;
use snapping::{Awake, Candidate, Highlight, Kind, Locus, Shape};

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
		}

		let kinds: Vec<Kind> = engaged.iter().map(|(candidate, _)| candidate.key.kind).collect();
		let marked = to_viewport.transform_point2(marked);
		let above = DAffine2::from_translation(marked - DVec2::new(0., 4.));
		overlay_context.text(&label(&kinds), COLOR_OVERLAY_WHITE, Some(COLOR_OVERLAY_BLACK_75), above, 4., [Pivot::Start, Pivot::End]);
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
}
