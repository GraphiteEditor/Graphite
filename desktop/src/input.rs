use std::time::Instant;
use winit::dpi::PhysicalPosition;
use winit::event::{ButtonSource, ElementState, MouseButton, MouseScrollDelta, PointerSource, TabletToolData, TabletToolKind, WindowEvent};
use winit::keyboard::ModifiersState;

use crate::ui::{InputEvent, MULTICLICK_ALLOWED_TRAVEL, MULTICLICK_TIMEOUT, PINCH_ZOOM_SPEED, SCROLL_LINE_HEIGHT, SCROLL_LINE_WIDTH, SCROLL_SPEED_X, SCROLL_SPEED_Y};
use crate::wrapper::messages::{EditorPointerState, InputMessage, ModifierKeys, MouseKeys, ScrollDelta};

pub(crate) struct InputState {
	start: Instant,
	viewport_info: Option<ViewportInfo>,
	modifiers: ModifiersState,
	pointer_position: PhysicalPosition<f64>,
	pointer_state: PointerState,
	click_tracker: ClickTracker,
	direct_input: bool,
}

impl InputState {
	pub(crate) fn new() -> Self {
		Self {
			start: Instant::now(),
			viewport_info: None,
			modifiers: ModifiersState::default(),
			pointer_position: PhysicalPosition::default(),
			pointer_state: PointerState::Hover { route: Route::Ui },
			click_tracker: ClickTracker::default(),
			direct_input: false,
		}
	}

	pub(crate) fn set_viewport_info(&mut self, x: f64, y: f64, width: f64, height: f64, scale: f64) {
		self.viewport_info = Some(ViewportInfo { x, y, width, height, scale });
	}

	pub(crate) fn set_direct_input(&mut self, enabled: bool) {
		self.direct_input = enabled;
	}

	pub(crate) fn lock_pointer(&mut self) {
		self.pointer_state = match self.pointer_state {
			PointerState::Hover { route } => PointerState::Locked {
				route,
				keys: MouseKeys::empty(),
				position: self.pointer_position,
			},
			PointerState::Stroke { route, keys } | PointerState::Locked { route, keys, .. } => PointerState::Locked {
				route,
				keys,
				position: self.pointer_position,
			},
		};
	}

	pub(crate) fn unlock_pointer(&mut self) -> Option<PhysicalPosition<f64>> {
		let PointerState::Locked {
			route: resume,
			keys,
			position: restore,
		} = self.pointer_state
		else {
			return None;
		};
		self.pointer_position = restore;
		self.pointer_state = match keys.is_empty() {
			true => PointerState::Hover { route: Route::Ui },
			false => PointerState::Stroke { route: resume, keys },
		};
		Some(restore)
	}

	pub(crate) fn pointer_locked(&self) -> bool {
		matches!(self.pointer_state, PointerState::Locked { .. })
	}

	pub(crate) fn modifiers(&self) -> ModifiersState {
		self.modifiers
	}

	pub(crate) fn process(&mut self, event: &WindowEvent, mut editor_callback: impl FnMut(InputMessage), mut ui_callback: impl FnMut(InputEvent)) {
		match event {
			WindowEvent::PointerMoved { position, source, .. } => {
				self.pointer_position = *position;

				let route = match self.pointer_state {
					PointerState::Hover { .. } => {
						let next = self.route(*position);
						self.pointer_state = PointerState::Hover { route: next };
						next
					}
					PointerState::Stroke { route, .. } => route,
					PointerState::Locked { keys, route: resume, .. } => match keys.is_empty() {
						true => Route::Ui,
						false => resume,
					},
				};
				match route {
					Route::Ui => ui_callback(InputEvent::pointer().position(*position).moved().modifiers(self.modifiers).build()),
					Route::Editor => editor_callback(InputMessage::PointerMove {
						editor_mouse_state: match source {
							PointerSource::TabletTool { kind, data } => self.tablet_pointer_state(kind, data),
							_ => self.pointer_state(),
						},
						modifier_keys: self.modifier_keys(),
					}),
				}
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

				let mouse_button = button.clone().mouse_button();
				let keys = match mouse_button {
					Some(MouseButton::Left) => MouseKeys::LEFT,
					Some(MouseButton::Right) => MouseKeys::RIGHT,
					Some(MouseButton::Middle) => MouseKeys::MIDDLE,
					Some(MouseButton::Back) => MouseKeys::BACK,
					Some(MouseButton::Forward) => MouseKeys::FORWARD,
					_ => MouseKeys::NONE,
				};

				let (pointer, route) = match self.pointer_state {
					PointerState::Hover { route } => match (state.is_pressed(), keys.is_empty()) {
						(true, false) => {
							let route = self.route(*position);
							(PointerState::Stroke { route, keys }, route)
						}
						(true, true) => (PointerState::Hover { route }, self.route(*position)),
						(false, _) => (PointerState::Hover { route }, route),
					},
					PointerState::Stroke { route, keys: mut held } => {
						match state.is_pressed() {
							true => held.insert(keys),
							false => held.remove(keys),
						}
						match held.is_empty() {
							true => (PointerState::Hover { route }, route),
							false => (PointerState::Stroke { route, keys: held }, route),
						}
					}
					PointerState::Locked { route, keys: mut held, position } => {
						let resume = if state.is_pressed() && held.is_empty() { Route::Ui } else { route };
						match state.is_pressed() {
							true => held.insert(keys),
							false => held.remove(keys),
						}
						(PointerState::Locked { route: resume, keys: held, position }, Route::Ui)
					}
				};
				self.pointer_state = pointer;

				let count = mouse_button.map_or(1, |button| self.click_tracker.input(*position, button, *state));

				let back_or_forward = matches!(mouse_button, Some(MouseButton::Back | MouseButton::Forward));
				if self.pointer_locked() || keys.is_empty() || !(back_or_forward || route == Route::Editor) {
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
							editor_mouse_state: EditorPointerState {
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
				if self.pointer_locked() || self.ui_captures(self.pointer_position) {
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
					editor_mouse_state: EditorPointerState { scroll_delta, ..self.pointer_state() },
					modifier_keys: self.modifier_keys(),
				});
			}
			WindowEvent::PinchGesture { delta, .. } => {
				if self.pointer_locked() || self.ui_captures(self.pointer_position) || !delta.is_normal() {
					ui_callback(InputEvent::pointer().zoomed(*delta).modifiers(self.modifiers).build());
					return;
				}

				// TODO: This is a temporary solution to handle pinch gestures, we should handle pinch gestures editor-side instead.
				let scroll_delta = ScrollDelta::new(0., -delta * PINCH_ZOOM_SPEED, 0.);
				editor_callback(InputMessage::WheelScroll {
					editor_mouse_state: EditorPointerState { scroll_delta, ..self.pointer_state() },
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

	fn ui_captures(&self, position: PhysicalPosition<f64>) -> bool {
		!self.direct_input || !self.viewport_info.as_ref().is_some_and(|info| info.contains(position))
	}

	fn route(&self, position: PhysicalPosition<f64>) -> Route {
		if self.ui_captures(position) { Route::Ui } else { Route::Editor }
	}

	fn pointer_keys(&self) -> MouseKeys {
		match self.pointer_state {
			PointerState::Hover { .. } => MouseKeys::empty(),
			PointerState::Stroke { keys, .. } | PointerState::Locked { keys, .. } => keys,
		}
	}

	fn pointer_state(&self) -> EditorPointerState {
		EditorPointerState {
			editor_position: (self.pointer_position.x / self.scale(), self.pointer_position.y / self.scale()).into(),
			mouse_keys: self.pointer_keys(),
			time: Some(self.start.elapsed().as_secs_f64() * 1000.),
			..Default::default()
		}
	}

	fn tablet_pointer_state(&self, kind: &TabletToolKind, data: &TabletToolData) -> EditorPointerState {
		EditorPointerState {
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

#[derive(Clone, Copy)]
enum PointerState {
	Hover { route: Route },
	Stroke { route: Route, keys: MouseKeys },
	Locked { route: Route, keys: MouseKeys, position: PhysicalPosition<f64> },
}

#[derive(Clone, Copy, PartialEq)]
enum Route {
	Ui,
	Editor,
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
