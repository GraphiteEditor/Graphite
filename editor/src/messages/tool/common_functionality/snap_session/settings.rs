use crate::messages::portfolio::document::utility_types::misc::{GridLevels, GridSnapping, GridType, SnappingState};
use crate::messages::prelude::*;
use glam::DVec2;
use snapping::{Grid, Settings};

/// The kinds of snap to offer, going by the document's snapping options.
pub fn settings(state: &SnappingState) -> Settings {
	Settings {
		anchors: state.path.anchor_point,
		midpoints: state.path.line_midpoint,
		curves: state.path.along_path,
		box_corners: state.bounding_box.corner_point,
		box_edge_midpoints: state.bounding_box.edge_midpoint,
		box_centers: state.bounding_box.center_point,
		box_edges: state.bounding_box.edges,
		artboards: state.artboards,
		tangents: state.path.tangent_to_path,
		perpendiculars: state.path.normal_to_path,
		point_alignment: state.path.align_with_anchor_point,
		box_alignment: state.bounding_box.align_with_edges,
		// These come only from targets that have been woken, and have no options of their own
		extensions: true,
		constructions: true,
		equal_lengths: true,
	}
}

/// The grid to land on, which is the finest one drawn at the document's present zoom. Returns `None` while grid snapping is off or the grid's options make no usable grid.
pub fn grid(document: &DocumentMessageHandler) -> Option<Grid> {
	grid_at_level(document, GridLevels::finest)
}

/// The grid of only the major lines drawn at the document's present zoom. An isometric grid has just the one level.
pub fn major_grid(document: &DocumentMessageHandler) -> Option<Grid> {
	grid_at_level(document, |levels| levels.major)
}

/// The grid drawn at the document's present zoom, with a rectangular grid's lines spaced as one of its levels.
fn grid_at_level(document: &DocumentMessageHandler, level: impl Fn(&GridLevels) -> DVec2) -> Option<Grid> {
	let state = &document.snapping_state;
	if !state.grid_snapping {
		return None;
	}
	let origin = state.grid.effective_origin(document);

	match state.grid.grid_type {
		GridType::Rectangular { .. } => {
			let levels = state.grid.drawn_levels(&document.document_ptz)?;
			Grid::new(origin, levels.families(level(&levels)))
		}
		GridType::Isometric { y_axis_spacing, angle_a, angle_b } => {
			let slopes = [-angle_a.to_radians().tan(), angle_b.to_radians().tan()];
			let spread = slopes[1] - slopes[0];
			let multiplier = GridSnapping::compute_isometric_multiplier(y_axis_spacing, spread, &document.document_ptz)?;

			// The slanted lines are spaced along Y, which is further apart than they are straight across
			let along_y = y_axis_spacing.abs() * multiplier;
			let slanted = slopes.map(|slope| (DVec2::new(1., slope), along_y / slope.hypot(1.)));
			Grid::new(origin, [(DVec2::Y, along_y / spread), slanted[0], slanted[1]])
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::messages::portfolio::document::utility_types::misc::SNAPPING_OPTION_GROUPS;

	/// The names of the engine's kinds of snap that some settings turn on.
	fn kinds_on(settings: &Settings) -> Vec<&'static str> {
		let Settings {
			anchors,
			midpoints,
			curves,
			box_corners,
			box_edge_midpoints,
			box_centers,
			box_edges,
			artboards,
			tangents,
			perpendiculars,
			point_alignment,
			box_alignment,
			extensions,
			constructions,
			equal_lengths,
		} = *settings;

		let kinds = [
			("anchors", anchors),
			("midpoints", midpoints),
			("curves", curves),
			("box_corners", box_corners),
			("box_edge_midpoints", box_edge_midpoints),
			("box_centers", box_centers),
			("box_edges", box_edges),
			("artboards", artboards),
			("tangents", tangents),
			("perpendiculars", perpendiculars),
			("point_alignment", point_alignment),
			("box_alignment", box_alignment),
			("extensions", extensions),
			("constructions", constructions),
			("equal_lengths", equal_lengths),
		];
		kinds.into_iter().filter(|&(_, on)| on).map(|(name, _)| name).collect()
	}

	#[test]
	fn each_option_alone_turns_on_its_own_kind_of_snap_and_no_other() {
		// Spacing and the grid are read by the session rather than told to the engine
		let expected = [
			("Anchors", Some("anchors")),
			("Midpoints", Some("midpoints")),
			("Box Corners", Some("box_corners")),
			("Box Centers", Some("box_centers")),
			("Box Edge Midpoints", Some("box_edge_midpoints")),
			("Artboards", Some("artboards")),
			("Along Paths", Some("curves")),
			("Along Box Edges", Some("box_edges")),
			("Tangent to Paths", Some("tangents")),
			("Perpendicular to Paths", Some("perpendiculars")),
			("With Anchors", Some("point_alignment")),
			("With Box Edges", Some("box_alignment")),
			("Even Spacing", None),
			("Equal Sizes", None),
			("Snap to Grid", None),
		];
		let options: Vec<_> = SNAPPING_OPTION_GROUPS.iter().flat_map(|(_, options)| options.iter().copied()).collect();
		assert_eq!(options.len(), expected.len());

		let mut all_off = SnappingState::default();
		for &(_, option, _) in &options {
			*option(&mut all_off) = false;
		}

		let always_on = kinds_on(&settings(&all_off));
		assert_eq!(always_on, vec!["extensions", "constructions", "equal_lengths"]);

		for (label, option, _) in options {
			let Some(&(_, expected_kind)) = expected.iter().find(|(expected_label, _)| *expected_label == label) else {
				panic!("the option {label} is missing from the expected kinds")
			};

			let mut state = all_off.clone();
			*option(&mut state) = true;
			let turned_on: Vec<_> = kinds_on(&settings(&state)).into_iter().filter(|kind| !always_on.contains(kind)).collect();
			assert_eq!(turned_on, Vec::from_iter(expected_kind), "the option {label} turned on the wrong kinds of snap");
		}
	}

	/// A document snapping to a rectangular grid, seen at a zoom level.
	fn document_with_grid(spacing: DVec2, subdivisions: u32, angle: f64, zoom: f64) -> DocumentMessageHandler {
		let mut document = DocumentMessageHandler::default();
		document.document_ptz.set_zoom(zoom);

		let state = &mut document.snapping_state;
		state.grid_snapping = true;
		state.grid.grid_type = GridType::Rectangular { spacing };
		state.grid.subdivisions = subdivisions;
		state.grid.angle = angle;

		document
	}

	fn spacings(grid: &Grid) -> Vec<f64> {
		grid.families().iter().map(|family| family.spacing).collect()
	}

	#[test]
	fn a_turned_rectangular_grid_has_its_lines_along_the_turned_directions() {
		let document = document_with_grid(DVec2::new(20., 30.), 1, 30., 1.);
		let grid = grid(&document).expect("the grid should be usable");
		let turned_x_axis = DVec2::from_angle(30_f64.to_radians());

		// The lines spaced along the turned X axis run along the turned Y axis, and the other way around
		let [spaced_along_x, spaced_along_y] = [grid.families()[0], grid.families()[1]];
		assert!(spaced_along_x.direction.abs_diff_eq(turned_x_axis.perp(), 1e-9) && (spaced_along_x.spacing - 20.).abs() < 1e-9);
		assert!(spaced_along_y.direction.abs_diff_eq(turned_x_axis, 1e-9) && (spaced_along_y.spacing - 30.).abs() < 1e-9);

		// Its points lie a whole number of spacings along each turned axis
		let point = grid.nearest_point(DVec2::new(123., -45.));
		for (axis, spacing) in [(turned_x_axis, 20.), (turned_x_axis.perp(), 30.)] {
			let steps = point.dot(axis) / spacing;
			assert!((steps - steps.round()).abs() < 1e-9, "{point:?} is off a line by {steps}");
		}
	}

	#[test]
	fn the_grid_snaps_to_the_minor_lines_while_they_are_drawn_and_to_the_major_ones_once_they_would_be_too_fine() {
		// Four subdivisions of major lines 40 apart put minor lines 10 apart, which is far enough apart on screen at full zoom
		let document = document_with_grid(DVec2::splat(40.), 4, 0., 1.);
		assert_eq!(spacings(&grid(&document).expect("the grid should be usable")), vec![10., 10.]);
		assert_eq!(spacings(&major_grid(&document).expect("the grid should be usable")), vec![40., 40.]);

		// At a third of the zoom the minor lines would be under 4 pixels apart, so only the major lines are drawn
		let document = document_with_grid(DVec2::splat(40.), 4, 0., 1. / 3.);
		assert_eq!(spacings(&grid(&document).expect("the grid should be usable")), vec![40., 40.]);

		// Further out, the major lines themselves are drawn coarser
		let document = document_with_grid(DVec2::splat(40.), 4, 0., 0.2);
		assert_eq!(spacings(&grid(&document).expect("the grid should be usable")), vec![80., 80.]);
	}

	#[test]
	fn options_that_make_no_usable_grid_give_none_to_draw_or_snap_to() {
		let unusable = [
			(DVec2::new(0., 10.), 1, 0.),
			(DVec2::new(-10., 10.), 1, 0.),
			(DVec2::new(f64::NAN, 10.), 1, 0.),
			(DVec2::new(f64::INFINITY, 10.), 1, 0.),
			(DVec2::splat(10.), 0, 0.),
			(DVec2::splat(10.), 1, f64::NAN),
			(DVec2::splat(10.), 1, f64::INFINITY),
		];

		for (spacing, subdivisions, angle) in unusable {
			let document = document_with_grid(spacing, subdivisions, angle, 1.);
			assert!(document.snapping_state.grid.drawn_levels(&document.document_ptz).is_none());
			assert!(grid(&document).is_none() && major_grid(&document).is_none());
		}
	}
}
