use core_types::list::{ATTR_DEPTH, ATTR_DISTANCE, ATTR_HEADING, ATTR_ID, ATTR_KIND, ATTR_STEP, ATTR_UV, ATTR_WIDTH, Item, List};
use core_types::misc::parse_f64;
use glam::DVec2;
use graphic_types::Vector;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use vector_types::vector::{CurveType, Path, PointId, SegmentId, SetVectorPaths, push_paths};

/// How close, as a fraction of the step just drawn, a draw must land on its path's first point to close the path there.
const CLOSURE_TOLERANCE: f64 = 1e-6;

/// How far `~` may turn either way, unless a command gives its own limit like `~(30)`.
const RANDOM_TURN_LIMIT: f64 = 180.;

/// The kind assigned to every drawn point, matching what points from elsewhere without a kind read as.
const DRAWN_KIND: i64 = 0;

/// The kind of a point that `.` places outside a shape without a number of its own.
const PLACED_KIND: i64 = 1;

/// The node's inputs, which the commands fall back to when they don't give their own numbers.
pub(crate) struct TurtleSettings {
	pub step: f64,
	pub step_scale: f64,
	/// The most a step's length varies at random, as a fraction of it.
	pub step_jitter: f64,
	pub angle: f64,
	pub angle_scale: f64,
	/// The most a turn's angle varies at random, as a fraction of it.
	pub angle_jitter: f64,
	pub width: f64,
	pub width_scale: f64,
	/// The most the width a line draws to varies at random, as a fraction of it.
	pub width_jitter: f64,
	pub tropism: f64,
	pub uv_increment: DVec2,
	pub seed: u64,
}

#[cfg(test)]
impl Default for TurtleSettings {
	/// The node's default inputs.
	fn default() -> Self {
		Self {
			step: 10.,
			step_scale: 0.5,
			step_jitter: 0.,
			angle: 90.,
			angle_scale: 0.5,
			angle_jitter: 0.,
			width: 1.,
			width_scale: 0.5,
			width_jitter: 0.,
			tropism: 10.,
			uv_increment: DVec2::ONE,
			seed: 0,
		}
	}
}

/// The turtle's place, direction, and pen, which `[` saves and `]` restores.
#[derive(Clone, Copy)]
struct Turtle {
	position: DVec2,
	/// Degrees, with `0.` along +X and positive angles turning clockwise on screen.
	heading: f64,
	distance: f64,
	depth: i64,
	/// The length of a step, which `"` and `_` scale.
	step: f64,
	/// The angle of a turn, which `;` and `@` scale.
	angle: f64,
	/// The width of the line, which `!` and `?` scale.
	width: f64,
	/// The U and V numbers, which `'` and `#` count up.
	uv: DVec2,
	/// The drawing's slot for the point the turtle stands on, which a branch shares with its parent so both weld there.
	anchor_slot: usize,
	/// The run being drawn, as an index into the drawing's runs.
	run: Option<usize>,
}

impl Turtle {
	fn turn(&mut self, degrees: f64) {
		self.heading = (self.heading + degrees).rem_euclid(360.);
	}

	/// Turns a sideways heading `tropism` degrees toward straight down, and less the nearer it is to vertical, without turning past it.
	/// A negative tropism bends toward straight up instead.
	fn bend_down(&mut self, tropism: f64) {
		let target = if tropism >= 0. { 90. } else { 270. };
		let remaining = (target - self.heading + 180.).rem_euclid(360.) - 180.;

		// The sideways part of the heading, which is exactly zero when vertical
		let sideways = direction(self.heading).x;
		self.turn((tropism * sideways).clamp(-remaining.abs(), remaining.abs()));
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

/// Scales an amount by a random factor within `jitter` of 1, only drawing from the generator when there's variation to give.
fn vary(amount: f64, jitter: f64, rng: &mut StdRng) -> f64 {
	if jitter == 0. {
		return amount;
	}

	amount * (1. + jitter * rng.random_range(-1. ..=1.))
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
	uvs: Vec<DVec2>,
	steps: Vec<f64>,
	kinds: Vec<i64>,
	/// Pool indices of each run, in the order the runs began.
	runs: Vec<Vec<u32>>,
	anchor_slots: Vec<AnchorSlot>,
}

impl Drawing {
	fn new_anchor_slot(&mut self, slot: AnchorSlot) -> usize {
		self.anchor_slots.push(slot);
		self.anchor_slots.len() - 1
	}

	/// Adds a point where the turtle stands, stamped with its state and a kind, and gives its pool index.
	fn push_point(&mut self, turtle: &Turtle, kind: i64) -> u32 {
		self.positions.push(turtle.position);
		self.depths.push(turtle.depth);
		self.distances.push(turtle.distance);
		// Reported from -180° to 180°, so the starting heading up the screen reads as -90°
		self.headings.push(180. - (180. - turtle.heading).rem_euclid(360.));
		self.widths.push(turtle.width);
		self.uvs.push(turtle.uv);
		self.steps.push(turtle.step);
		self.kinds.push(kind);

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

		let anchor = self.push_point(&arrival, DRAWN_KIND);
		if let Some(slot) = self.anchor_slots.get_mut(turtle.anchor_slot) {
			*slot = AnchorSlot::Drawn(anchor);
		}
		anchor
	}

	/// Draws a segment forward from the turtle, continuing its run or beginning one from the point it stands on.
	/// The `width` stamps only the point drawn to, leaving the turtle's own width as it was.
	fn draw(&mut self, turtle: &mut Turtle, length: f64, width: f64) {
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
			self.push_point(&Turtle { width, ..*turtle }, DRAWN_KIND)
		};

		self.runs[run].push(end);
		turtle.anchor_slot = self.new_anchor_slot(AnchorSlot::Drawn(end));

		// A closed run is finished, so drawing on begins a new one from the point it closed on
		turtle.run = if closes { None } else { Some(run) };
	}

	/// Moves the turtle forward without drawing, so its next draw begins a new run.
	fn travel(&mut self, turtle: &mut Turtle, length: f64) {
		turtle.advance(length);
		turtle.run = None;
		turtle.anchor_slot = self.new_anchor_slot(AnchorSlot::Undrawn(*turtle));
	}

	/// Adds a closed path through the places the turtle recorded with `.`, leaving out a last one that returns to the first.
	fn add_polygon(&mut self, mut anchors: Vec<Turtle>) {
		if let [first, .., before_last, last] = anchors.as_slice()
			&& last.position.distance(first.position) <= CLOSURE_TOLERANCE * last.position.distance(before_last.position)
		{
			anchors.pop();
		}
		if anchors.len() < 3 {
			return;
		}

		let mut references: Vec<u32> = anchors.iter().map(|anchor| self.push_point(anchor, DRAWN_KIND)).collect();
		references.push(references[0]);
		self.runs.push(references);
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
		vector.0.set_attribute_values(ATTR_UV, self.uvs);
		vector.0.set_attribute_values(ATTR_STEP, self.steps);
		vector.0.set_attribute_values(ATTR_KIND, self.kinds);

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
pub(crate) fn turtle_path(commands: &str, settings: &TurtleSettings) -> Item<Vector> {
	let mut drawing = Drawing::default();
	let mut turtle = Turtle {
		position: DVec2::ZERO,
		heading: 270.,
		distance: 0.,
		depth: 0,
		step: settings.step,
		angle: settings.angle,
		width: settings.width,
		uv: DVec2::ZERO,
		anchor_slot: 0,
		run: None,
	};
	turtle.anchor_slot = drawing.new_anchor_slot(AnchorSlot::Undrawn(turtle));
	let mut saved = Vec::new();

	// Each source of randomness gets its own generator, seeded far from the others, so a jitter turned up from 0 changes the drawing gradually and leaves the rest alone
	let generator = |source: u64| StdRng::seed_from_u64(settings.seed ^ source.wrapping_mul(0x9E37_79B9_7F4A_7C15));
	let mut random_turn_rng = generator(0);
	let mut step_jitter_rng = generator(1);
	let mut angle_jitter_rng = generator(2);
	let mut width_jitter_rng = generator(3);

	// The anchors recorded by `.` for each shape that `{` began and `}` hasn't yet closed, innermost last
	let mut polygons: Vec<Vec<Turtle>> = Vec::new();

	let mut remaining = commands;
	while let Some(symbol) = remaining.chars().next() {
		remaining = &remaining[symbol.len_utf8()..];

		// A parenthesized list after a symbol belongs to it
		let parameters = if remaining.starts_with('(') {
			let (parameters, after) = split_parameters(remaining);
			remaining = after;
			parameters
		} else {
			""
		};
		let number = |index: usize| parameters.split(',').nth(index).and_then(|parameter| parse_f64(parameter.trim()));

		match symbol {
			'F' | 'G' => {
				let length = vary(number(0).unwrap_or(turtle.step), settings.step_jitter, &mut step_jitter_rng);
				let width = vary(number(1).unwrap_or(turtle.width), settings.width_jitter, &mut width_jitter_rng);
				drawing.draw(&mut turtle, length, width);
			}
			'f' | 'g' => {
				let length = vary(number(0).unwrap_or(turtle.step), settings.step_jitter, &mut step_jitter_rng);
				drawing.travel(&mut turtle, length);
			}
			'H' => {
				let length = vary(number(0).unwrap_or(turtle.step) / 2., settings.step_jitter, &mut step_jitter_rng);
				let width = vary(number(1).unwrap_or(turtle.width), settings.width_jitter, &mut width_jitter_rng);
				drawing.draw(&mut turtle, length, width);
			}
			'h' => {
				let length = vary(number(0).unwrap_or(turtle.step) / 2., settings.step_jitter, &mut step_jitter_rng);
				drawing.travel(&mut turtle, length);
			}
			'+' => turtle.turn(-vary(number(0).unwrap_or(turtle.angle), settings.angle_jitter, &mut angle_jitter_rng)),
			'-' => turtle.turn(vary(number(0).unwrap_or(turtle.angle), settings.angle_jitter, &mut angle_jitter_rng)),
			'|' => turtle.turn(180.),
			'~' => turtle.turn(number(0).unwrap_or(RANDOM_TURN_LIMIT) * random_turn_rng.random_range(-1. ..=1.)),
			'T' => turtle.bend_down(number(0).unwrap_or(settings.tropism)),
			'"' => turtle.step *= number(0).unwrap_or(settings.step_scale),
			'_' => {
				let divisor = number(0).unwrap_or(settings.step_scale);
				if divisor != 0. {
					turtle.step /= divisor;
				}
			}
			';' => turtle.angle *= number(0).unwrap_or(settings.angle_scale),
			'@' => {
				let divisor = number(0).unwrap_or(settings.angle_scale);
				if divisor != 0. {
					turtle.angle /= divisor;
				}
			}
			'!' => turtle.width *= number(0).unwrap_or(settings.width_scale),
			'?' => {
				let divisor = number(0).unwrap_or(settings.width_scale);
				if divisor != 0. {
					turtle.width /= divisor;
				}
			}
			'\'' => turtle.uv.x += number(0).unwrap_or(settings.uv_increment.x),
			'#' => turtle.uv.y += number(0).unwrap_or(settings.uv_increment.y),
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
			'{' => polygons.push(Vec::new()),
			'.' => match polygons.last_mut() {
				Some(anchors) => anchors.push(turtle),
				// Outside a shape, the point joins no path, so it marks a position without drawing anything
				None => {
					let kind = number(0).map_or(PLACED_KIND, |kind| kind.round() as i64);
					drawing.push_point(&turtle, kind);
				}
			},
			'}' => {
				if let Some(anchors) = polygons.pop() {
					drawing.add_polygon(anchors);
				}
			}
			_ => {}
		}
	}

	// Shapes still open when the commands end close as if their `}` came last, innermost first
	while let Some(anchors) = polygons.pop() {
		drawing.add_polygon(anchors);
	}

	drawing.into_item()
}
