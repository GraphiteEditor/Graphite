use glam::DVec2;
use graphene::document::Document;
use graphene::LayerId;

use crate::consts::SNAP_TOLERANCE;

#[derive(Debug, Clone)]
pub struct SnapHandler {
	snap_targets: Option<[Vec<f64>; 2]>,
	snapping_enabled: bool,
}
impl Default for SnapHandler {
	fn default() -> Self {
		Self {
			snap_targets: None,
			snapping_enabled: true,
		}
	}
}

impl SnapHandler {
	/// Gets a list of snap targets for the X and Y axes in Viewport coords for the target layers (usually all layers or all non-selected layers.)
	/// This should be cached at the start of a drag.
	pub fn start_snap(&mut self, document: &Document, target_layers: Vec<Vec<LayerId>>, ignore_layers: &[Vec<LayerId>]) {
		if self.snapping_enabled {
			let targets = target_layers
				.iter()
				.filter(|path| !ignore_layers.contains(path))
				.filter_map(|path| document.viewport_bounding_box(path).ok()?)
				.flat_map(|[bound1, bound2]| [bound1, bound2, ((bound1 + bound2) / 2.)])
				.map(|vec| vec.into())
				.unzip();

			self.snap_targets = Some([targets.0, targets.1]);
		}
	}

	/// Finds the closest snap from an array of layers to the specified snap targets in viewport coords.
	/// Returns 0 for each axis that there is no snap less than the snap tolerance.
	pub fn snap_layers(&self, document: &Document, selected_layers: &[Vec<LayerId>], mouse_delta: DVec2) -> DVec2 {
		if let Some([targets_x, targets_y]) = &self.snap_targets {
			let (snap_x, snap_y): (Vec<f64>, Vec<f64>) = selected_layers
				.iter()
				.filter_map(|path| document.viewport_bounding_box(path).ok()?)
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

			// Do not move if over snap tolerence
			let clamped_closest_move = DVec2::new(
				if closest_move.x.abs() > SNAP_TOLERANCE { 0. } else { closest_move.x },
				if closest_move.y.abs() > SNAP_TOLERANCE { 0. } else { closest_move.y },
			);

			clamped_closest_move
		} else {
			DVec2::ZERO
		}
	}

	/// Handles snapping of a viewport position, returning another viewport position.
	pub fn snap_position(&self, position_viewport: DVec2) -> DVec2 {
		if let Some([targets_x, targets_y]) = &self.snap_targets {
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

			// Do not move if over snap tolerence
			let clamped_closest_move = DVec2::new(
				if closest_move.x.abs() > SNAP_TOLERANCE { 0. } else { closest_move.x },
				if closest_move.y.abs() > SNAP_TOLERANCE { 0. } else { closest_move.y },
			);

			position_viewport + clamped_closest_move
		} else {
			position_viewport
		}
	}
	pub fn set_snapping_enabled(&mut self, new_snapping_enabled: bool) {
		self.snapping_enabled = new_snapping_enabled;
		if !new_snapping_enabled {
			self.snap_targets = None;
		}
	}
}
