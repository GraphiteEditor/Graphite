use bezier_rs::Joint;

pub fn parse_joint(joint: i32) -> Joint {
	match joint {
		0 => Joint::Bevel,
		1 => Joint::Miter,
		2 => Joint::Round,
		_ => panic!("Unexpected Joint string: '{}'", joint),
	}
}
