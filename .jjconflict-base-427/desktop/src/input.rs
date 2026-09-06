use std::time::Instant;
use winit::dpi::PhysicalPosition;
use winit::event::{ButtonSource, ElementState, MouseButton, MouseScrollDelta, PointerSource, TabletToolData, TabletToolKind, WindowEvent};
use winit::keyboard::ModifiersState;

use crate::ui::{InputEvent, MULTICLICK_ALLOWED_TRAVEL, MULTICLICK_TIMEOUT, PINCH_ZOOM_SPEED, SCROLL_LINE_HEIGHT, SCROLL_LINE_WIDTH, SCROLL_SPEED_X, SCROLL_SPEED_Y};
use crate::wrapper::messages::{InputMessage, ModifierKeys, MouseKeys, PointerState, ScrollDelta};

pub(crate) struct InputState {
	start: Instant,
	viewport_info: Option<ViewportInfo>,
	pointer_lock_position: Option<PhysicalPosition<f64>>,
	modifiers: ModifiersState,
	pointer_position: PhysicalPosition<f64>,
	pointer_keys: MouseKeys,
	ui_capture: bool,
	click_tracker: ClickTracker,
}

impl InputState {
	pub(crate) fn new() -> Self {
		Self {
			start: Instant::now(),
			viewport_info: None,
			pointer_lock_position: None,
			modifiers: ModifiersState::default(),
			pointer_position: PhysicalPosition::default(),
			pointer_keys: MouseKeys::empty(),
			ui_capture: true,
			click_tracker: ClickTracker::default(),
		}
	}

	pub(crate) fn set_viewport_info(&mut self, x: f64, y: f64, width: f64, height: f64, scale: f64) {
		self.viewport_info = Some(ViewportInfo { x, y, width, height, scale });
	}

	pub(crate) fn lock_pointer(&mut self) {
		self.pointer_lock_position = Some(self.pointer_position);
	}

	pub(crate) fn unlock_pointer(&mut self) -> Option<PhysicalPosition<f64>> {
		let position = self.pointer_lock_position.take();
		if let Some(position) = position {
			self.pointer_position = position;
		}
		position
	}

	pub(crate) fn pointer_locked(&self) -> bool {
		self.pointer_lock_position.is_some()
	}

	pub(crate) fn modifiers(&self) -> ModifiersState {
		self.modifiers
	}

	pub(crate) fn process(&mut self, event: &WindowEvent, mut editor_callback: impl FnMut(InputMessage), mut ui_callback: impl FnMut(InputEvent)) {
		match event {
			WindowEvent::PointerMoved { position, source, .. } => {
				self.pointer_position = *position;

				let PointerSource::TabletTool { kind, data } = source else {
					ui_callback(InputEvent::pointer().position(*position).moved().modifiers(self.modifiers).build());
					return;
				};
				let ui_capture = if self.pointer_keys.is_empty() {
					self.pointer_locked() || !self.in_viewport(*position)
				} else {
					self.ui_capture
				};
				if ui_capture {
					ui_callback(InputEvent::pointer().position(*position).moved().modifiers(self.modifiers).build());
					return;
				}

				editor_callback(InputMessage::PointerMove {
					editor_mouse_state: self.tablet_pointer_state(kind, data),
					modifier_keys: self.modifier_keys(),
				});
			}
			WindowEvent::PointerEntered { position, .. } => {
				self.pointer_position = *position;
				ui_callback(InputEvent::pointer().position(*position).entered().modifiers(self.modifiers).build())
			}
			WindowEvent::PointerLeft { position: Some(position), .. } => {
				self.pointer_position = *position;
				ui_callback(InputEvent::pointer().position(*position).exited().modifiers(self.modifiers).build())
			}
			WindowEvent::PointerLeft { position: None, .. } => ui_callback(InputEvent::pointer().exited().modifiers(self.modifiers).build()),
			WindowEvent::PointerButton { state, button, position, .. } => {
				self.pointer_position = *position;

				let tablet = matches!(button, ButtonSource::TabletTool { .. });

				// Stroke keeps capture decided from first button press until all buttons are released.
				if state.is_pressed() && self.pointer_keys.is_empty() {
					self.ui_capture = self.pointer_locked() || !tablet || !self.in_viewport(*position);
				}

				let mouse_button = button.clone().mouse_button();
				let keys = match mouse_button {
					Some(MouseButton::Left) => MouseKeys::LEFT,
					Some(MouseButton::Right) => MouseKeys::RIGHT,
					Some(MouseButton::Middle) => MouseKeys::MIDDLE,
					Some(MouseButton::Back) => MouseKeys::BACK,
					Some(MouseButton::Forward) => MouseKeys::FORWARD,
					_ => MouseKeys::NONE,
				};
				match state {
					ElementState::Pressed => self.pointer_keys.insert(keys),
					ElementState::Released => self.pointer_keys.remove(keys),
				}

				let count = mouse_button.map_or(1, |button| self.click_tracker.input(*position, button, *state));

				let back_or_forward = matches!(mouse_button, Some(MouseButton::Back | MouseButton::Forward));
				if self.pointer_locked() || !(back_or_forward || (tablet && !self.ui_capture)) {
					let pointer = InputEvent::pointer().position(*position);
					let input = match state {
						ElementState::Pressed => pointer.pressed(button.clone(), count),
						ElementState::Released => pointer.released(button.clone(), count),
					};
					ui_callback(input.modifiers(self.modifiers).build());
					return;
				}

				let editor_mouse_state = match button {
					ButtonSource::TabletTool { kind, data, .. } => self.tablet_pointer_state(kind, data),
					_ => self.pointer_state(),
				};
				let modifier_keys = self.modifier_keys();
				match state {
					ElementState::Pressed => editor_callback(InputMessage::PointerDown { editor_mouse_state, modifier_keys }),
					ElementState::Released if count % 2 == 0 => {
						editor_callback(InputMessage::PointerUp { editor_mouse_state, modifier_keys });
						editor_callback(InputMessage::DoubleClick {
							editor_mouse_state: PointerState {
								mouse_keys: keys,
								..editor_mouse_state
							},
							modifier_keys,
						});
					}
					ElementState::Released => editor_callback(InputMessage::PointerUp { editor_mouse_state, modifier_keys }),
				}
			}
			WindowEvent::MouseWheel { delta, .. } => {
				if self.pointer_locked() || !self.in_viewport(self.pointer_position) {
					let input = match delta {
						MouseScrollDelta::LineDelta(x, y) => InputEvent::pointer().scrolled_lines(f64::from(*x), f64::from(*y)),
						MouseScrollDelta::PixelDelta(position) => InputEvent::pointer().scrolled_pixels(position.x, position.y),
					};
					ui_callback(input.modifiers(self.modifiers).build());
					return;
				}

				let (x, y) = match delta {
					MouseScrollDelta::LineDelta(x, y) => (f64::from(*x) * SCROLL_LINE_WIDTH, f64::from(*y) * SCROLL_LINE_HEIGHT),
					MouseScrollDelta::PixelDelta(position) => (position.x, position.y),
				};

				let scroll_delta = ScrollDelta::new(-x * SCROLL_SPEED_X, -y * SCROLL_SPEED_Y, 0.);

				editor_callback(InputMessage::WheelScroll {
					editor_mouse_state: PointerState { scroll_delta, ..self.pointer_state() },
					modifier_keys: self.modifier_keys(),
				});
			}
			WindowEvent::PinchGesture { delta, .. } => {
				if self.pointer_locked() || !self.in_viewport(self.pointer_position) || !delta.is_normal() {
					ui_callback(InputEvent::pointer().zoomed(*delta).modifiers(self.modifiers).build());
					return;
				}

				// TODO: This is a temporary solution to handle pinch gestures, we should handle pinch gestures editor-side instead.
				let scroll_delta = ScrollDelta::new(0., -delta * PINCH_ZOOM_SPEED, 0.);
				editor_callback(InputMessage::WheelScroll {
					editor_mouse_state: PointerState { scroll_delta, ..self.pointer_state() },
					modifier_keys: self.modifier_keys() | ModifierKeys::CONTROL,
				});
			}
			WindowEvent::ModifiersChanged(modifiers) => {
				self.modifiers = modifiers.state();
			}
			WindowEvent::KeyboardInput { event, .. } => ui_callback(InputEvent::key(event).modifiers(self.modifiers).build()),
			_ => {}
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
			time: Some(self.start.elapsed().as_secs_f64() * 1000.),
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

	fn modifier_keys(&self) -> ModifierKeys {
		let mut keys = ModifierKeys::empty();
		keys.set(ModifierKeys::SHIFT, self.modifiers.shift_key());
		keys.set(ModifierKeys::CONTROL, self.modifiers.control_key());
		keys.set(ModifierKeys::ALT, self.modifiers.alt_key());
		keys.set(ModifierKeys::META_OR_COMMAND, self.modifiers.meta_key());
		keys
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

#[derive(Default)]
struct ClickTracker {
	left: ClickChains,
	right: ClickChains,
	middle: ClickChains,
	back: ClickChains,
	forward: ClickChains,
}

impl ClickTracker {
	fn input(&mut self, position: PhysicalPosition<f64>, button: MouseButton, state: ElementState) -> u32 {
		let position = (position.x as i32, position.y as i32);
		let clicks = match button {
			MouseButton::Left => &mut self.left,
			MouseButton::Right => &mut self.right,
			MouseButton::Middle => &mut self.middle,
			MouseButton::Back => &mut self.back,
			MouseButton::Forward => &mut self.forward,
			_ => return 1,
		};
		let chain = match state {
			ElementState::Pressed => &mut clicks.down,
			ElementState::Released => &mut clicks.up,
		};

		let now = Instant::now();
		let count = match chain {
			Some(previous) => {
				let within_time = now.saturating_duration_since(previous.time) <= MULTICLICK_TIMEOUT;
				let dx = position.0.abs_diff(previous.position.0) as usize;
				let dy = position.1.abs_diff(previous.position.1) as usize;
				let within_distance = dx <= MULTICLICK_ALLOWED_TRAVEL && dy <= MULTICLICK_ALLOWED_TRAVEL;
				if within_time && within_distance { previous.count.saturating_add(1) } else { 1 }
			}
			None => 1,
		};
		*chain = Some(Click { time: now, position, count });
		count
	}
}

#[derive(Default)]
struct ClickChains {
	down: Option<Click>,
	up: Option<Click>,
}

struct Click {
	time: Instant,
	position: (i32, i32),
	count: u32,
}
