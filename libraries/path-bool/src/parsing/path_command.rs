use glam::DVec2;

#[derive(Clone, Debug)]
pub enum AbsolutePathCommand {
	H(f64),
	V(f64),
	M(DVec2),
	L(DVec2),
	C(DVec2, DVec2, DVec2),
	S(DVec2, DVec2),
	Q(DVec2, DVec2),
	T(DVec2),
	A(f64, f64, f64, bool, bool, DVec2),
	Z,
}

#[derive(Clone, Debug)]
pub enum RelativePathCommand {
	H(f64),
	V(f64),
	M(f64, f64),
	L(f64, f64),
	C(f64, f64, f64, f64, f64, f64),
	S(f64, f64, f64, f64),
	Q(f64, f64, f64, f64),
	T(f64, f64),
	A(f64, f64, f64, bool, bool, f64, f64),
}

#[derive(Clone, Debug)]
pub enum PathCommand {
	Absolute(AbsolutePathCommand),
	Relative(RelativePathCommand),
}

pub fn to_absolute_commands<I>(commands: I) -> impl Iterator<Item = AbsolutePathCommand>
where
	I: IntoIterator<Item = PathCommand>,
{
	let mut last_point = DVec2::ZERO;
	let mut first_point = last_point;

	commands.into_iter().flat_map(move |cmd| match cmd {
		PathCommand::Absolute(abs_cmd) => {
			match abs_cmd {
				AbsolutePathCommand::H(x) => {
					last_point.x = x;
				}
				AbsolutePathCommand::V(y) => {
					last_point.y = y;
				}
				AbsolutePathCommand::M(point) => {
					last_point = point;
					first_point = point;
				}
				AbsolutePathCommand::L(point) => {
					last_point = point;
				}
				AbsolutePathCommand::C(_, _, end) => {
					last_point = end;
				}
				AbsolutePathCommand::S(_, end) => {
					last_point = end;
				}
				AbsolutePathCommand::Q(_, end) => {
					last_point = end;
				}
				AbsolutePathCommand::T(end) => {
					last_point = end;
				}
				AbsolutePathCommand::A(_, _, _, _, _, end) => {
					last_point = end;
				}
				AbsolutePathCommand::Z => {
					last_point = first_point;
				}
			}
			vec![abs_cmd]
		}
		PathCommand::Relative(rel_cmd) => match rel_cmd {
			RelativePathCommand::H(dx) => {
				last_point.x += dx;
				vec![AbsolutePathCommand::L(last_point)]
			}
			RelativePathCommand::V(dy) => {
				last_point.y += dy;
				vec![AbsolutePathCommand::L(last_point)]
			}
			RelativePathCommand::M(dx, dy) => {
				last_point += DVec2::new(dx, dy);
				first_point = last_point;
				vec![AbsolutePathCommand::M(last_point)]
			}
			RelativePathCommand::L(dx, dy) => {
				last_point += DVec2::new(dx, dy);
				vec![AbsolutePathCommand::L(last_point)]
			}
			RelativePathCommand::C(dx1, dy1, dx2, dy2, dx, dy) => {
				let c1 = last_point + DVec2::new(dx1, dy1);
				let c2 = last_point + DVec2::new(dx2, dy2);
				last_point += DVec2::new(dx, dy);
				vec![AbsolutePathCommand::C(c1, c2, last_point)]
			}
			RelativePathCommand::S(dx2, dy2, dx, dy) => {
				let c2 = last_point + DVec2::new(dx2, dy2);
				last_point += DVec2::new(dx, dy);
				vec![AbsolutePathCommand::S(c2, last_point)]
			}
			RelativePathCommand::Q(dx1, dy1, dx, dy) => {
				let control = last_point + DVec2::new(dx1, dy1);
				last_point += DVec2::new(dx, dy);
				vec![AbsolutePathCommand::Q(control, last_point)]
			}
			RelativePathCommand::T(dx, dy) => {
				last_point += DVec2::new(dx, dy);
				vec![AbsolutePathCommand::T(last_point)]
			}
			RelativePathCommand::A(rx, ry, x_axis_rotation, large_arc_flag, sweep_flag, dx, dy) => {
				last_point += DVec2::new(dx, dy);
				vec![AbsolutePathCommand::A(rx, ry, x_axis_rotation, large_arc_flag, sweep_flag, last_point)]
			}
		},
	})
}
