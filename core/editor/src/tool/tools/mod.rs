use document_core::{LayerId, Operation};
use glam::{DAffine2, DVec2};

use crate::{input::mouse::ViewportPosition, message_prelude::Message};

// already implemented
pub mod ellipse;
pub mod fill;
pub mod line;
pub mod pen;
pub mod rectangle;
pub mod shape;

// not implemented yet
pub mod crop;
pub mod eyedropper;
pub mod navigate;
pub mod path;
pub mod select;

fn make_transform(id: LayerId, square: bool, center: bool, drag_start: ViewportPosition, drag_current: ViewportPosition, transform: DAffine2) -> Message {
	let x0 = drag_start.x as f64;
	let y0 = drag_start.y as f64;
	let x1 = drag_current.x as f64;
	let y1 = drag_current.y as f64;

	let (x0, y0, x1, y1) = if square {
		let (x_dir, y_dir) = ((x1 - x0).signum(), (y1 - y0).signum());
		let max_dist = f64::max((x1 - x0).abs(), (y1 - y0).abs());
		if center {
			(x0 - max_dist * x_dir, y0 - max_dist * y_dir, x0 + max_dist * x_dir, y0 + max_dist * y_dir)
		} else {
			(x0, y0, x0 + max_dist * x_dir, y0 + max_dist * y_dir)
		}
	} else {
		let (x0, y0) = if center {
			let delta_x = x1 - x0;
			let delta_y = y1 - y0;

			(x0 - delta_x, y0 - delta_y)
		} else {
			(x0, y0)
		};
		(x0, y0, x1, y1)
	};

	Operation::SetLayerTransform {
		path: vec![id],
		transform: (transform.inverse() * glam::DAffine2::from_scale_angle_translation(DVec2::new(x1 - x0, y1 - y0), 0., DVec2::new(x0, y0))).to_cols_array(),
	}
	.into()
}
