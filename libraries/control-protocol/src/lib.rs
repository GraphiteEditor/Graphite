//! The commands that scripts, QA, and agents send to operate the Graphite editor the way a person does, the results they get back,
//! the scenario files that hold a sequence of commands, and the messages that carry them between a client and an editor host.
//!
//! Each message travels as one line of JSON.

use glam::{DVec2, UVec2};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use std::path::PathBuf;

/// Increases whenever a change would break a client or host written against an earlier version.
pub const PROTOCOL_VERSION: u32 = 1;

/// Which origin a point is measured from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Space {
	/// The page's top left corner.
	#[default]
	Page,
	/// The document viewport's top left corner.
	Viewport,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MouseButton {
	#[default]
	Left,
	Middle,
	Right,
}

/// An area of the page, in CSS pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Rectangle {
	pub x: f64,
	pub y: f64,
	pub width: f64,
	pub height: f64,
}

/// Frames captured along a drag and laid out in one image, each marked with where the pointer was.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filmstrip {
	/// Where the image is written, with a relative path landing in the host's output directory.
	pub path: PathBuf,
	/// How many frames to capture, spread evenly along the drag.
	pub frames: Option<NonZeroU32>,
	/// Limits each frame to the document viewport (`"viewport"`) or the first element matching a CSS selector.
	pub region: Option<String>,
	/// Limits each frame to an exact area of the page, overriding the region.
	pub clip: Option<Rectangle>,
}

/// One step of operating the editor with the mouse and keyboard, or of observing the result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum Command {
	/// Moves the pointer one small step per rendered frame, as a real pointer does, or in the given number of steps.
	Move { to: DVec2, space: Option<Space>, steps: Option<NonZeroU32> },

	/// Presses a mouse button.
	Down { button: Option<MouseButton> },

	/// Releases a mouse button.
	Up { button: Option<MouseButton> },

	/// Moves the pointer to a point and clicks there, with the keys held from before the first press.
	Click {
		at: DVec2,
		space: Option<Space>,
		button: Option<MouseButton>,
		count: Option<NonZeroU32>,
		keys: Option<Vec<String>>,
	},

	/// Drags between two points with the keys held from before the press.
	/// With `release` set to false, the button and keys stay held once the drag succeeds.
	Drag {
		from: DVec2,
		to: DVec2,
		space: Option<Space>,
		button: Option<MouseButton>,
		keys: Option<Vec<String>>,
		steps: Option<NonZeroU32>,
		release: Option<bool>,
		filmstrip: Option<Filmstrip>,
	},

	/// Holds a key, named as in `KeyboardEvent.key` or `KeyboardEvent.code`, such as `Shift` or `KeyP`.
	KeyDown { key: String },

	/// Releases a held key.
	KeyUp { key: String },

	/// Taps a key, or a combination such as `Control+KeyZ`, where a final `+` is the plus key itself.
	Press { key: String },

	/// Types text into the focused field.
	Type { text: String },

	/// Turns the mouse wheel, in notches of up to 100 pixels, optionally after moving the pointer to a point.
	Scroll { delta: DVec2, at: Option<DVec2>, space: Option<Space> },

	/// Pauses for a number of milliseconds.
	Wait { milliseconds: u64 },

	/// Waits until the editor stops redrawing, giving up after the timeout in milliseconds.
	WaitIdle { timeout: Option<u64> },

	/// Captures the page, or part of it, optionally marking the pointer with a crosshair.
	Screenshot {
		path: PathBuf,
		cursor: Option<bool>,
		region: Option<String>,
		clip: Option<Rectangle>,
	},

	/// Finds elements by a `data-*` attribute name, the visible text they contain, or both.
	Locate { data: Option<String>, text: Option<String> },

	/// Fails unless the page shows the expected elements, found as `locate` finds them and narrowed to those with a field holding `value`.
	/// With only a `value`, it looks among every field. It expects exactly `count` elements, or at least one without a count.
	Expect {
		data: Option<String>,
		text: Option<String>,
		value: Option<String>,
		count: Option<u32>,
	},

	/// Changes the size of the page, in CSS pixels.
	Resize { size: UVec2 },

	/// Reports the state of the session.
	Status {},
}

impl Command {
	/// The name the command is given by its `type` field.
	pub fn name(&self) -> &'static str {
		match self {
			Command::Move { .. } => "move",
			Command::Down { .. } => "down",
			Command::Up { .. } => "up",
			Command::Click { .. } => "click",
			Command::Drag { .. } => "drag",
			Command::KeyDown { .. } => "keyDown",
			Command::KeyUp { .. } => "keyUp",
			Command::Press { .. } => "press",
			Command::Type { .. } => "type",
			Command::Scroll { .. } => "scroll",
			Command::Wait { .. } => "wait",
			Command::WaitIdle { .. } => "waitIdle",
			Command::Screenshot { .. } => "screenshot",
			Command::Locate { .. } => "locate",
			Command::Expect { .. } => "expect",
			Command::Resize { .. } => "resize",
			Command::Status {} => "status",
		}
	}
}

/// An element found by a `locate` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Located {
	#[serde(rename = "box")]
	pub bounds: Rectangle,
	pub center: DVec2,
	/// The start of the element's text, with its whitespace collapsed.
	pub text: String,
	/// What the element holds if it is a field, or else what the first field inside it holds.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
	pub url: String,
	pub size: UVec2,
	pub pointer: DVec2,
	/// Where the document viewport is on the page, if one is open.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub viewport: Option<Rectangle>,
	pub held_keys: Vec<String>,
	pub held_buttons: Vec<MouseButton>,
	pub crashed: bool,
}

/// What came of one command.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResult {
	/// The `type` of the command this reports on.
	#[serde(rename = "type")]
	pub command: String,
	pub ok: bool,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub error: Option<String>,
	/// Where a screenshot or filmstrip was written.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub path: Option<PathBuf>,
	/// The pointer's position on the page in each frame of a filmstrip, in the order the frames are laid out.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub frames: Option<Vec<DVec2>>,
	/// Whether the editor stopped redrawing before the timeout.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub stable: Option<bool>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub located: Option<Vec<Located>>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub status: Option<SessionStatus>,
	/// The errors the page logged since the previous command, or for the first, since it loaded.
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub console_errors: Vec<String>,
}

impl CommandResult {
	pub fn succeeded(command: &Command) -> Self {
		Self {
			command: command.name().to_string(),
			ok: true,
			..Default::default()
		}
	}

	pub fn failed(command: &Command, error: String) -> Self {
		Self {
			command: command.name().to_string(),
			ok: false,
			error: Some(error),
			..Default::default()
		}
	}
}

/// Reads the steps of a scenario file, which holds one command per line written as JSON, leaving out blank lines and `//` comments.
pub fn parse_scenario(text: &str) -> Result<Vec<Command>, String> {
	text.lines()
		.enumerate()
		.filter(|(_, line)| {
			let line = line.trim();
			!line.is_empty() && !line.starts_with("//")
		})
		.map(|(index, line)| serde_json::from_str(line).map_err(|error| format!("Line {}: {error}", index + 1)))
		.collect()
}

/// A message from a client to an editor host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ClientMessage {
	/// Attaches to the session, which the host refuses unless the protocol version matches and the token is the host's own.
	Attach { version: u32, token: String },

	/// Performs commands in order, stopping at the first that fails, since the later ones were written assuming it succeeded.
	/// With `fail_on_console_errors`, a command also fails if the page has logged an error since the previous one.
	Perform {
		id: u64,
		commands: Vec<Command>,
		#[serde(default)]
		fail_on_console_errors: bool,
	},

	/// Ends the session and closes the editor.
	Shutdown,
}

/// A message from an editor host to a client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum HostMessage {
	/// Accepts a connection, naming the host's process so a client can tell it apart from one that has replaced it.
	Attached { version: u32, process_id: u32 },

	/// Turns away a connection, after which the host closes it.
	Refused { reason: String },

	/// Reports on each command performed, in order.
	Results { id: u64, results: Vec<CommandResult> },

	/// Confirms a shutdown, after which the connection closes once the host has exited.
	ShuttingDown,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn scenario_steps_parse() {
		let steps = parse_scenario(
			r#"// Draw a rectangle
				{ "type": "resize", "size": [1600, 1000] }
				{ "type": "press", "key": "KeyM" }

				// Drag out the rectangle
				{ "type": "drag", "from": [200, 150], "to": [500, 350], "space": "viewport", "filmstrip": { "path": "drag.png", "frames": 4 } }
				{ "type": "waitIdle" }
				{ "type": "screenshot", "path": "rectangle.png", "region": "viewport" }
				{ "type": "status" }
			"#,
		)
		.unwrap();

		assert_eq!(steps.len(), 6);
		assert_eq!(steps[0], Command::Resize { size: UVec2::new(1600, 1000) });
		assert!(
			matches!(&steps[2], Command::Drag { to, space: Some(Space::Viewport), filmstrip: Some(Filmstrip { frames: Some(frames), .. }), .. } if *to == DVec2::new(500., 350.) && frames.get() == 4)
		);
		assert_eq!(steps[5], Command::Status {});
	}

	#[test]
	fn scenario_mistakes_name_their_line() {
		let error = parse_scenario("// A mistake on the third line\n{ \"type\": \"status\" }\n{ \"type\": \"teleport\" }").unwrap_err();
		assert!(error.starts_with("Line 3:"));
	}

	#[test]
	fn qa_fields_parse() {
		let steps = parse_scenario("{ \"type\": \"expect\", \"data\": \"layer\", \"count\": 0 }\n{ \"type\": \"expect\", \"value\": \"500.26\" }").unwrap();
		assert!(matches!(steps[0], Command::Expect { count: Some(0), .. }));
		assert!(matches!(&steps[1], Command::Expect { value: Some(value), count: None, .. } if value == "500.26"));

		// Clients that predate the option leave it out
		let perform: ClientMessage = serde_json::from_str(r#"{ "type": "perform", "id": 1, "commands": [] }"#).unwrap();
		assert!(matches!(perform, ClientMessage::Perform { fail_on_console_errors: false, .. }));
	}

	#[test]
	fn mistakes_are_refused() {
		// A misspelled field
		assert!(serde_json::from_str::<Command>(r#"{ "type": "move", "to": [1, 2], "stpes": 3 }"#).is_err());
		// A count that must be a positive integer
		assert!(serde_json::from_str::<Command>(r#"{ "type": "click", "at": [1, 2], "count": 0 }"#).is_err());
		// An unknown command
		assert!(serde_json::from_str::<Command>(r#"{ "type": "teleport", "to": [1, 2] }"#).is_err());
		// A field on a command that takes none
		assert!(serde_json::from_str::<Command>(r#"{ "type": "status", "milliseconds": 5 }"#).is_err());
	}

	#[test]
	fn names_match_the_type_field() {
		let commands = [Command::KeyDown { key: "Shift".into() }, Command::WaitIdle { timeout: None }, Command::Status {}];

		for command in commands {
			let json = serde_json::to_value(&command).unwrap();
			assert_eq!(json["type"], command.name());
		}
	}
}
