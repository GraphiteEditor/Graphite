use crate::messages::portfolio::document::utility_types::misc::{GridSnapping, GridType, SnappingState};
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
		// Sliding along the edges of a layer's bounding box has no option to turn it on
		box_edges: false,
		artboards: state.artboards,
		tangents: state.path.tangent_to_path,
		perpendiculars: state.path.normal_to_path,
		alignment: state.bounding_box.align_with_edges || state.path.align_with_anchor_point,
		// These come only from targets that have been woken, and have no options of their own
		extensions: true,
		constructions: true,
		equal_lengths: true,
	}
}

/// The grid to land on, which is the one drawn at the document's present zoom. Returns `None` while grid snapping is off or the grid's options make no usable grid.
pub fn grid(document: &DocumentMessageHandler) -> Option<Grid> {
	let state = &document.snapping_state;
	if !state.grid_snapping {
		return None;
	}

	match state.grid.grid_type {
		GridType::Rectangular { spacing } => Grid::rectangular(state.grid.origin, GridSnapping::compute_rectangle_spacing(spacing, &document.document_ptz)?),
		GridType::Isometric { y_axis_spacing, angle_a, angle_b } => {
			let slopes = [-angle_a.to_radians().tan(), angle_b.to_radians().tan()];
			let spread = slopes[1] - slopes[0];
			let multiplier = GridSnapping::compute_isometric_multiplier(y_axis_spacing, spread, &document.document_ptz)?;

			// The slanted lines are spaced along Y, which is further apart than they are straight across
			let along_y = y_axis_spacing.abs() * multiplier;
			let slanted = slopes.map(|slope| (DVec2::new(1., slope), along_y / slope.hypot(1.)));
			Grid::new(state.grid.origin, [(DVec2::Y, along_y / spread), slanted[0], slanted[1]])
		}
	}
}
