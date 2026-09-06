use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, KeyLocation, ModifiersState, PhysicalKey, SmolStr};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InputEvent {
	pub(crate) kind: InputEventKind,
	pub(crate) modifiers: Modifiers,
}

impl InputEvent {
	pub fn pointer() -> PointerInputEventBuilder {
		PointerInputEventBuilder { position: UnknownPosition }
	}

	pub fn key(event: &KeyEvent) -> InputEventBuilder {
		let action = match (event.state, event.repeat) {
			(ElementState::Pressed, false) => KeyAction::Press,
			(ElementState::Pressed, true) => KeyAction::Repeat,
			(ElementState::Released, _) => KeyAction::Release,
		};
		InputEventBuilder::new(InputEventKind::Key {
			key: event.logical_key.clone(),
			key_without_modifiers: event.key_without_modifiers.clone(),
			physical_key: event.physical_key,
			location: event.location,
			text: event.text_with_all_modifiers.clone(),
			action,
		})
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MouseButton {
	Left,
	Right,
	Middle,
	Unknown,
}

impl From<winit::event::MouseButton> for MouseButton {
	fn from(button: winit::event::MouseButton) -> Self {
		match button {
			winit::event::MouseButton::Left => Self::Left,
			winit::event::MouseButton::Right => Self::Right,
			winit::event::MouseButton::Middle => Self::Middle,
			_ => Self::Unknown,
		}
	}
}

impl From<winit::event::ButtonSource> for MouseButton {
	fn from(button: winit::event::ButtonSource) -> Self {
		button.mouse_button().map_or(Self::Unknown, Self::from)
	}
}

#[must_use]
#[derive(Clone, Debug)]
pub struct PointerInputEventBuilder<P = UnknownPosition> {
	position: P,
}

impl PointerInputEventBuilder<UnknownPosition> {
	pub fn position(self, position: PhysicalPosition<f64>) -> PointerInputEventBuilder<PhysicalPosition<f64>> {
		PointerInputEventBuilder { position }
	}
}

impl PointerInputEventBuilder<PhysicalPosition<f64>> {
	pub fn moved(self) -> InputEventBuilder {
		self.finish(PointerAction::Move)
	}

	pub fn entered(self) -> InputEventBuilder {
		self.finish(PointerAction::Enter)
	}
}

impl<P: PointerPosition> PointerInputEventBuilder<P> {
	pub fn exited(self) -> InputEventBuilder {
		self.finish(PointerAction::Exit)
	}

	pub fn pressed(self, button: impl Into<MouseButton>, count: u32) -> InputEventBuilder {
		self.finish(PointerAction::Press { button: button.into(), count })
	}

	pub fn released(self, button: impl Into<MouseButton>, count: u32) -> InputEventBuilder {
		self.finish(PointerAction::Release { button: button.into(), count })
	}

	pub fn scrolled_lines(self, x: f64, y: f64) -> InputEventBuilder {
		self.finish(PointerAction::ScrollLines { x, y })
	}

	pub fn scrolled_pixels(self, x: f64, y: f64) -> InputEventBuilder {
		self.finish(PointerAction::ScrollPixels { x, y })
	}

	pub fn zoomed(self, delta: f64) -> InputEventBuilder {
		self.finish(PointerAction::Zoom(delta))
	}

	fn finish(self, action: PointerAction) -> InputEventBuilder {
		InputEventBuilder::new(InputEventKind::Pointer {
			position: self.position.position(),
			action,
		})
	}
}

#[must_use]
#[derive(Clone, Debug)]
pub struct InputEventBuilder {
	kind: InputEventKind,
	modifiers: Modifiers,
}

impl InputEventBuilder {
	fn new(kind: InputEventKind) -> Self {
		Self {
			kind,
			modifiers: Modifiers::default(),
		}
	}

	pub fn shift(mut self, on: bool) -> Self {
		self.modifiers.shift = on;
		self
	}

	pub fn control(mut self, on: bool) -> Self {
		self.modifiers.control = on;
		self
	}

	pub fn alt(mut self, on: bool) -> Self {
		self.modifiers.alt = on;
		self
	}

	pub fn alt_graph(mut self, on: bool) -> Self {
		self.modifiers.alt_graph = on;
		self
	}

	pub fn meta(mut self, on: bool) -> Self {
		self.modifiers.meta = on;
		self
	}

	pub fn caps_lock(mut self, on: bool) -> Self {
		self.modifiers.caps_lock = on;
		self
	}

	pub fn num_lock(mut self, on: bool) -> Self {
		self.modifiers.num_lock = on;
		self
	}

	pub fn modifiers(mut self, modifiers: ModifiersState) -> Self {
		self.modifiers.shift = modifiers.shift_key();
		self.modifiers.control = modifiers.control_key();
		self.modifiers.alt = modifiers.alt_key();
		self.modifiers.meta = modifiers.meta_key();
		self
	}

	pub fn build(self) -> InputEvent {
		InputEvent {
			kind: self.kind,
			modifiers: self.modifiers,
		}
	}
}

#[expect(private_bounds)]
pub trait PointerPosition: OptionalPosition {}
impl PointerPosition for UnknownPosition {}
impl PointerPosition for PhysicalPosition<f64> {}

#[derive(Clone, Copy, Debug)]
pub struct UnknownPosition;

trait OptionalPosition {
	fn position(self) -> Option<PhysicalPosition<f64>>;
}
impl OptionalPosition for UnknownPosition {
	fn position(self) -> Option<PhysicalPosition<f64>> {
		None
	}
}
impl OptionalPosition for PhysicalPosition<f64> {
	fn position(self) -> Option<PhysicalPosition<f64>> {
		Some(self)
	}
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum InputEventKind {
	Pointer {
		position: Option<PhysicalPosition<f64>>,
		action: PointerAction,
	},
	Key {
		key: Key,
		key_without_modifiers: Key,
		physical_key: PhysicalKey,
		location: KeyLocation,
		text: Option<SmolStr>,
		action: KeyAction,
	},
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum PointerAction {
	Move,
	Enter,
	Exit,
	Press { button: MouseButton, count: u32 },
	Release { button: MouseButton, count: u32 },
	ScrollLines { x: f64, y: f64 },
	ScrollPixels { x: f64, y: f64 },
	Zoom(f64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum KeyAction {
	Press,
	Repeat,
	Release,
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct Modifiers {
	pub(crate) shift: bool,
	pub(crate) control: bool,
	pub(crate) alt: bool,
	pub(crate) alt_graph: bool,
	pub(crate) meta: bool,
	pub(crate) caps_lock: bool,
	pub(crate) num_lock: bool,
}
