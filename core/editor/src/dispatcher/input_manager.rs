use crate::tools::ToolType;

use super::{
	events::{Event, Key, MouseState},
	Action,
};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct KeyState {
	depressed: bool,
	// time of last press
	// mod keys held down while pressing
	// …
}

#[derive(Debug, Default)]
pub struct InputPreprocessor {
	mouse_keys: MouseState,
	keyboard: HashMap<Key, KeyState>,
	//key_translation: HashMap<Key, VirtualInputAction>,
	pub mouse_state: MouseState,
}

impl InputPreprocessor {
	pub fn handle_user_input(&mut self, event: Event) -> Vec<Event> {
		// clean user input and if possible reconstruct it
		// store the changes in the keyboard if it is a key event
		// translate the key events to VirtualKeyActions and return them
		// transform canvas coordinates to document coordinates
		// Last pressed key
		// respect t {
		match event {
			Event::MouseMove(pos) => self.mouse_state.position = pos,
			Event::LmbDown(mouse_state) | Event::RmbDown(mouse_state) | Event::MmbDown(mouse_state) | Event::LmbUp(mouse_state) | Event::RmbUp(mouse_state) | Event::MmbUp(mouse_state) => {
				self.mouse_state = mouse_state;
			}
			_ => (),
		}
		vec![event]
	}
}

macro_rules! key {
	($path:expr, $action:expr, $k:ident, ($($s:ident),*)) => {
		($path, $action, Event::KeyDown(Key::$k), &[$(Key::$s,)*])
	};
	($action:expr, $k:ident, ($($s:ident),*)) => {
		key!("*", $action, $k, ($($s),*))
	};
	($path:expr, $action:expr, $k:ident) => {
		key!($path, $action, $k, ())
	};
	($action:expr, $k:ident) => {
		key!("*", $action, $k, ())
	};
}

const DEFAULT_MAPPING: &[(&str, &str, Event, &[Key])] = &[
	key!("Undo", KeyZ, (KeyControl)),
	key!("*", "Redo", KeyZ, (KeyControl, KeyShift)),
	key!("Redo", KeyZ, (KeyControl, KeyCaps)),
	key!("Center", KeyAlt),
];

#[derive(Debug, Default)]
pub struct InputMapper {}

impl InputMapper {
	pub fn translate_event(&mut self, event: Event, input: &InputPreprocessor, _actions: &[(String, Action)]) -> Vec<Action> {
		vec![self.dummy_translation(event, input)]
	}
	fn dummy_translation(&mut self, event: Event, input: &InputPreprocessor) -> Action {
		match event {
			Event::SelectTool(tool_name) => Action::SelectTool(tool_name),
			Event::SelectPrimaryColor(color) => Action::SelectPrimaryColor(color),
			Event::SelectSecondaryColor(color) => Action::SelectSecondaryColor(color),
			Event::SwapColors => Action::SwapColors,
			Event::ResetColors => Action::ResetColors,
			Event::MouseMove(_) => Action::MouseMove,
			Event::ToggleLayerVisibility(path) => Action::ToggleLayerVisibility(path),
			Event::LmbDown(_) => Action::LmbDown,
			Event::LmbUp(_) => Action::LmbUp,
			Event::RmbDown(_) => Action::RmbDown,
			Event::RmbUp(_) => Action::RmbUp,
			Event::MmbDown(_) => Action::MmbDown,
			Event::MmbUp(_) => Action::MmbUp,
			Event::AmbiguousMouseUp(_) | Event::AmbiguousMouseDown(_) => Action::NoOp,
			Event::Action(a) => a,

			event => self.translate_key(event, input),
		}
	}

	fn translate_key(&self, event: Event, _input: &InputPreprocessor) -> Action {
		use Key::*;
		match event {
			Event::KeyUp(key) => match key {
				KeyAlt => Action::UnCenter,
				KeyShift | KeyCaps => Action::UnlockAspectRatio,
				_ => Action::NoOp,
			},
			Event::KeyDown(key) => match key {
				Key1 => Action::LogInfo,
				Key2 => Action::LogDebug,
				Key3 => Action::LogTrace,
				KeyV => Action::SelectTool(ToolType::Select),
				KeyL => Action::SelectTool(ToolType::Line),
				KeyP => Action::SelectTool(ToolType::Pen),
				KeyM => Action::SelectTool(ToolType::Rectangle),
				KeyY => Action::SelectTool(ToolType::Shape),
				KeyE => Action::SelectTool(ToolType::Ellipse),
				KeyX => Action::SwapColors,
				KeyZ => Action::Undo,
				KeyEnter => Action::Confirm,
				KeyAlt => Action::Center,
				KeyShift | KeyCaps => Action::LockAspectRatio,
				_ => Action::NoOp,
			},
			_ => todo!("Implement layer handling"),
		}
	}
}
