use std::f64::consts::PI;

use glam::{DAffine2, DVec2};
use graphene::{
	layers::style::{self, Stroke},
	LayerId, Operation,
};

use crate::{
	consts::{COLOR_ACCENT, SNAP_OVERLAY_UNSNAPPED_OPACITY, SNAP_OVERLAY_VISIBLE, SNAP_TOLERANCE},
	document::DocumentMessageHandler,
	message_prelude::*,
};

#[derive(Debug, Clone, Default)]
pub struct SnapHandler {
	snap_targets: Option<(Vec<f64>, Vec<f64>)>,
	overlay_paths: Vec<Vec<LayerId>>,
}

impl SnapHandler {
	/// Updates the snapping overlays with the specified distances (iterator represents (pos, distance))
	fn update_overlays(
		overlay_paths: &mut Vec<Vec<LayerId>>,
		responses: &mut VecDeque<Message>,
		viewport_bounds: DVec2,
		positions_distances: (impl Iterator<Item = (f64, f64)>, impl Iterator<Item = (f64, f64)>),
		closest_distance: DVec2,
	) {
		fn add_overlay_line(responses: &mut VecDeque<Message>, transform: [f64; 6], opacity: f64, index: usize, overlay_paths: &mut Vec<Vec<LayerId>>) {
			let layer_path = if index >= overlay_paths.len() {
				let layer_path = vec![generate_uuid()];
				responses.push_back(
					DocumentMessage::Overlays(
						Operation::AddOverlayLine {
							path: layer_path.clone(),
							transform,
							style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), None),
						}
						.into(),
					)
					.into(),
				);
				overlay_paths.push(layer_path.clone());
				layer_path
			} else {
				let layer_path = overlay_paths[index].clone();
				responses.push_back(DocumentMessage::Overlays(Operation::SetLayerTransform { path: layer_path.clone(), transform }.into()).into());
				layer_path
			};

			responses.push_back(DocumentMessage::Overlays(Operation::SetLayerOpacity { path: layer_path, opacity }.into()).into());
		}

		let mut index = 0;
		for (x_target, distance) in positions_distances.0.filter(|(_pos, dist)| dist.abs() < SNAP_OVERLAY_VISIBLE) {
			add_overlay_line(
				responses,
				DAffine2::from_scale_angle_translation(DVec2::new(viewport_bounds.y, 1.), PI / 2., DVec2::new((x_target + 0.5).round() - 0.5, 0.)).to_cols_array(),
				if closest_distance.x == distance {
					1.
				} else {
					SNAP_OVERLAY_UNSNAPPED_OPACITY - distance.abs() / (SNAP_OVERLAY_VISIBLE / SNAP_OVERLAY_UNSNAPPED_OPACITY)
				},
				index,
				overlay_paths,
			);
			index += 1;
		}
		for (y_target, distance) in positions_distances.1.filter(|(_pos, dist)| dist.abs() < SNAP_OVERLAY_VISIBLE) {
			add_overlay_line(
				responses,
				DAffine2::from_scale_angle_translation(DVec2::new(viewport_bounds.x, 1.), 0., DVec2::new(0., (y_target + 0.5).round() - 0.5)).to_cols_array(),
				if closest_distance.y == distance {
					1.
				} else {
					SNAP_OVERLAY_UNSNAPPED_OPACITY - distance.abs() / (SNAP_OVERLAY_VISIBLE / SNAP_OVERLAY_UNSNAPPED_OPACITY)
				},
				index,
				overlay_paths,
			);
			index += 1;
		}
		Self::remove_overlays_after(overlay_paths, responses, index);
	}

	/// Remove all overlays after index.
	fn remove_overlays_after(overlay_paths: &mut Vec<Vec<LayerId>>, responses: &mut VecDeque<Message>, index: usize) {
		while overlay_paths.len() > index {
			responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: overlay_paths.pop().unwrap() }.into()).into());
		}
	}

	/// Gets a list of snap targets for the X and Y axes in Viewport coords for the target layers (usually all layers or all non-selected layers.)
	/// This should be called at the start of a drag.
	pub fn start_snap(&mut self, document_message_handler: &DocumentMessageHandler, target_layers: Vec<&[LayerId]>) {
		if document_message_handler.snapping_enabled {
			// Could be made into sorted Vec or a HashSet for more performant lookups.
			self.snap_targets = Some(
				target_layers
					.iter()
					.filter_map(|path| document_message_handler.graphene_document.viewport_bounding_box(path).ok()?)
					.flat_map(|[bound1, bound2]| [bound1, bound2, ((bound1 + bound2) / 2.)])
					.map(|vec| vec.into())
					.unzip(),
			);
		}
	}

	/// Finds the closest snap from an array of layers to the specified snap targets in viewport coords.
	/// Returns 0 for each axis that there is no snap less than the snap tolerance.
	pub fn snap_layers(
		&mut self,
		responses: &mut VecDeque<Message>,
		viewport_bounds: DVec2,
		document_message_handler: &DocumentMessageHandler,

		selected_layers: &[Vec<LayerId>],
		mouse_delta: DVec2,
	) -> DVec2 {
		if document_message_handler.snapping_enabled {
			if let Some((targets_x, targets_y)) = &self.snap_targets {
				let (snap_x, snap_y): (Vec<f64>, Vec<f64>) = selected_layers
					.iter()
					.filter_map(|path| document_message_handler.graphene_document.viewport_bounding_box(path).ok()?)
					.flat_map(|[bound1, bound2]| [bound1, bound2, (bound1 + bound2) / 2.])
					.map(|vec| vec.into())
					.unzip();

				let positions_distances = (
					targets_x.iter().flat_map(|&target| snap_x.iter().map(move |&snap| (target, target - mouse_delta.x - snap))),
					targets_y.iter().flat_map(|&target| snap_y.iter().map(move |&snap| (target, target - mouse_delta.y - snap))),
				);

				let min = (
					positions_distances.0.clone().min_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).expect("Could not compare position.")),
					positions_distances.1.clone().min_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).expect("Could not compare position.")),
				);

				let closest_distance = DVec2::new(min.0.map_or(0., |(_pos, dist)| dist), min.1.map_or(0., |(_pos, dist)| dist));

				// Clamp, do not move if over snap tolerance
				let clamped_closest_distance = DVec2::new(
					if closest_distance.x.abs() > SNAP_TOLERANCE { 0. } else { closest_distance.x },
					if closest_distance.y.abs() > SNAP_TOLERANCE { 0. } else { closest_distance.y },
				);

				Self::update_overlays(&mut self.overlay_paths, responses, viewport_bounds, positions_distances, clamped_closest_distance);

				clamped_closest_distance
			} else {
				DVec2::ZERO
			}
		} else {
			DVec2::ZERO
		}
	}

	/// Handles snapping of a viewport position, returning another viewport position.
	pub fn snap_position(&mut self, responses: &mut VecDeque<Message>, viewport_bounds: DVec2, document_message_handler: &DocumentMessageHandler, position_viewport: DVec2) -> DVec2 {
		if document_message_handler.snapping_enabled {
			if let Some((targets_x, targets_y)) = &self.snap_targets {
				let positions_distances = (targets_x.iter().map(|&x| (x, x - position_viewport.x)), targets_y.iter().map(|&y| (y, y - position_viewport.y)));

				let min = (
					positions_distances.0.clone().min_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).expect("Could not compare position.")),
					positions_distances.1.clone().min_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).expect("Could not compare position.")),
				);

				let closest_distance = DVec2::new(min.0.map_or(0., |(_pos, dist)| dist), min.1.map_or(0., |(_pos, dist)| dist));

				// Do not move if over snap tolerance
				let clamped_closest_distance = DVec2::new(
					if closest_distance.x.abs() > SNAP_TOLERANCE { 0. } else { closest_distance.x },
					if closest_distance.y.abs() > SNAP_TOLERANCE { 0. } else { closest_distance.y },
				);

				Self::update_overlays(&mut self.overlay_paths, responses, viewport_bounds, positions_distances, clamped_closest_distance);

				position_viewport + clamped_closest_distance
			} else {
				position_viewport
			}
		} else {
			position_viewport
		}
	}

	/// Removes snap target data & overlays. Call this when snapping is done.
	pub fn cleanup(&mut self, responses: &mut VecDeque<Message>) {
		Self::remove_overlays_after(&mut self.overlay_paths, responses, 0);
		self.snap_targets = None;
	}
}
