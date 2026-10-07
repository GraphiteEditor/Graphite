//! Drives the Graphite web editor the way a person does, with real mouse and keyboard input delivered to its own isolated browser,
//! for scripts, QA, and agents. Run it with `cargo run drive <command>`.

mod browser;
mod client;
mod contact_sheet;
mod host;
mod paths;
mod processes;
mod session;
mod socket;

use clap::{Args, Parser, Subcommand, ValueEnum};
use glam::{DVec2, UVec2};
use graphite_editor_control_protocol::{Command, CommandResult, Filmstrip, MouseButton, Rectangle, Space, parse_scenario};
use serde::Serialize;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::session::SessionOptions;

#[derive(Parser)]
#[command(
	name = "cargo run drive",
	bin_name = "cargo run drive",
	about = "Drive the Graphite web editor with real mouse and keyboard input in an isolated browser"
)]
struct Arguments {
	#[command(subcommand)]
	command: CliCommand,
}

#[derive(Subcommand)]
enum CliCommand {
	/// Download the driver's own isolated browser and the Playwright package that drives it.
	Install,

	/// Serve the editor with the Vite dev server, building its Wasm first when run through `cargo run drive`.
	Serve {
		/// The port to serve on, which `start` then opens.
		#[arg(long, default_value_t = 8080)]
		port: u16,
	},

	/// Open the editor in a new session.
	Start {
		/// The editor's address, by default the dev server that `serve` started, or else http://localhost:8080/.
		#[arg(long)]
		url: Option<String>,
		/// The page's size in CSS pixels, as <width>x<height>.
		#[arg(long, value_parser = parse_size)]
		size: Option<UVec2>,
		/// How many device pixels make one CSS pixel, for a closer look at small details.
		#[arg(long, default_value_t = 1., value_parser = parse_scale)]
		scale: f64,
		/// Show the browser's window.
		#[arg(long)]
		headed: bool,
	},

	/// Change the size of the page.
	Resize {
		/// The page's width in CSS pixels.
		width: u32,
		/// The page's height in CSS pixels.
		height: u32,
		#[command(flatten)]
		shot: Shot,
	},

	/// Report the state of the session.
	Status,

	/// Close the session.
	Stop,

	/// Close the session and the dev server that `serve` started.
	Shutdown,

	/// Move the pointer.
	#[command(allow_negative_numbers = true)]
	Move {
		/// The horizontal position in CSS pixels.
		#[arg(value_parser = parse_coordinate)]
		x: f64,
		/// The vertical position in CSS pixels.
		#[arg(value_parser = parse_coordinate)]
		y: f64,
		#[command(flatten)]
		space: InSpace,
		#[command(flatten)]
		shot: Shot,
	},

	/// Press a mouse button: left (the default), middle, or right.
	Down {
		#[arg(value_parser = parse_button)]
		button: Option<MouseButton>,
		#[command(flatten)]
		shot: Shot,
	},

	/// Release a mouse button: left (the default), middle, or right.
	Up {
		#[arg(value_parser = parse_button)]
		button: Option<MouseButton>,
		#[command(flatten)]
		shot: Shot,
	},

	/// Click at a point, with any keys held from before the first press.
	#[command(allow_negative_numbers = true)]
	Click {
		/// The horizontal position in CSS pixels.
		#[arg(value_parser = parse_coordinate)]
		x: f64,
		/// The vertical position in CSS pixels.
		#[arg(value_parser = parse_coordinate)]
		y: f64,
		#[command(flatten)]
		space: InSpace,
		#[command(flatten)]
		button: ButtonOption,
		/// How many clicks to make, such as 2 for a double click.
		#[arg(long)]
		count: Option<NonZeroU32>,
		#[command(flatten)]
		keys: Keys,
		#[command(flatten)]
		shot: Shot,
	},

	/// Drag between two points, with any keys held from before the press.
	///
	/// To press a key partway through a drag, use `drag --hold`, then `key down`, then `move`, then `up`.
	#[command(allow_negative_numbers = true)]
	Drag {
		/// The starting horizontal position in CSS pixels.
		#[arg(value_parser = parse_coordinate)]
		x1: f64,
		/// The starting vertical position in CSS pixels.
		#[arg(value_parser = parse_coordinate)]
		y1: f64,
		/// The ending horizontal position in CSS pixels.
		#[arg(value_parser = parse_coordinate)]
		x2: f64,
		/// The ending vertical position in CSS pixels.
		#[arg(value_parser = parse_coordinate)]
		y2: f64,
		#[command(flatten)]
		space: InSpace,
		#[command(flatten)]
		button: ButtonOption,
		#[command(flatten)]
		keys: Keys,
		/// How many steps to move in, instead of one for every 8 pixels.
		#[arg(long)]
		steps: Option<NonZeroU32>,
		/// Keep the button and keys held once the drag is done.
		#[arg(long)]
		hold: bool,
		/// Capture frames along the drag and lay them out in this image, each with a crosshair where the pointer was.
		#[arg(long)]
		filmstrip: Option<PathBuf>,
		/// How many frames the filmstrip captures.
		#[arg(long, requires = "filmstrip")]
		frames: Option<NonZeroU32>,
		#[command(flatten)]
		area: CaptureArea,
		#[command(flatten)]
		shot: Shot,
	},

	/// Hold, release, or tap a key such as Shift, Control, Alt, Enter, or KeyP, where only `press` also takes a combination such as Control+KeyZ.
	Key {
		/// Whether to hold, release, or tap the key.
		action: KeyAction,
		/// The key, named as in `KeyboardEvent.key` or `KeyboardEvent.code`.
		key: String,
		#[command(flatten)]
		shot: Shot,
	},

	/// Type text into the focused field.
	Type {
		/// The text to type, which may be given as several arguments joined by spaces.
		#[arg(required = true)]
		text: Vec<String>,
		#[command(flatten)]
		shot: Shot,
	},

	/// Turn the mouse wheel, in notches of up to 100 pixels.
	#[command(allow_negative_numbers = true)]
	Scroll {
		/// How far to scroll horizontally, in pixels.
		#[arg(value_parser = parse_coordinate)]
		delta_x: f64,
		/// How far to scroll vertically, in pixels.
		#[arg(value_parser = parse_coordinate)]
		delta_y: f64,
		/// Move the pointer here first, as <x>,<y>.
		#[arg(long, value_parser = parse_point)]
		at: Option<DVec2>,
		#[command(flatten)]
		space: InSpace,
		#[command(flatten)]
		shot: Shot,
	},

	/// Capture the page.
	Screenshot {
		/// The image to write, with a relative path landing in the output folder.
		file: PathBuf,
		/// Mark where the pointer is with a crosshair.
		#[arg(long)]
		cursor: bool,
		#[command(flatten)]
		area: CaptureArea,
	},

	/// Find elements by a `data-*` attribute name, the visible text they contain, or both.
	Locate {
		/// The name of a `data-*` attribute the elements have, without the `data-` prefix.
		#[arg(long)]
		data: Option<String>,
		/// Text the elements contain.
		#[arg(long)]
		text: Option<String>,
	},

	/// Pause for a number of milliseconds, or with `idle`, wait until the editor stops redrawing.
	Wait {
		/// A number of milliseconds, or `idle`.
		duration: String,
		#[command(flatten)]
		shot: Shot,
	},

	/// Perform a JSON array of commands.
	Act {
		/// The commands, as a JSON array.
		json: String,
		#[command(flatten)]
		shot: Shot,
	},

	/// Perform the steps of a scenario file.
	Run {
		/// The scenario file, which holds one command per line written as JSON, along with any blank lines and `//` comments.
		file: PathBuf,
		#[command(flatten)]
		shot: Shot,
	},

	/// Host a session, which `start` runs in the background.
	#[command(hide = true)]
	Host {
		#[arg(long)]
		url: String,
		#[arg(long)]
		width: u32,
		#[arg(long)]
		height: u32,
		#[arg(long)]
		scale: f64,
		#[arg(long)]
		headed: bool,
	},
}

#[derive(Args)]
struct Shot {
	/// Once the command is done and the editor is idle, capture a screenshot to this file.
	#[arg(long = "shot", id = "shot")]
	path: Option<PathBuf>,
}

#[derive(Args)]
struct InSpace {
	/// Measure points from the document viewport's top left corner (`viewport`) instead of the page's (`page`).
	#[arg(long = "in", value_parser = parse_space)]
	space: Option<Space>,
}

#[derive(Args)]
struct ButtonOption {
	/// The mouse button: left (the default), middle, or right.
	#[arg(long, value_parser = parse_button)]
	button: Option<MouseButton>,
}

#[derive(Args)]
struct Keys {
	/// Keys to hold, separated by commas, such as Shift,Alt.
	#[arg(long, value_delimiter = ',')]
	keys: Option<Vec<String>>,
}

#[derive(Args)]
struct CaptureArea {
	/// Limit the capture to the document viewport (`viewport`) or the first element matching a CSS selector.
	#[arg(long)]
	region: Option<String>,
	/// Limit the capture to an exact area of the page, as <x>,<y>,<width>,<height>.
	#[arg(long, value_parser = parse_clip)]
	clip: Option<Rectangle>,
}

#[derive(Clone, Copy, ValueEnum)]
enum KeyAction {
	Down,
	Up,
	Press,
}

// A number that a position or distance can be, which rules out infinities and NaN
fn parse_coordinate(value: &str) -> Result<f64, String> {
	value
		.trim()
		.parse::<f64>()
		.ok()
		.filter(|number| number.is_finite())
		.ok_or_else(|| format!("Expected a finite number, not \"{value}\""))
}

fn parse_scale(value: &str) -> Result<f64, String> {
	parse_coordinate(value)
		.ok()
		.filter(|scale| *scale > 0.)
		.ok_or_else(|| format!("Expected a positive number, not \"{value}\""))
}

fn parse_numbers<const COUNT: usize>(value: &str, separator: char) -> Result<[f64; COUNT], String> {
	let numbers = value
		.split(separator)
		.map(|part| parse_coordinate(part).ok())
		.collect::<Option<Vec<_>>>()
		.and_then(|numbers| <[f64; COUNT]>::try_from(numbers).ok());
	numbers.ok_or_else(|| format!("Expected {COUNT} numbers separated by \"{separator}\""))
}

fn parse_point(value: &str) -> Result<DVec2, String> {
	parse_numbers::<2>(value, ',').map(DVec2::from)
}

fn parse_size(value: &str) -> Result<UVec2, String> {
	let [width, height] = parse_numbers::<2>(value, 'x')?;
	if width < 1. || height < 1. || width.fract() != 0. || height.fract() != 0. {
		return Err("Expected a width and height that are positive integers".to_string());
	}
	Ok(UVec2::new(width as u32, height as u32))
}

fn parse_clip(value: &str) -> Result<Rectangle, String> {
	let [x, y, width, height] = parse_numbers::<4>(value, ',')?;
	Ok(Rectangle { x, y, width, height })
}

fn parse_space(value: &str) -> Result<Space, String> {
	match value {
		"page" => Ok(Space::Page),
		"viewport" => Ok(Space::Viewport),
		_ => Err(format!("Expected \"page\" or \"viewport\", not \"{value}\"")),
	}
}

fn parse_button(value: &str) -> Result<MouseButton, String> {
	match value {
		"left" => Ok(MouseButton::Left),
		"middle" => Ok(MouseButton::Middle),
		"right" => Ok(MouseButton::Right),
		_ => Err(format!("Unknown mouse button \"{value}\"")),
	}
}

// The commands a command line invocation performs, and where it captures a screenshot afterward if asked to
fn commands_for(command: CliCommand) -> Result<(Vec<Command>, Option<PathBuf>), String> {
	let no_shot = Shot { path: None };

	let (commands, shot) = match command {
		CliCommand::Resize { width, height, shot } => (vec![Command::Resize { size: UVec2::new(width, height) }], shot),
		CliCommand::Status => (vec![Command::Status {}], no_shot),
		CliCommand::Move { x, y, space, shot } => (
			vec![Command::Move {
				to: DVec2::new(x, y),
				space: space.space,
				steps: None,
			}],
			shot,
		),
		CliCommand::Down { button, shot } => (vec![Command::Down { button }], shot),
		CliCommand::Up { button, shot } => (vec![Command::Up { button }], shot),
		CliCommand::Click {
			x,
			y,
			space,
			button,
			count,
			keys,
			shot,
		} => (
			vec![Command::Click {
				at: DVec2::new(x, y),
				space: space.space,
				button: button.button,
				count,
				keys: keys.keys,
			}],
			shot,
		),
		CliCommand::Drag {
			x1,
			y1,
			x2,
			y2,
			space,
			button,
			keys,
			steps,
			hold,
			filmstrip,
			frames,
			area,
			shot,
		} => {
			let filmstrip = filmstrip.map(|path| Filmstrip {
				path,
				frames,
				region: area.region,
				clip: area.clip,
			});
			let drag = Command::Drag {
				from: DVec2::new(x1, y1),
				to: DVec2::new(x2, y2),
				space: space.space,
				button: button.button,
				keys: keys.keys,
				steps,
				release: Some(!hold),
				filmstrip,
			};
			(vec![drag], shot)
		}
		CliCommand::Key { action, key, shot } => {
			let command = match action {
				KeyAction::Down => Command::KeyDown { key },
				KeyAction::Up => Command::KeyUp { key },
				KeyAction::Press => Command::Press { key },
			};
			(vec![command], shot)
		}
		CliCommand::Type { text, shot } => (vec![Command::Type { text: text.join(" ") }], shot),
		CliCommand::Scroll { delta_x, delta_y, at, space, shot } => (
			vec![Command::Scroll {
				delta: DVec2::new(delta_x, delta_y),
				at,
				space: space.space,
			}],
			shot,
		),
		CliCommand::Screenshot { file, cursor, area } => (
			vec![Command::Screenshot {
				path: file,
				cursor: Some(cursor),
				region: area.region,
				clip: area.clip,
			}],
			no_shot,
		),
		CliCommand::Locate { data, text } => (vec![Command::Locate { data, text }], no_shot),
		CliCommand::Wait { duration, shot } => {
			let command = if duration == "idle" {
				Command::WaitIdle { timeout: None }
			} else {
				let milliseconds = duration.parse().map_err(|_| format!("Expected a number of milliseconds or \"idle\", not \"{duration}\""))?;
				Command::Wait { milliseconds }
			};
			(vec![command], shot)
		}
		CliCommand::Act { json, shot } => {
			let commands = serde_json::from_str(&json).map_err(|error| format!("Expected a JSON array of commands: {error}"))?;
			(commands, shot)
		}
		CliCommand::Run { file, shot } => {
			let contents = std::fs::read_to_string(&file).map_err(|error| format!("Failed to read {}: {error}", file.display()))?;
			let steps = parse_scenario(&contents).map_err(|error| format!("The scenario in {} could not be read. {error}", file.display()))?;
			(steps, shot)
		}
		CliCommand::Install | CliCommand::Serve { .. } | CliCommand::Start { .. } | CliCommand::Stop | CliCommand::Shutdown | CliCommand::Host { .. } => {
			return Err("This command does not act on the editor".to_string());
		}
	};

	Ok((commands, shot.path))
}

fn print_json(value: &impl Serialize) {
	let mut output = Vec::new();
	let mut serializer = serde_json::Serializer::with_formatter(&mut output, serde_json::ser::PrettyFormatter::with_indent(b"\t"));
	match value.serialize(&mut serializer) {
		Ok(()) => println!("{}", String::from_utf8_lossy(&output)),
		Err(error) => eprintln!("Failed to print the results: {error}"),
	}
}

fn run_program(program: &mut std::process::Command, failure: &str) -> Result<(), String> {
	let status = program.status().map_err(|error| format!("{failure}: {error}"))?;
	if status.success() { Ok(()) } else { Err(failure.to_string()) }
}

fn install() -> Result<(), String> {
	let directory = paths::playwright_directory();
	let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
	run_program(
		std::process::Command::new(npm).args(["ci", "--no-audit", "--no-fund"]).current_dir(&directory),
		"Failed to install Playwright",
	)?;

	// The full browser runs headless too, so the lighter headless shell is left out
	let playwright = directory.join("node_modules").join("playwright-core").join("cli.js");
	run_program(
		std::process::Command::new("node")
			.arg(playwright)
			.args(["install", "--no-shell", "chromium"])
			.env("PLAYWRIGHT_BROWSERS_PATH", paths::browsers_directory()),
		"The browser failed to download",
	)
}

// Returns whether every command succeeded
fn run(command: CliCommand) -> Result<bool, String> {
	match command {
		CliCommand::Install => install()?,
		CliCommand::Serve { port } => {
			let record = processes::start_dev_server(port)?;
			println!("Serving the editor at http://127.0.0.1:{}/", record.port);
		}
		CliCommand::Start { url, size, scale, headed } => {
			let url = url
				.or_else(|| processes::dev_server_record().map(|record| format!("http://127.0.0.1:{}/", record.port)))
				.unwrap_or_else(|| paths::DEFAULT_EDITOR_URL.to_string());
			let size = size.unwrap_or(paths::DEFAULT_WINDOW_SIZE);

			client::start_session(&SessionOptions {
				url: url.clone(),
				size,
				scale,
				headed,
			})?;
			println!("The editor is open at {url}");
		}
		CliCommand::Stop => client::stop_session(),
		CliCommand::Shutdown => {
			client::stop_session();
			processes::stop_dev_server();
		}
		CliCommand::Host { url, width, height, scale, headed } => {
			let size = UVec2::new(width, height);
			host::run(SessionOptions { url, size, scale, headed })?;
		}
		command => {
			let (mut commands, shot) = commands_for(command)?;
			if let Some(path) = shot {
				commands.push(Command::WaitIdle { timeout: None });
				commands.push(Command::Screenshot {
					path,
					cursor: None,
					region: None,
					clip: None,
				});
			}

			let results: Vec<CommandResult> = client::perform(commands)?;
			print_json(&results);
			return Ok(results.iter().all(|result| result.ok));
		}
	}

	Ok(true)
}

fn main() -> ExitCode {
	match run(Arguments::parse().command) {
		Ok(true) => ExitCode::SUCCESS,
		Ok(false) => ExitCode::FAILURE,
		Err(error) => {
			eprintln!("{error}");
			ExitCode::FAILURE
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::Path;

	#[test]
	fn arguments_parse() {
		assert_eq!(parse_point("10,-20.5").unwrap(), DVec2::new(10., -20.5));
		assert_eq!(parse_size("1600x1000").unwrap(), UVec2::new(1600, 1000));
		assert!(parse_size("1600x0").is_err());
		assert!(parse_clip("1,2,3").is_err());
		assert!(parse_space("document").is_err());
		assert!(parse_scale("0").is_err());
	}

	#[test]
	fn command_line_is_well_formed() {
		<Arguments as clap::CommandFactory>::command().debug_assert();
	}

	#[test]
	fn command_lines_parse() {
		Arguments::try_parse_from([
			"driver",
			"drag",
			"10",
			"-20",
			"30",
			"40",
			"--in",
			"viewport",
			"--keys",
			"Shift,Alt",
			"--filmstrip",
			"drag.png",
			"--frames",
			"4",
		])
		.unwrap();
		Arguments::try_parse_from(["driver", "scroll", "0", "-300", "--at", "100,200"]).unwrap();
		Arguments::try_parse_from(["driver", "key", "press", "Control+KeyZ", "--shot", "undo.png"]).unwrap();
		assert!(Arguments::try_parse_from(["driver", "screenshot", "a.png", "--shot", "b.png"]).is_err());
	}

	#[test]
	fn non_finite_numbers_are_refused() {
		assert!(Arguments::try_parse_from(["driver", "move", "inf", "0"]).is_err());
		assert!(Arguments::try_parse_from(["driver", "move", "0", "NaN"]).is_err());
		assert!(Arguments::try_parse_from(["driver", "scroll", "0", "-infinity"]).is_err());
		assert!(Arguments::try_parse_from(["driver", "start", "--scale", "inf"]).is_err());
	}

	#[test]
	fn relative_paths_land_in_the_output_directory() {
		assert_eq!(paths::output_path(Path::new("shot.png")), paths::output_directory().join("shot.png"));

		let nested = paths::output_path(Path::new("folder/shot.png"));
		assert_eq!(nested.as_os_str(), paths::output_directory().join("folder").join("shot.png").as_os_str());
	}
}
