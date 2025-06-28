//! requires bezier-rs

use crate::curve::{Curve, CurveManipulatorGroup, ValueMapperNode};
use bezier_rs::{Bezier, TValue};
use graphene_core::color::{Channel, Linear};
use graphene_core::context::Ctx;

const WINDOW_SIZE: usize = 1024;

#[node_macro::node(category(""))]
fn generate_curves<C: Channel + Linear>(_: impl Ctx, curve: Curve, #[implementations(f32, f64)] _target_format: C) -> ValueMapperNode<C> {
	let [mut pos, mut param]: [[f32; 2]; 2] = [[0.; 2], curve.first_handle];
	let mut lut = vec![C::from_f64(0.); WINDOW_SIZE];
	let end = CurveManipulatorGroup {
		anchor: [1.; 2],
		handles: [curve.last_handle, [0.; 2]],
	};
	for sample in curve.manipulator_groups.iter().chain(std::iter::once(&end)) {
		let [x0, y0, x1, y1, x2, y2, x3, y3] = [pos[0], pos[1], param[0], param[1], sample.handles[0][0], sample.handles[0][1], sample.anchor[0], sample.anchor[1]].map(f64::from);

		let bezier = Bezier::from_cubic_coordinates(x0, y0, x1, y1, x2, y2, x3, y3);

		let [left, right] = [pos[0], sample.anchor[0]].map(|c| c.clamp(0., 1.));
		let lut_index_left: usize = (left * (lut.len() - 1) as f32).floor() as _;
		let lut_index_right: usize = (right * (lut.len() - 1) as f32).ceil() as _;
		for index in lut_index_left..=lut_index_right {
			let x = index as f64 / (lut.len() - 1) as f64;
			let y = if x <= x0 {
				y0
			} else if x >= x3 {
				y3
			} else {
				bezier.find_tvalues_for_x(x)
					.next()
					.map(|t| bezier.evaluate(TValue::Parametric(t.clamp(0., 1.))).y)
					// Fall back to a very bad approximation if Bezier-rs fails
					.unwrap_or_else(|| (x - x0) / (x3 - x0) * (y3 - y0) + y0)
			};
			lut[index] = C::from_f64(y);
		}

		pos = sample.anchor;
		param = sample.handles[1];
	}
	ValueMapperNode::new(lut)
}
