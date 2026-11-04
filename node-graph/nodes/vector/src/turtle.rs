use core_types::list::{ATTR_DEPTH, ATTR_DISTANCE, ATTR_HEADING, ATTR_ID, ATTR_WIDTH, Item, List};
use core_types::misc::parse_f64;
use glam::DVec2;
use graphic_types::Vector;
use vector_types::vector::{CurveType, Path, PointId, SegmentId, SetVectorPaths, push_paths};

/// How close, as a fraction of the step just drawn, a draw must land on its path's first point to close the path there.
const CLOSURE_TOLERANCE: f64 = 1e-6;

/// The turtle's place, direction, and pen, which `[` saves and `]` restores.
#[derive(Clone, Copy)]
struct Turtle {
	position: DVec2,
	/// Degrees, with `0.` along +X and positive angles turning clockwise on screen.
	heading: f64,
	distance: f64,
	depth: i64,
	width: f64,
	/// The drawing's slot for the point the turtle stands on, which a branch shares with its parent so both weld there.
	anchor_slot: usize,
	/// The run being drawn, as an index into the drawing's runs.
	run: Option<usize>,
}

impl Turtle {
	fn turn(&mut self, degrees: f64) {
		self.heading = (self.heading + degrees).rem_euclid(360.);
	}

	/// Moves forward regardless of the pen, counting the distance traveled.
	fn advance(&mut self, length: f64) {
		self.position += length * direction(self.heading);
		self.distance += length.abs();
	}
}

/// The unit vector a heading in degrees points along, exact for multiples of 90° so square lattices stay on whole numbers.
fn direction(heading: f64) -> DVec2 {
	let quarter_turns = heading / 90.;
	if quarter_turns.fract() == 0. {
		return match (quarter_turns as i64).rem_euclid(4) {
			0 => DVec2::X,
			1 => DVec2::Y,
			2 => DVec2::NEG_X,
			_ => DVec2::NEG_Y,
		};
	}

	DVec2::from_angle(heading.to_radians())
}

/// A place the turtle has stood, which gains its point the first time something is drawn from it.
#[derive(Clone, Copy)]
enum AnchorSlot {
	Drawn(u32),
	/// The turtle as it arrived, whose state stamps the point once one is drawn.
	Undrawn(Turtle),
}

/// The points and path runs a turtle lays down, with each point's attributes alongside.
#[derive(Default)]
struct Drawing {
	positions: Vec<DVec2>,
	depths: Vec<i64>,
	distances: Vec<f64>,
	headings: Vec<f64>,
	widths: Vec<f64>,
	/// Pool indices of each run, in the order the runs began.
	runs: Vec<Vec<u32>>,
	anchor_slots: Vec<AnchorSlot>,
}

impl Drawing {
	fn new_anchor_slot(&mut self, slot: AnchorSlot) -> usize {
		self.anchor_slots.push(slot);
		self.anchor_slots.len() - 1
	}

	/// Adds a point where the turtle stands, stamped with its state, and gives its pool index.
	fn push_point(&mut self, turtle: &Turtle) -> u32 {
		self.positions.push(turtle.position);
		self.depths.push(turtle.depth);
		self.distances.push(turtle.distance);
		// Reported from -180° to 180°, so the starting heading up the screen reads as -90°
		self.headings.push(180. - (180. - turtle.heading).rem_euclid(360.));
		self.widths.push(turtle.width);

		(self.positions.len() - 1) as u32
	}

	/// The point of the place the turtle stands, drawing it there if this is the first time it's needed.
	fn anchor_point(&mut self, turtle: &Turtle) -> u32 {
		let arrival = match self.anchor_slots.get(turtle.anchor_slot) {
			Some(AnchorSlot::Drawn(anchor)) => return *anchor,
			Some(AnchorSlot::Undrawn(arrival)) => *arrival,
			None => {
				log::error!("The turtle stands at a place the drawing has no slot for");
				*turtle
			}
		};

		let anchor = self.push_point(&arrival);
		if let Some(slot) = self.anchor_slots.get_mut(turtle.anchor_slot) {
			*slot = AnchorSlot::Drawn(anchor);
		}
		anchor
	}

	/// Draws a segment forward from the turtle, continuing its run or beginning one from the point it stands on.
	fn draw(&mut self, turtle: &mut Turtle, length: f64) {
		let start = self.anchor_point(turtle);
		let run = match turtle.run {
			Some(run) => run,
			None => {
				self.runs.push(vec![start]);
				self.runs.len() - 1
			}
		};

		turtle.advance(length);

		// Landing back on the run's first point closes the run there rather than adding a duplicate point
		let first = self.runs[run][0];
		let closes = self.runs[run].len() >= 3 && turtle.position.distance(self.positions[first as usize]) <= CLOSURE_TOLERANCE * length.abs();
		let end = if closes {
			turtle.position = self.positions[first as usize];
			first
		} else {
			self.push_point(turtle)
		};

		self.runs[run].push(end);
		turtle.anchor_slot = self.new_anchor_slot(AnchorSlot::Drawn(end));
		turtle.run = Some(run);
	}

	/// Builds the vector, numbering the points and then the segments in the order they were drawn.
	fn into_item(self) -> Item<Vector> {
		let point_count = self.positions.len();
		let mut vector = Vector(List::new_from_elements(self.positions));

		if point_count > 0 {
			let mut point_id = PointId::ZERO;
			vector.0.set_attribute_values(ATTR_ID, (0..point_count).map(|_| point_id.next_id()).collect::<Vec<_>>());
		}
		vector.0.set_attribute_values(ATTR_DEPTH, self.depths);
		vector.0.set_attribute_values(ATTR_DISTANCE, self.distances);
		vector.0.set_attribute_values(ATTR_HEADING, self.headings);
		vector.0.set_attribute_values(ATTR_WIDTH, self.widths);

		let mut segment_id = SegmentId::ZERO;
		let runs = self.runs.into_iter().filter(|references| references.len() > 1).map(|references| {
			let ids = (1..references.len()).map(|_| segment_id.next_id()).collect();
			(CurveType::Linear, references, ids)
		});
		let mut paths = List::<Path>::new();
		push_paths(&mut paths, runs);

		Item::new_from_element(vector).with_paths(paths)
	}
}

/// Splits a parenthesized parameter list off the front of the commands, giving its insides and the commands after it.
/// An unclosed list runs to the end of the commands.
fn split_parameters(commands: &str) -> (&str, &str) {
	let mut nesting = 0;
	for (index, character) in commands.char_indices() {
		match character {
			'(' => nesting += 1,
			')' => {
				nesting -= 1;
				if nesting == 0 {
					return (&commands[1..index], &commands[index + 1..]);
				}
			}
			_ => {}
		}
	}

	(&commands[1..], "")
}

/// Follows turtle commands from the origin, facing up the screen, and gives the paths it draws.
pub(crate) fn turtle_path(commands: &str, step: f64, angle: f64, draw_symbols: &str, move_symbols: &str) -> Item<Vector> {
	let mut drawing = Drawing::default();
	let mut turtle = Turtle {
		position: DVec2::ZERO,
		heading: 270.,
		distance: 0.,
		depth: 0,
		width: 1.,
		anchor_slot: 0,
		run: None,
	};
	turtle.anchor_slot = drawing.new_anchor_slot(AnchorSlot::Undrawn(turtle));
	let mut saved = Vec::new();

	let mut remaining = commands;
	while let Some(symbol) = remaining.chars().next() {
		remaining = &remaining[symbol.len_utf8()..];

		// A parenthesized list after a symbol belongs to it, and only its first number is read
		let parameter = if remaining.starts_with('(') {
			let (parameters, after) = split_parameters(remaining);
			remaining = after;
			parameters.split(',').next().and_then(|parameter| parse_f64(parameter.trim()))
		} else {
			None
		};

		if draw_symbols.contains(symbol) {
			drawing.draw(&mut turtle, parameter.unwrap_or(step));
			continue;
		}
		if move_symbols.contains(symbol) {
			turtle.advance(parameter.unwrap_or(step));
			turtle.run = None;
			turtle.anchor_slot = drawing.new_anchor_slot(AnchorSlot::Undrawn(turtle));
			continue;
		}

		match symbol {
			'+' => turtle.turn(-parameter.unwrap_or(angle)),
			'-' => turtle.turn(parameter.unwrap_or(angle)),
			'|' => turtle.turn(180.),
			'[' => {
				saved.push(turtle);
				turtle.depth += 1;
				turtle.run = None;
			}
			']' => {
				if let Some(restored) = saved.pop() {
					turtle = restored;
				}
			}
			'!' => {
				if let Some(width) = parameter {
					turtle.width = width;
				}
			}
			_ => {}
		}
	}

	drawing.into_item()
}
