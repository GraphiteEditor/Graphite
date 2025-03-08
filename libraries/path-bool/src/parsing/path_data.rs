use crate::BooleanError;
use crate::path::{Path, path_from_commands, path_to_commands};
use crate::path_command::{AbsolutePathCommand, PathCommand, RelativePathCommand};
use glam::DVec2;
use regex::Regex;

pub fn commands_from_path_data(d: &str) -> Result<Vec<PathCommand>, BooleanError> {
	let re_float = Regex::new(r"^\s*,?\s*(-?\d*(?:\d\.|\.\d|\d)\d*(?:[eE][+\-]?\d+)?)").unwrap();
	let re_cmd = Regex::new(r"^\s*([MLCSQTAZHVmlhvcsqtaz])").unwrap();
	let re_bool = Regex::new(r"^\s*,?\s*([01])").unwrap();

	let mut i = 0;
	let mut last_cmd = 'M';
	let mut commands = Vec::new();

	let get_cmd = |i: &mut usize, last_cmd: char| -> Option<char> {
		if *i >= d.len() - 1.min(d.len()) {
			return None;
		}

		if let Some(cap) = re_cmd.captures(&d[*i..]) {
			*i += cap[0].len();
			Some(cap[1].chars().next().unwrap())
		} else {
			match last_cmd {
				'M' => Some('L'),
				'm' => Some('l'),
				'z' | 'Z' => None,
				_ => Some(last_cmd),
			}
		}
	};

	let get_float = |i: &mut usize| -> f64 {
		if let Some(cap) = re_float.captures(&d[*i..]) {
			*i += cap[0].len();
			cap[1].parse().unwrap()
		} else {
			panic!("Invalid path data. Expected a number at index {}, got {}", i, &d[*i..]);
		}
	};

	let get_bool = |i: &mut usize| -> bool {
		if let Some(cap) = re_bool.captures(&d[*i..]) {
			*i += cap[0].len();
			&cap[1] == "1"
		} else {
			panic!("Invalid path data. Expected a flag at index {}", i);
		}
	};

	while let Some(cmd) = get_cmd(&mut i, last_cmd) {
		last_cmd = cmd;
		match cmd {
			'M' => commands.push(PathCommand::Absolute(AbsolutePathCommand::M(DVec2::new(get_float(&mut i), get_float(&mut i))))),
			'L' => commands.push(PathCommand::Absolute(AbsolutePathCommand::L(DVec2::new(get_float(&mut i), get_float(&mut i))))),
			'C' => commands.push(PathCommand::Absolute(AbsolutePathCommand::C(
				DVec2::new(get_float(&mut i), get_float(&mut i)),
				DVec2::new(get_float(&mut i), get_float(&mut i)),
				DVec2::new(get_float(&mut i), get_float(&mut i)),
			))),
			'S' => commands.push(PathCommand::Absolute(AbsolutePathCommand::S(
				DVec2::new(get_float(&mut i), get_float(&mut i)),
				DVec2::new(get_float(&mut i), get_float(&mut i)),
			))),
			'Q' => commands.push(PathCommand::Absolute(AbsolutePathCommand::Q(
				DVec2::new(get_float(&mut i), get_float(&mut i)),
				DVec2::new(get_float(&mut i), get_float(&mut i)),
			))),
			'T' => commands.push(PathCommand::Absolute(AbsolutePathCommand::T(DVec2::new(get_float(&mut i), get_float(&mut i))))),
			'A' => commands.push(PathCommand::Absolute(AbsolutePathCommand::A(
				get_float(&mut i),
				get_float(&mut i),
				get_float(&mut i),
				get_bool(&mut i),
				get_bool(&mut i),
				DVec2::new(get_float(&mut i), get_float(&mut i)),
			))),
			'Z' | 'z' => commands.push(PathCommand::Absolute(AbsolutePathCommand::Z)),
			'H' => commands.push(PathCommand::Absolute(AbsolutePathCommand::H(get_float(&mut i)))),
			'V' => commands.push(PathCommand::Absolute(AbsolutePathCommand::V(get_float(&mut i)))),
			'm' => commands.push(PathCommand::Relative(RelativePathCommand::M(get_float(&mut i), get_float(&mut i)))),
			'l' => commands.push(PathCommand::Relative(RelativePathCommand::L(get_float(&mut i), get_float(&mut i)))),
			'h' => commands.push(PathCommand::Relative(RelativePathCommand::H(get_float(&mut i)))),
			'v' => commands.push(PathCommand::Relative(RelativePathCommand::V(get_float(&mut i)))),
			'c' => commands.push(PathCommand::Relative(RelativePathCommand::C(
				get_float(&mut i),
				get_float(&mut i),
				get_float(&mut i),
				get_float(&mut i),
				get_float(&mut i),
				get_float(&mut i),
			))),
			's' => commands.push(PathCommand::Relative(RelativePathCommand::S(
				get_float(&mut i),
				get_float(&mut i),
				get_float(&mut i),
				get_float(&mut i),
			))),
			'q' => commands.push(PathCommand::Relative(RelativePathCommand::Q(
				get_float(&mut i),
				get_float(&mut i),
				get_float(&mut i),
				get_float(&mut i),
			))),
			't' => commands.push(PathCommand::Relative(RelativePathCommand::T(get_float(&mut i), get_float(&mut i)))),
			'a' => commands.push(PathCommand::Relative(RelativePathCommand::A(
				get_float(&mut i),
				get_float(&mut i),
				get_float(&mut i),
				get_bool(&mut i),
				get_bool(&mut i),
				get_float(&mut i),
				get_float(&mut i),
			))),
			_ => return Err(BooleanError::InvalidPathCommand(cmd)),
		}
	}

	Ok(commands)
}

pub fn path_from_path_data(d: &str) -> Result<Path, BooleanError> {
	Ok(path_from_commands(commands_from_path_data(d)?).collect())
}

pub fn path_to_path_data(path: &Path, eps: f64) -> String {
	path_to_commands(path.iter(), eps)
		.map(|cmd| match cmd {
			PathCommand::Absolute(abs_cmd) => match abs_cmd {
				AbsolutePathCommand::H(dx) => format!("H {:.12}", dx),
				AbsolutePathCommand::V(dy) => format!("V {:.12}", dy),
				AbsolutePathCommand::M(p) => format!("M {:.12},{:.12}", p.x, p.y),
				AbsolutePathCommand::L(p) => format!("L {:.12},{:.12}", p.x, p.y),
				AbsolutePathCommand::C(p1, p2, p3) => format!("C {:.12},{:.12} {:.12},{:.12} {:.12},{:.12}", p1.x, p1.y, p2.x, p2.y, p3.x, p3.y),
				AbsolutePathCommand::S(p1, p2) => {
					format!("S {:.12},{:.12} {:.12},{:.12}", p1.x, p1.y, p2.x, p2.y)
				}
				AbsolutePathCommand::Q(p1, p2) => {
					format!("Q {:.12},{:.12} {:.12},{:.12}", p1.x, p1.y, p2.x, p2.y)
				}
				AbsolutePathCommand::T(p) => format!("T {:.12},{:.12}", p.x, p.y),
				AbsolutePathCommand::A(rx, ry, x_axis_rotation, large_arc_flag, sweep_flag, p) => {
					format!("A {:.12} {:.12} {:.12} {} {} {:.12},{:.12}", rx, ry, x_axis_rotation, large_arc_flag as u8, sweep_flag as u8, p.x, p.y)
				}
				AbsolutePathCommand::Z => "Z".to_string(),
			},
			PathCommand::Relative(rel_cmd) => match rel_cmd {
				RelativePathCommand::M(dx, dy) => format!("m {:.12},{:.12}", dx, dy),
				RelativePathCommand::L(dx, dy) => format!("l {:.12},{:.12}", dx, dy),
				RelativePathCommand::H(dx) => format!("h {:.12}", dx),
				RelativePathCommand::V(dy) => format!("v {:.12}", dy),
				RelativePathCommand::C(dx1, dy1, dx2, dy2, dx, dy) => format!("c{:.12},{:.12} {:.12},{:.12} {:.12},{:.12}", dx1, dy1, dx2, dy2, dx, dy),
				RelativePathCommand::S(dx2, dy2, dx, dy) => {
					format!("s {:.12},{:.12} {:.12},{:.12}", dx2, dy2, dx, dy)
				}
				RelativePathCommand::Q(dx1, dy1, dx, dy) => {
					format!("q {:.12},{:.12} {:.12},{:.12}", dx1, dy1, dx, dy)
				}
				RelativePathCommand::T(dx, dy) => format!("t{:.12},{:.12}", dx, dy),
				RelativePathCommand::A(rx, ry, x_axis_rotation, large_arc_flag, sweep_flag, dx, dy) => {
					format!("a {:.12} {:.12} {:.12} {} {} {:.12},{:.12}", rx, ry, x_axis_rotation, large_arc_flag as u8, sweep_flag as u8, dx, dy)
				}
			},
		})
		.collect::<Vec<String>>()
		.join(" ")
}
