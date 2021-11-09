use glam::DVec2;
use graphene::LayerId;

use crate::consts::SNAP_TOLERANCE;
use crate::document::DocumentMessageHandler;

/// Gets the snap targets on a layer (currently top left, centre, and bottom right) in document space.
fn get_layer_snap_targets(layer: &[LayerId], document: &DocumentMessageHandler) -> Option<[DVec2; 3]> {
	document.graphene_document.layer(layer).unwrap().current_bounding_box().map(|[bound1, bound2]| {
		let center = (bound1 + bound2) / 2.;
		[bound1, bound2, center]
	})
}

/// Gets a list of snap targets for the X and Y axes in Document coords for the non-selected layers.
/// This should be cached at the start of a drag.
pub fn get_snap_targets(document: &DocumentMessageHandler, selected_layers: &[Vec<LayerId>], ignore_layers: &[Vec<LayerId>]) -> [Vec<f64>; 2] {
	let mut snap_targets_x = Vec::new();
	let mut snap_targets_y = Vec::new();
	for path in document.all_layers_sorted() {
		if !selected_layers.contains(&path) && !ignore_layers.contains(&path) {
			if let Some(snap_targets) = get_layer_snap_targets(&path, document) {
				snap_targets_x.extend(snap_targets.map(|v| v.x));
				snap_targets_y.extend(snap_targets.map(|v| v.y));
			}
		}
	}
	[snap_targets_x, snap_targets_y]
}

/// Finds the closest snap from an array of layers to the specified snap targets in viewport coords.
/// Returns 0 for each axis that there is no snap less than the snap tolerance.
pub fn snap_layers(snap_targets: &[Vec<f64>; 2], document: &DocumentMessageHandler, selected_layers: &[Vec<LayerId>], mouse_delta: DVec2) -> DVec2 {
	// Convert mouse delta to document coords
	let mouse_delta_document = document.graphene_document.root.transform.transform_vector2(mouse_delta);

	let mut closest_move = DVec2::splat(f64::MAX);
	// For each layer in the selected layers, find the minimum distance required to snap it to a snap target.
	// If this is less than the current closest move, then update the closest move on that axis.
	for path in selected_layers {
		if let Some(layer_snap_targets) = get_layer_snap_targets(path, document) {
			for target_x in layer_snap_targets.map(|v| v.x) {
				if let Some(min) = snap_targets[0]
					.iter()
					.map(|x| x - mouse_delta_document.x)
					.map(|x| (x - target_x))
					.reduce(|x, y| if x.abs() > y.abs() { y } else { x })
				{
					if min.abs() < closest_move.x.abs() {
						closest_move.x = min;
					}
				}
			}
			for target_y in layer_snap_targets.map(|v| v.y) {
				if let Some(min) = snap_targets[1]
					.iter()
					.map(|y| y - mouse_delta_document.y)
					.map(|y| (y - target_y))
					.reduce(|x, y| if x.abs() > y.abs() { y } else { x })
				{
					if min.abs() < closest_move.y.abs() {
						closest_move.y = min;
					}
				}
			}
		}
	}

	// Convert to viewport coords
	closest_move = document.graphene_document.root.transform.inverse().transform_vector2(closest_move);

	// Do not move if over snap tolerence
	if closest_move.x.abs() > SNAP_TOLERANCE {
		closest_move.x = 0.;
	}

	if closest_move.y.abs() > SNAP_TOLERANCE {
		closest_move.y = 0.;
	}
	closest_move
}

/// Handles snapping of a viewport position, returning another viewport position.
pub fn snap_position(snap: &[Vec<f64>; 2], document: &DocumentMessageHandler, position_viewport: DVec2) -> DVec2 {
	// Convert to document coordinates
	let position_document = document.graphene_document.root.transform.inverse().transform_point2(position_viewport);

	// For each list of snap targets, find the shortest distance to move the point to that target.
	let mut closest_move = DVec2::ZERO;
	if let Some(min) = snap[0].iter().map(|x| (x - position_document.x)).reduce(|x, y| if x.abs() > y.abs() { y } else { x }) {
		closest_move.x = min;
	}
	if let Some(min) = snap[1].iter().map(|y| (y - position_document.y)).reduce(|x, y| if x.abs() > y.abs() { y } else { x }) {
		closest_move.y = min;
	}
	// Convert the resulting movement to viewport coords
	closest_move = document.graphene_document.root.transform.inverse().transform_vector2(closest_move);

	// Do not move if over snap tolerence
	if closest_move.x.abs() > SNAP_TOLERANCE {
		closest_move.x = 0.;
	}

	if closest_move.y.abs() > SNAP_TOLERANCE {
		closest_move.y = 0.;
	}

	position_viewport + closest_move
}
