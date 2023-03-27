use bezier_rs::{Cap, Join};

pub fn parse_join(join: i32) -> Join {
	match join {
		0 => Join::Bevel,
		1 => Join::Miter,
		2 => Join::Round,
		_ => panic!("Unexpected Join value: '{}'", join),
	}
}

pub fn parse_cap(cap: i32) -> Cap {
	match cap {
		0 => Cap::Butt,
		1 => Cap::Round,
		2 => Cap::Square,
		_ => panic!("Unexpected Cap value: '{}'", cap),
	}
}
