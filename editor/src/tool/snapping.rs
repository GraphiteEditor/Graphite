use std::{collections::HashMap, f64::consts::PI};

use super::DocumentMessageHandler;
use glam::{DAffine2, DVec2};
use graphene::{
	layers::style::{self, Stroke},
	LayerId, Operation,
};

use crate::{
	consts::{COLOR_ACCENT, SNAP_TOLERANCE},
	message_prelude::*,
};

#[derive(Debug, Clone, Default)]
pub struct SnapHandler {
	enabled: bool,
	document: DocumentMessageHandler,
	snap_targets: Option<(Vec<f64>, Vec<f64>)>,
	snappables: HashMap<Vec<LayerId>, Vec<Vec<LayerId>>>,
	overlay_messages: Vec<Message>,

	overlay_paths: Vec<Vec<LayerId>>,
}

impl SnapHandler {
	fn add_overlay_line(&mut self, transform: DAffine2) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];

		let operation = Operation::AddOverlayLine {
			path: layer_path.clone(),
			transform: transform.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), None),
		};
		self.overlay_messages.push(DocumentMessage::Overlay(operation.into()).into());

		layer_path
	}

	fn add_overlays(&mut self, viewport_bounds: DVec2) {
		if let Some((x_targets, y_targets)) = &self.snap_targets {
			for x_target in x_targets {
				self.overlay_paths
					.push(self.add_overlay_line(DAffine2::from_scale_angle_translation(DVec2::new(viewport_bounds.y, 1.), PI / 2., DVec2::new(x_target.round(), 0.))));
			}
			for y_target in y_targets {
				self.overlay_paths
					.push(self.add_overlay_line(DAffine2::from_scale_angle_translation(DVec2::new(viewport_bounds.x, 1.), 0., DVec2::new(0., y_target.round()))));
			}
		}
	}

	fn remove_overlays(&mut self, responses: &mut VecDeque<Message>) {
		while let Some(layer) = self.overlay_paths.pop() {
			responses.push_back(DocumentMessage::Overlay(Operation::DeleteLayer { path: layer }.into()).into());
		}
	}

	/// Gets a list of snap targets for the X and Y axes in Viewport coords for the target layers (usually all layers or all non-selected layers.)
	/// This should be called at the start of a drag.
	pub fn start_snap(&mut self, viewport_bounds: DVec2, target_layers: Vec<Vec<LayerId>>) {
		if self.document.snapping_enabled {
			// Could be made into sorted Vec or a HashSet for more performant lookups.
			self.snap_targets = Some(
				target_layers
					.iter()
					.filter_map(|path| self.document.graphene_document.viewport_bounding_box(path).ok()?)
					.flat_map(|[bound1, bound2]| [bound1, bound2, ((bound1 + bound2) / 2.)])
					.map(|vec| vec.into())
					.unzip(),
			);
			self.add_overlays(viewport_bounds);
		}
	}

	/// Finds the closest snap from an array of layers to the specified snap targets in viewport coords.
	/// Returns 0 for each axis that there is no snap less than the snap tolerance.
	pub fn snap_layers(&self, mouse_delta: DVec2) -> DVec2 {
		if self.enabled {
			if let Some((targets_x, targets_y)) = &self.snap_targets {
				let (snap_x, snap_y): (Vec<f64>, Vec<f64>) = self
					.snappables
					.iter()
					.filter_map(|(path, _)| self.document.graphene_document.viewport_bounding_box(path).ok()?)
					.flat_map(|[bound1, bound2]| [bound1, bound2, (bound1 + bound2) / 2.])
					.map(|vec| vec.into())
					.unzip();

				let closest_move = DVec2::new(
					targets_x
						.iter()
						.flat_map(|target| snap_x.iter().map(move |snap| target - mouse_delta.x - snap))
						.min_by(|a, b| a.abs().partial_cmp(&b.abs()).expect("Could not compare document bounds."))
						.unwrap_or(0.),
					targets_y
						.iter()
						.flat_map(|target| snap_y.iter().map(move |snap| target - mouse_delta.y - snap))
						.min_by(|a, b| a.abs().partial_cmp(&b.abs()).expect("Could not compare document bounds."))
						.unwrap_or(0.),
				);

				// Clamp, do not move if over snap tolerance
				DVec2::new(
					if closest_move.x.abs() > SNAP_TOLERANCE { 0. } else { closest_move.x },
					if closest_move.y.abs() > SNAP_TOLERANCE { 0. } else { closest_move.y },
				)
			} else {
				DVec2::ZERO
			}
		} else {
			DVec2::ZERO
		}
	}

	/// Handles snapping of a viewport position, returning another viewport position.
	pub fn snap_position(&self, position_viewport: DVec2) -> DVec2 {
		if self.document.snapping_enabled {
			if let Some((targets_x, targets_y)) = &self.snap_targets {
				// For each list of snap targets, find the shortest distance to move the point to that target.
				let closest_move = DVec2::new(
					targets_x
						.iter()
						.map(|x| (x - position_viewport.x))
						.min_by(|a, b| a.abs().partial_cmp(&b.abs()).expect("Could not compare document bounds."))
						.unwrap_or(0.),
					targets_y
						.iter()
						.map(|y| (y - position_viewport.y))
						.min_by(|a, b| a.abs().partial_cmp(&b.abs()).expect("Could not compare document bounds."))
						.unwrap_or(0.),
				);

				// Do not move if over snap tolerance
				let clamped_closest_move = DVec2::new(
					if closest_move.x.abs() > SNAP_TOLERANCE { 0. } else { closest_move.x },
					if closest_move.y.abs() > SNAP_TOLERANCE { 0. } else { closest_move.y },
				);

				position_viewport + clamped_closest_move
			} else {
				position_viewport
			}
		} else {
			position_viewport
		}
	}

	pub fn cleanup(&mut self, responses: &mut VecDeque<Message>) {
		self.remove_overlays(responses);
		self.snap_targets = None;
	}

	// ---
	pub fn add_snappable(&mut self, layer: Vec<LayerId>) {
		self.snappables.insert(layer, vec![]);
	}

	pub fn remove_snappable(&mut self, layer: Vec<LayerId>) {
		self.snappables.remove(&layer);
	}

	pub fn process_overlays(&mut self, responses: &mut VecDeque<Message>) {
		responses.extend(self.overlay_messages.into_iter());
		self.overlay_messages.clear();
	}
}
