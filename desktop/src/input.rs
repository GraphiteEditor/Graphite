use winit::dpi::PhysicalPosition;
use winit::event::{ButtonSource, ElementState, MouseButton, MouseScrollDelta, PointerSource, TabletToolData, TabletToolKind, WindowEvent};
use winit::keyboard::ModifiersState;

use crate::ui::{PINCH_ZOOM_SPEED, SCROLL_LINE_HEIGHT, SCROLL_LINE_WIDTH, SCROLL_SPEED_X, SCROLL_SPEED_Y};
use crate::wrapper::messages::{DesktopWrapperMessage, InputMessage, ModifierKeys, MouseKeys, PointerState, ScrollDelta};

pub(crate) struct InputState {
	viewport_info: Option<ViewportInfo>,
	pointer_locked: bool,
	modifier_keys: ModifierKeys,
	pointer_position: PhysicalPosition<f64>,
	pointer_keys: MouseKeys,
	ui_capture: bool,
}

pub(crate) enum InputAction {
	Ui(WindowEvent),
	Editor(DesktopWrapperMessage),
}

impl InputAction {
	fn editor(message: InputMessage) -> Self {
		Self::Editor(DesktopWrapperMessage::Input(message))
	}
}

impl InputState {
	pub(crate) fn new() -> Self {
		Self {
			viewport_info: None,
			pointer_locked: false,
			modifier_keys: ModifierKeys::empty(),
			pointer_position: PhysicalPosition::default(),
			pointer_keys: MouseKeys::empty(),
			ui_capture: true,
		}
	}

	pub(crate) fn set_viewport_info(&mut self, x: f64, y: f64, width: f64, height: f64, scale: f64) {
		self.viewport_info = Some(ViewportInfo { x, y, width, height, scale });
	}

	pub(crate) fn set_pointer_locked(&mut self, locked: bool) {
		self.pointer_locked = locked;
	}

	pub(crate) fn pointer_position(&self) -> PhysicalPosition<f64> {
		self.pointer_position
	}

	pub(crate) fn process(&mut self, event: &WindowEvent) -> Vec<InputAction> {
		match event {
			WindowEvent::PointerMoved { position, source, .. } => {
				self.pointer_position = *position;

				let PointerSource::TabletTool { kind, data } = source else {
					return vec![InputAction::Ui(event.clone())];
				};
				let ui_capture = if self.pointer_keys.is_empty() {
					self.pointer_locked || !self.in_viewport(*position)
				} else {
					self.ui_capture
				};
				if ui_capture {
					return vec![InputAction::Ui(event.clone())];
				}

				vec![InputAction::editor(InputMessage::PointerMove {
					editor_mouse_state: self.tablet_pointer_state(kind, data),
					modifier_keys: self.modifier_keys,
				})]
			}
			WindowEvent::PointerEntered { position, .. } | WindowEvent::PointerLeft { position: Some(position), .. } => {
				self.pointer_position = *position;
				vec![InputAction::Ui(event.clone())]
			}
			WindowEvent::PointerButton { state, button, position, .. } => {
				self.pointer_position = *position;

				let tablet = matches!(button, ButtonSource::TabletTool { .. });

				// Stroke keeps capture decided from first button press until all buttons are released.
				if state.is_pressed() && self.pointer_keys.is_empty() {
					self.ui_capture = self.pointer_locked || !tablet || !self.in_viewport(*position);
				}

				let mouse_button = button.clone().mouse_button();
				let keys = match mouse_button {
					MouseButton::Left => MouseKeys::LEFT,
					MouseButton::Right => MouseKeys::RIGHT,
					MouseButton::Middle => MouseKeys::MIDDLE,
					MouseButton::Back => MouseKeys::BACK,
					MouseButton::Forward => MouseKeys::FORWARD,
					MouseButton::Other(_) => MouseKeys::NONE,
				};
				match state {
					ElementState::Pressed => self.pointer_keys.insert(keys),
					ElementState::Released => self.pointer_keys.remove(keys),
				}

				let back_or_forward = matches!(mouse_button, MouseButton::Back | MouseButton::Forward);
				if self.pointer_locked || !(back_or_forward || (tablet && !self.ui_capture)) {
					return vec![InputAction::Ui(event.clone())];
				}

				let editor_mouse_state = match button {
					ButtonSource::TabletTool { kind, data, .. } => self.tablet_pointer_state(kind, data),
					_ => self.pointer_state(),
				};
				let modifier_keys = self.modifier_keys;
				vec![InputAction::editor(match state {
					ElementState::Pressed => InputMessage::PointerDown { editor_mouse_state, modifier_keys },
					ElementState::Released => InputMessage::PointerUp { editor_mouse_state, modifier_keys },
				})]
			}
			WindowEvent::MouseWheel { delta, .. } => {
				if self.pointer_locked || !self.in_viewport(self.pointer_position) {
					return vec![InputAction::Ui(event.clone())];
				}

				let (x, y) = match delta {
					MouseScrollDelta::LineDelta(x, y) => (f64::from(*x) * SCROLL_LINE_WIDTH, f64::from(*y) * SCROLL_LINE_HEIGHT),
					MouseScrollDelta::PixelDelta(position) => (position.x, position.y),
				};

				let scroll_delta = ScrollDelta::new(-x * SCROLL_SPEED_X, -y * SCROLL_SPEED_Y, 0.);

				vec![InputAction::editor(InputMessage::WheelScroll {
					editor_mouse_state: PointerState { scroll_delta, ..self.pointer_state() },
					modifier_keys: self.modifier_keys,
				})]
			}
			WindowEvent::PinchGesture { delta, .. } => {
				if self.pointer_locked || !self.in_viewport(self.pointer_position) || !delta.is_normal() {
					return vec![InputAction::Ui(event.clone())];
				}

				// TODO: This is a temporary solution to handle pinch gestures, we should handle pinch gestures editor-side instead.
				let scroll_delta = ScrollDelta::new(0., -delta * PINCH_ZOOM_SPEED, 0.);
				vec![InputAction::editor(InputMessage::WheelScroll {
					editor_mouse_state: PointerState { scroll_delta, ..self.pointer_state() },
					modifier_keys: self.modifier_keys | ModifierKeys::CONTROL,
				})]
			}
			WindowEvent::ModifiersChanged(modifiers) => {
				self.modifier_keys = to_modifier_keys(modifiers.state());
				vec![InputAction::Ui(event.clone())]
			}
			_ => vec![InputAction::Ui(event.clone())],
		}
	}

	fn scale(&self) -> f64 {
		self.viewport_info.as_ref().map_or(1., |info| info.scale)
	}

	fn in_viewport(&self, position: PhysicalPosition<f64>) -> bool {
		self.viewport_info.as_ref().is_some_and(|info| info.contains(position))
	}

	fn pointer_state(&self) -> PointerState {
		PointerState {
			editor_position: (self.pointer_position.x / self.scale(), self.pointer_position.y / self.scale()).into(),
			mouse_keys: self.pointer_keys,
			..Default::default()
		}
	}

	fn tablet_pointer_state(&self, kind: &TabletToolKind, data: &TabletToolData) -> PointerState {
		PointerState {
			pressure: data.force.map(|force| force.normalized(None)),
			tilt: data.clone().tilt().map(|tilt| (f64::from(tilt.x), f64::from(tilt.y)).into()),
			twist: data.twist.map(f64::from),
			wheel: data.tangential_force.map(f64::from),
			eraser: matches!(kind, TabletToolKind::Eraser),
			..self.pointer_state()
		}
	}
}

struct ViewportInfo {
	x: f64,
	y: f64,
	width: f64,
	height: f64,
	scale: f64,
}

impl ViewportInfo {
	fn contains(&self, position: PhysicalPosition<f64>) -> bool {
		position.x >= self.x && position.y >= self.y && position.x <= self.x + self.width && position.y <= self.y + self.height
	}
}

fn to_modifier_keys(modifiers: ModifiersState) -> ModifierKeys {
	let mut keys = ModifierKeys::empty();
	keys.set(ModifierKeys::SHIFT, modifiers.shift_key());
	keys.set(ModifierKeys::CONTROL, modifiers.control_key());
	keys.set(ModifierKeys::ALT, modifiers.alt_key());
	keys.set(ModifierKeys::META_OR_COMMAND, modifiers.meta_key());
	keys
}
