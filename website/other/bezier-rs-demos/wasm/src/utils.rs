use bezier_rs::{Cap, Joint};

pub fn parse_joint(joint: i32) -> Joint {
	match joint {
		0 => Joint::Bevel,
		1 => Joint::Miter,
		2 => Joint::Round,
		_ => panic!("Unexpected Joint value: '{}'", joint),
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
