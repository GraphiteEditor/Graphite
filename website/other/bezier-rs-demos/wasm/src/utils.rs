use bezier_rs::{Cap, Join};
use glam::DVec2;
use js_sys::Array;
use wasm_bindgen::{JsCast, JsValue};

pub fn parse_join(join: i32, miter_limit: f64) -> Join {
	match join {
		0 => Join::Bevel,
		1 => Join::Miter(Some(miter_limit)),
		2 => Join::Round,
		_ => panic!("Unexpected Join value: '{join}'"),
	}
}

pub fn parse_cap(cap: i32) -> Cap {
	match cap {
		0 => Cap::Butt,
		1 => Cap::Round,
		2 => Cap::Square,
		_ => panic!("Unexpected Cap value: '{cap}'"),
	}
}

pub fn parse_point(js_point: &JsValue) -> DVec2 {
	let point = js_point.to_owned().dyn_into::<Array>().unwrap();
	let x = point.get(0).as_f64().unwrap();
	let y = point.get(1).as_f64().unwrap();
	DVec2::new(x, y)
}
