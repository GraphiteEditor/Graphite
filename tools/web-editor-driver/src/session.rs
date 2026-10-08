use glam::{DVec2, UVec2};
use graphite_editor_control_protocol::{Command, CommandResult, Filmstrip, Located, MouseButton, Rectangle, SessionStatus, Space};
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::browser::Browser;
use crate::contact_sheet::{Frame, write_contact_sheet};
use crate::paths::output_path;

const VIEWPORT_SELECTOR: &str = "[data-viewport]";
const EDITOR_READY_SELECTOR: &str = "[data-workspace]";
const EDITOR_READY_TIMEOUT: Duration = Duration::from_secs(120);
const POINTER_STEP_DISTANCE: f64 = 8.;
const WHEEL_NOTCH: f64 = 100.;
// Bounds on how many frames a move or scroll takes, however far it goes
const MAX_POINTER_STEPS: u32 = 1000;
const MAX_WHEEL_NOTCHES: f64 = 100.;
const FRAME_TIMEOUT: Duration = Duration::from_secs(1);
const IDLE_TIMEOUT: Duration = Duration::from_secs(5);
const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(50);
const IDLE_CAPTURE_COUNT: u32 = 3;
const IDLE_MINIMUM_SPAN: Duration = Duration::from_millis(200);
const DEFAULT_FILMSTRIP_FRAMES: u32 = 8;
const LOCATED_TEXT_LENGTH: usize = 80;

#[derive(Debug, Clone)]
pub struct SessionOptions {
	pub url: String,
	pub size: UVec2,
	pub scale: f64,
	pub headed: bool,
}

struct HeldKey {
	/// The key as `held_key_name` gives it.
	name: String,
	/// The key as it was pressed, which must also release it.
	pressed_as: String,
}

// A modifier counts as one held key whichever side is named, since the page sees the same modifier state either way
fn held_key_name(key: &str) -> &str {
	for modifier in ["Shift", "Control", "Alt", "Meta"] {
		if let Some(side) = key.strip_prefix(modifier)
			&& (side == "Left" || side == "Right")
		{
			return modifier;
		}
	}
	key
}

// Splits a combination such as `Control+KeyZ` into its keys, where a final `+` is the plus key itself rather than a separator
fn split_key_combination(combination: &str) -> Vec<&str> {
	let mut keys = Vec::new();
	let mut start = 0;

	for (index, character) in combination.char_indices() {
		if character == '+' && index + 1 < combination.len() {
			keys.push(&combination[start..index]);
			start = index + 1;
		}
	}
	keys.push(&combination[start..]);

	keys
}

// Trims an area to the page as the screenshot will, so the crosshair is measured from the corner actually captured
fn clamp_to_page(area: Rectangle, page_size: UVec2) -> Result<Rectangle, String> {
	let page_size = page_size.as_dvec2();
	let top_left = DVec2::new(area.x, area.y).clamp(DVec2::ZERO, page_size);
	let bottom_right = DVec2::new(area.x + area.width, area.y + area.height).clamp(DVec2::ZERO, page_size);
	if bottom_right.x <= top_left.x || bottom_right.y <= top_left.y {
		return Err("The area to capture lies outside the page".to_string());
	}

	let size = bottom_right - top_left;
	Ok(Rectangle {
		x: top_left.x,
		y: top_left.y,
		width: size.x,
		height: size.y,
	})
}

/// One browser page holding the editor, driven only through the input a person could give it.
pub struct EditorSession {
	browser: Arc<Browser>,
	url: String,
	scale: f64,
	size: UVec2,
	pointer: DVec2,
	held_keys: Vec<HeldKey>,
	held_buttons: Vec<MouseButton>,
	crashed: bool,
}

impl EditorSession {
	pub fn launch(browser: Arc<Browser>, options: SessionOptions) -> Result<Self, String> {
		browser.launch(&options.url, options.size, options.scale, options.headed, EDITOR_READY_SELECTOR, EDITOR_READY_TIMEOUT)?;

		let session = Self {
			browser,
			url: options.url,
			scale: options.scale,
			size: options.size,
			pointer: DVec2::ZERO,
			held_keys: Vec::new(),
			held_buttons: Vec::new(),
			crashed: false,
		};
		session.wait_idle(IDLE_TIMEOUT)?;

		Ok(session)
	}

	pub fn perform(&mut self, command: &Command) -> CommandResult {
		self.dispatch(command).unwrap_or_else(|error| CommandResult::failed(command, error))
	}

	fn dispatch(&mut self, command: &Command) -> Result<CommandResult, String> {
		let mut result = CommandResult::succeeded(command);

		match command {
			Command::Move { to, space, steps } => {
				let steps = explicit_step_count(*steps)?;
				let to = self.to_page(*to, *space)?;
				self.move_pointer(to, steps, None)?;
			}
			Command::Down { button } => self.press_button(button.unwrap_or_default(), 1)?,
			Command::Up { button } => self.release_button(button.unwrap_or_default(), 1)?,
			Command::Click { at, space, button, count, keys } => {
				let button = button.unwrap_or_default();
				self.require_button_not_held(button)?;

				let at = self.to_page(*at, *space)?;
				self.move_pointer(at, None, None)?;

				let mut pressed_keys = Vec::new();
				let clicked = self.hold_keys(keys.as_deref().unwrap_or_default(), &mut pressed_keys).and_then(|()| {
					for click_count in 1..=count.map_or(1, |count| count.get()) {
						self.press_button(button, click_count)?;
						self.release_button(button, click_count)?;
					}
					Ok(())
				});
				let released = self.release_pressed(button, &pressed_keys);
				clicked.and(released)?;
			}
			Command::Drag {
				from,
				to,
				space,
				button,
				keys,
				steps,
				release,
				filmstrip,
			} => {
				let button = button.unwrap_or_default();
				self.require_button_not_held(button)?;
				let steps = explicit_step_count(*steps)?;

				let from = self.to_page(*from, *space)?;
				let to = self.to_page(*to, *space)?;
				self.move_pointer(from, None, None)?;

				let mut pressed_keys = Vec::new();
				let dragged = self
					.hold_keys(keys.as_deref().unwrap_or_default(), &mut pressed_keys)
					.and_then(|()| self.press_button(button, 1))
					.and_then(|()| self.move_pointer(to, steps, filmstrip.as_ref()));

				// Whatever the drag pressed is released at the end or on failure, but stays held with `release: false` once the drag succeeds
				let released = if dragged.is_err() || *release != Some(false) {
					self.release_pressed(button, &pressed_keys)
				} else {
					Ok(())
				};
				let frames = dragged?;
				released?;

				if let Some(filmstrip) = filmstrip {
					let path = output_path(&filmstrip.path);
					write_contact_sheet(&path, &frames, self.scale)?;
					result.path = Some(path);
					result.frames = Some(frames.iter().map(|frame| frame.page_pointer).collect());
				}
			}
			Command::KeyDown { key } => self.hold_keys(std::slice::from_ref(key), &mut Vec::new())?,
			Command::KeyUp { key } => self.release_keys(std::slice::from_ref(key))?,
			Command::Press { key } => {
				let keys = split_key_combination(key);
				let Some((final_key, modifiers)) = keys.split_last() else { return Ok(result) };
				let modifiers = modifiers.iter().map(|modifier| modifier.to_string()).collect::<Vec<_>>();

				let mut pressed_keys = Vec::new();
				let pressed = self.hold_keys(&modifiers, &mut pressed_keys).and_then(|()| {
					self.browser.key_down(final_key)?;
					self.browser.key_up(final_key)?;

					// Tapping a key leaves it up, even one that was held before
					let name = held_key_name(final_key);
					self.held_keys.retain(|held| held.name != name);
					Ok(())
				});
				let released = self.release_keys(&pressed_keys);
				pressed.and(released)?;
			}
			Command::Type { text } => self.browser.type_text(text)?,
			Command::Scroll { delta, at, space } => {
				if let Some(at) = at {
					let at = self.to_page(*at, *space)?;
					self.move_pointer(at, None, None)?;
				}

				// A wheel turns in notches, so a large delta is spread over several, one per frame
				let notches = (delta.abs().max_element() / WHEEL_NOTCH).ceil().clamp(1., MAX_WHEEL_NOTCHES);
				for _ in 0..notches as u32 {
					self.browser.wheel(*delta / notches)?;
					self.next_frame()?;
				}
			}
			Command::Wait { milliseconds } => std::thread::sleep(Duration::from_millis(*milliseconds)),
			Command::WaitIdle { timeout } => {
				let timeout = timeout.map_or(IDLE_TIMEOUT, Duration::from_millis);
				result.stable = Some(self.wait_idle(timeout)?);
			}
			Command::Screenshot { path, cursor, region, clip } => {
				let path = output_path(path);
				let frame = self.capture_frame(region.as_deref(), *clip)?;

				if *cursor == Some(true) {
					write_contact_sheet(&path, &[frame], self.scale)?;
				} else {
					write_file(&path, &frame.image)?;
				}
				result.path = Some(path);
			}
			Command::Locate { data, text } => result.located = Some(self.locate(data.as_deref(), text.as_deref())?),
			Command::Resize { size } => {
				if size.min_element() == 0 {
					return Err("The page's width and height must be at least one pixel".to_string());
				}
				self.browser.resize(*size)?;
				self.size = *size;
			}
			Command::Status {} => result.status = Some(self.status()?),
		}

		Ok(result)
	}

	fn status(&mut self) -> Result<SessionStatus, String> {
		let events = self.browser.take_events()?;
		self.crashed |= events.crashed;

		// A crashed page answers no queries
		let viewport = if self.crashed { None } else { self.region_box(Some("viewport"))? };

		Ok(SessionStatus {
			url: self.url.clone(),
			size: self.size,
			pointer: self.pointer,
			viewport,
			held_keys: self.held_keys.iter().map(|held| held.pressed_as.clone()).collect(),
			held_buttons: self.held_buttons.clone(),
			crashed: self.crashed,
			console_errors: events.console_errors,
		})
	}

	fn to_page(&self, point: DVec2, space: Option<Space>) -> Result<DVec2, String> {
		if space != Some(Space::Viewport) {
			return Ok(point);
		}

		let viewport = self.region_box(Some("viewport"))?.ok_or("No document viewport is on screen to measure the point from")?;
		Ok(point + DVec2::new(viewport.x, viewport.y))
	}

	fn region_box(&self, region: Option<&str>) -> Result<Option<Rectangle>, String> {
		let Some(region) = region.filter(|region| *region != "page") else { return Ok(None) };

		let selector = if region == "viewport" { VIEWPORT_SELECTOR } else { region };
		let bounds = self.browser.bounding_box(selector)?;
		if bounds.is_none() && region != "viewport" {
			return Err(format!("No element matches the region \"{region}\""));
		}

		Ok(bounds)
	}

	// Captures an area of the page along with where the pointer sits within it
	fn capture_frame(&self, region: Option<&str>, clip: Option<Rectangle>) -> Result<Frame, String> {
		let area = match clip {
			Some(clip) => Some(clip),
			None if region == Some("viewport") => Some(self.region_box(region)?.ok_or("No document viewport is on screen to capture")?),
			None => self.region_box(region)?,
		};
		let clip = area.map(|area| clamp_to_page(area, self.size)).transpose()?;
		let origin = clip.map_or(DVec2::ZERO, |clip| DVec2::new(clip.x, clip.y));

		Ok(Frame {
			image: self.browser.screenshot(clip)?,
			pointer: self.pointer - origin,
			page_pointer: self.pointer,
		})
	}

	fn next_frame(&self) -> Result<(), String> {
		self.browser.next_frame(FRAME_TIMEOUT)
	}

	// A real pointer only lands on whole device pixels
	fn snap_to_device_pixels(&self, point: DVec2) -> DVec2 {
		(point * self.scale).round() / self.scale
	}

	// Moves one small step per rendered frame, as a real pointer does, optionally capturing frames spread evenly along the path
	fn move_pointer(&mut self, to: DVec2, steps: Option<u32>, filmstrip: Option<&Filmstrip>) -> Result<Vec<Frame>, String> {
		let from = self.pointer;
		let step_count = steps.unwrap_or_else(|| ((from.distance(to) / POINTER_STEP_DISTANCE).ceil() as u32).clamp(1, MAX_POINTER_STEPS));
		let frame_count = filmstrip.map_or(0, |filmstrip| filmstrip.frames.map_or(DEFAULT_FILMSTRIP_FRAMES, |frames| frames.get()).min(step_count));
		let mut frames = Vec::new();

		for step in 1..=step_count {
			let next = self.snap_to_device_pixels(from.lerp(to, step as f64 / step_count as f64));
			self.browser.mouse_move(next)?;
			self.pointer = next;
			self.next_frame()?;

			let Some(filmstrip) = filmstrip else { continue };
			let frames_due = (step as u64 * frame_count as u64 / step_count as u64) as usize;
			if frames_due <= frames.len() {
				continue;
			}

			self.wait_idle(IDLE_TIMEOUT)?;
			frames.push(self.capture_frame(filmstrip.region.as_deref(), filmstrip.clip)?);
		}

		Ok(frames)
	}

	fn require_button_not_held(&self, button: MouseButton) -> Result<(), String> {
		if self.held_buttons.contains(&button) {
			return Err(format!("The {} button is already held, so release it with `up` first", button_name(button)));
		}
		Ok(())
	}

	fn press_button(&mut self, button: MouseButton, click_count: u32) -> Result<(), String> {
		self.browser.mouse_down(button, click_count)?;
		if !self.held_buttons.contains(&button) {
			self.held_buttons.push(button);
		}
		self.next_frame()
	}

	fn release_button(&mut self, button: MouseButton, click_count: u32) -> Result<(), String> {
		self.browser.mouse_up(button, click_count)?;
		self.held_buttons.retain(|held| *held != button);
		self.next_frame()
	}

	// Releases the button if it is still held, then the keys, attempting both even if the first fails
	fn release_pressed(&mut self, button: MouseButton, pressed_keys: &[String]) -> Result<(), String> {
		let button_released = if self.held_buttons.contains(&button) { self.release_button(button, 1) } else { Ok(()) };
		let keys_released = self.release_keys(pressed_keys);
		button_released.and(keys_released)
	}

	// Holds each key not already held, adding each one it presses to `pressed_keys` right away so a caller can release exactly those
	fn hold_keys(&mut self, keys: &[String], pressed_keys: &mut Vec<String>) -> Result<(), String> {
		let mut pressed_any = false;
		for key in keys {
			let name = held_key_name(key);
			if self.held_keys.iter().any(|held| held.name == name) {
				continue;
			}

			self.browser.key_down(key)?;
			self.held_keys.push(HeldKey {
				name: name.to_string(),
				pressed_as: key.clone(),
			});
			pressed_keys.push(key.clone());
			pressed_any = true;
		}

		if pressed_any { self.next_frame() } else { Ok(()) }
	}

	fn release_keys(&mut self, keys: &[String]) -> Result<(), String> {
		for key in keys.iter().rev() {
			let name = held_key_name(key);
			let pressed_as = self.held_keys.iter().find(|held| held.name == name).map_or(key.as_str(), |held| held.pressed_as.as_str());

			self.browser.key_up(pressed_as)?;
			self.held_keys.retain(|held| held.name != name);
		}

		if keys.is_empty() { Ok(()) } else { self.next_frame() }
	}

	// Waits until three consecutive captures of the page are identical and span at least 200 milliseconds, or returns false on timeout
	fn wait_idle(&self, timeout: Duration) -> Result<bool, String> {
		let deadline = Instant::now() + timeout;
		let mut previous: Option<Vec<u8>> = None;
		let mut identical_captures = 0;
		let mut identical_since = Instant::now();

		while Instant::now() < deadline {
			self.next_frame()?;
			let current = self.browser.screenshot(None)?;

			if previous.as_ref() == Some(&current) {
				identical_captures += 1;
			} else {
				previous = Some(current);
				identical_captures = 1;
				identical_since = Instant::now();
			}
			if identical_captures >= IDLE_CAPTURE_COUNT && identical_since.elapsed() >= IDLE_MINIMUM_SPAN {
				return Ok(true);
			}

			std::thread::sleep(IDLE_POLL_INTERVAL);
		}

		Ok(false)
	}

	fn locate(&self, data: Option<&str>, text: Option<&str>) -> Result<Vec<Located>, String> {
		if data.is_none() && text.is_none() {
			return Err("Locating needs a `data` attribute name or some visible `text`".to_string());
		}

		let located = self.browser.locate(data, text)?.into_iter().map(|element| {
			let bounds = element.bounds;
			let text = element.text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(LOCATED_TEXT_LENGTH).collect();
			let center = DVec2::new(bounds.x + bounds.width / 2., bounds.y + bounds.height / 2.);

			Located { bounds, center, text }
		});
		Ok(located.collect())
	}
}

fn button_name(button: MouseButton) -> &'static str {
	match button {
		MouseButton::Left => "left",
		MouseButton::Middle => "middle",
		MouseButton::Right => "right",
	}
}

// Refused rather than lowered, since taking fewer steps than asked would change the motion the caller described
fn explicit_step_count(steps: Option<NonZeroU32>) -> Result<Option<u32>, String> {
	match steps.map(NonZeroU32::get) {
		Some(steps) if steps > MAX_POINTER_STEPS => Err(format!("A move takes at most {MAX_POINTER_STEPS} steps")),
		steps => Ok(steps),
	}
}

pub fn write_file(path: &Path, contents: &[u8]) -> Result<(), String> {
	if let Some(parent) = path.parent() {
		std::fs::create_dir_all(parent).map_err(|error| format!("Failed to create the folder {}: {error}", parent.display()))?;
	}
	std::fs::write(path, contents).map_err(|error| format!("Failed to write {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn key_combinations_split_on_plus() {
		assert_eq!(split_key_combination("Control+KeyZ"), ["Control", "KeyZ"]);
		assert_eq!(split_key_combination("Control+Shift+KeyZ"), ["Control", "Shift", "KeyZ"]);
		assert_eq!(split_key_combination("Control++"), ["Control", "+"]);
		assert_eq!(split_key_combination("+"), ["+"]);
		assert_eq!(split_key_combination("KeyP"), ["KeyP"]);
	}

	#[test]
	fn sided_modifiers_are_held_as_one() {
		assert_eq!(held_key_name("ShiftLeft"), "Shift");
		assert_eq!(held_key_name("ControlRight"), "Control");
		assert_eq!(held_key_name("Shift"), "Shift");
		assert_eq!(held_key_name("KeyS"), "KeyS");
		assert_eq!(held_key_name("AltGraph"), "AltGraph");
	}

	#[test]
	fn captures_are_trimmed_to_the_page() {
		let page = UVec2::new(100, 50);
		let trimmed = clamp_to_page(
			Rectangle {
				x: -10.,
				y: 40.,
				width: 30.,
				height: 30.,
			},
			page,
		)
		.unwrap();
		assert_eq!(
			trimmed,
			Rectangle {
				x: 0.,
				y: 40.,
				width: 20.,
				height: 10.
			}
		);

		let outside = Rectangle {
			x: 200.,
			y: 0.,
			width: 10.,
			height: 10.,
		};
		assert!(clamp_to_page(outside, page).is_err());
	}
}
