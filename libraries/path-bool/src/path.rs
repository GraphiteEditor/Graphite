pub(crate) mod intersection_path_segment;
pub(crate) mod line_segment;
pub(crate) mod line_segment_aabb;
pub(crate) mod path_cubic_segment_self_intersection;
pub(crate) mod path_segment;

use glam::DVec2;

#[cfg(feature = "parsing")]
use crate::path_command::{AbsolutePathCommand, PathCommand, to_absolute_commands};
use crate::path_segment::PathSegment;

pub type Path = Vec<PathSegment>;

fn reflect_control_point(point: DVec2, control_point: DVec2) -> DVec2 {
	point * 2. - control_point
}

#[cfg(feature = "parsing")]
pub fn path_from_commands<I>(commands: I) -> impl Iterator<Item = PathSegment>
where
	I: IntoIterator<Item = PathCommand>,
{
	let mut first_point: Option<DVec2> = None;
	let mut last_point: Option<DVec2> = None;
	let mut last_control_point: Option<DVec2> = None;

	to_absolute_commands(commands).filter_map(move |cmd| match cmd {
		AbsolutePathCommand::M(point) => {
			last_point = Some(point);
			first_point = Some(point);
			last_control_point = None;
			None
		}
		AbsolutePathCommand::L(point) => {
			let start = last_point.unwrap();
			last_point = Some(point);
			last_control_point = None;
			Some(PathSegment::Line(start, point))
		}
		AbsolutePathCommand::H(x) => {
			let start = last_point.unwrap();
			let point = DVec2::new(x, start.y);
			last_point = Some(point);
			last_control_point = None;
			Some(PathSegment::Line(start, point))
		}
		AbsolutePathCommand::V(y) => {
			let start = last_point.unwrap();
			let point = DVec2::new(start.x, y);
			last_point = Some(point);
			last_control_point = None;
			Some(PathSegment::Line(start, point))
		}
		AbsolutePathCommand::C(c1, c2, end) => {
			let start = last_point.unwrap();
			last_point = Some(end);
			last_control_point = Some(c2);
			Some(PathSegment::Cubic(start, c1, c2, end))
		}
		AbsolutePathCommand::S(c2, end) => {
			let start = last_point.unwrap();
			let c1 = reflect_control_point(start, last_control_point.unwrap_or(start));
			last_point = Some(end);
			last_control_point = Some(c2);
			Some(PathSegment::Cubic(start, c1, c2, end))
		}
		AbsolutePathCommand::Q(c, end) => {
			let start = last_point.unwrap();
			last_point = Some(end);
			last_control_point = Some(c);
			Some(PathSegment::Quadratic(start, c, end))
		}
		AbsolutePathCommand::T(end) => {
			let start = last_point.unwrap();
			let c = reflect_control_point(start, last_control_point.unwrap_or(start));
			last_point = Some(end);
			last_control_point = Some(c);
			Some(PathSegment::Quadratic(start, c, end))
		}
		AbsolutePathCommand::A(rx, ry, x_axis_rotation, large_arc_flag, sweep_flag, end) => {
			let start = last_point.unwrap();
			last_point = Some(end);
			last_control_point = None;
			Some(PathSegment::Arc(start, rx, ry, x_axis_rotation, large_arc_flag, sweep_flag, end))
		}
		AbsolutePathCommand::Z => {
			let start = last_point.unwrap();
			let end = first_point.unwrap();
			last_point = Some(end);
			last_control_point = None;
			Some(PathSegment::Line(start, end))
		}
	})
}

#[cfg(feature = "parsing")]
pub fn path_to_commands<'a, I>(segments: I, eps: f64) -> impl Iterator<Item = PathCommand> + 'a
where
	I: IntoIterator<Item = &'a PathSegment> + 'a,
{
	let mut last_point: Option<DVec2> = None;

	segments
		.into_iter()
		.flat_map(move |seg| {
			let start = seg.start();
			let mut commands = Vec::new();

			if last_point.is_none_or(|lp| !start.abs_diff_eq(lp, eps)) {
				if last_point.is_some() {
					commands.push(PathCommand::Absolute(AbsolutePathCommand::Z));
				}

				commands.push(PathCommand::Absolute(AbsolutePathCommand::M(start)));
			}

			match seg {
				PathSegment::Line(_, end) => {
					commands.push(PathCommand::Absolute(AbsolutePathCommand::L(*end)));
					last_point = Some(*end);
				}
				PathSegment::Cubic(_, c1, c2, end) => {
					commands.push(PathCommand::Absolute(AbsolutePathCommand::C(*c1, *c2, *end)));
					last_point = Some(*end);
				}
				PathSegment::Quadratic(_, c, end) => {
					commands.push(PathCommand::Absolute(AbsolutePathCommand::Q(*c, *end)));
					last_point = Some(*end);
				}
				PathSegment::Arc(_, rx, ry, x_axis_rotation, large_arc_flag, sweep_flag, end) => {
					commands.push(PathCommand::Absolute(AbsolutePathCommand::A(*rx, *ry, *x_axis_rotation, *large_arc_flag, *sweep_flag, *end)));
					last_point = Some(*end);
				}
			}

			commands
		})
		.chain(std::iter::once(PathCommand::Absolute(AbsolutePathCommand::Z)))
}
