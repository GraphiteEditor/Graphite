use super::tool_prelude::*;

#[derive(Default)]
pub struct NavigateTool {
	fsm_state: NavigateToolFsmState,
	tool_data: NavigateToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Navigate)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum NavigateToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,

	// Tool-specific messages
	ClickZoom {
		zoom_in: bool,
	},
	PointerMove {
		snap_angle: Key,
		snap_zoom: Key,
	},
	RotateCanvasBegin,
	TransformCanvasEnd,
	TranslateCanvasBegin,
	ZoomCanvasBegin,
}

impl ToolMetadata for NavigateTool {
	fn icon_name(&self) -> String {
		"GeneralNavigateTool".into()
	}
	fn tooltip(&self) -> String {
		"Navigate Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Navigate
	}
}

impl LayoutHolder for NavigateTool {
	fn layout(&self) -> Layout {
		Layout::WidgetLayout(WidgetLayout::default())
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for NavigateTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &(), responses, true);
	}

	fn actions(&self) -> ActionList {
		use NavigateToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(NavigateToolMessageDiscriminant;
				TranslateCanvasBegin,
				RotateCanvasBegin,
				ZoomCanvasBegin,
			),
			_ => actions!(NavigateToolMessageDiscriminant;
				ClickZoom,
				PointerMove,
				TransformCanvasEnd,
			),
		}
	}
}

impl ToolTransition for NavigateTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(NavigateToolMessage::Abort.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum NavigateToolFsmState {
	#[default]
	Ready,
	Panning,
	Tilting,
	Zooming,
}

#[derive(Clone, Debug, Default)]
struct NavigateToolData {
	drag_start: DVec2,
}

impl Fsm for NavigateToolFsmState {
	type ToolData = NavigateToolData;
	type ToolOptions = ();

	fn transition(
		self,
		message: ToolMessage,
		tool_data: &mut Self::ToolData,
		ToolActionHandlerData { input, .. }: &mut ToolActionHandlerData,
		_tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let ToolMessage::Navigate(navigate) = message else {
			return self;
		};

		match navigate {
			NavigateToolMessage::ClickZoom { zoom_in } => {
				responses.add_front(NavigationMessage::TransformCanvasEnd { abort_transform: false });

				// Mouse has not moved from pointerdown to pointerup
				if tool_data.drag_start == input.mouse.position {
					responses.add_front(if zoom_in {
						NavigationMessage::IncreaseCanvasZoom { center_on_mouse: true }
					} else {
						NavigationMessage::DecreaseCanvasZoom { center_on_mouse: true }
					});
				}

				NavigateToolFsmState::Ready
			}
			NavigateToolMessage::PointerMove { snap_angle, snap_zoom } => {
				responses.add_front(NavigationMessage::PointerMove {
					snap_angle,
					wait_for_snap_angle_release: false,
					snap_zoom,
					zoom_from_viewport: Some(tool_data.drag_start),
				});
				self
			}
			NavigateToolMessage::TranslateCanvasBegin => {
				tool_data.drag_start = input.mouse.position;
				responses.add_front(NavigationMessage::TranslateCanvasBegin);
				NavigateToolFsmState::Panning
			}
			NavigateToolMessage::RotateCanvasBegin => {
				tool_data.drag_start = input.mouse.position;
				responses.add_front(NavigationMessage::RotateCanvasBegin { was_dispatched_from_menu: false });
				NavigateToolFsmState::Tilting
			}
			NavigateToolMessage::ZoomCanvasBegin => {
				tool_data.drag_start = input.mouse.position;
				responses.add_front(NavigationMessage::ZoomCanvasBegin);
				NavigateToolFsmState::Zooming
			}
			NavigateToolMessage::TransformCanvasEnd => {
				responses.add_front(NavigationMessage::TransformCanvasEnd { abort_transform: false });
				NavigateToolFsmState::Ready
			}
			NavigateToolMessage::Abort => {
				responses.add_front(NavigationMessage::TransformCanvasEnd { abort_transform: false });
				NavigateToolFsmState::Ready
			}
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			NavigateToolFsmState::Ready => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Zoom In"), HintInfo::keys([Key::Shift], "Zoom Out").prepend_plus()]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Zoom"), HintInfo::keys([Key::Control], "Increments").prepend_plus()]),
				HintGroup(vec![
					HintInfo::keys_and_mouse([Key::Space], MouseMotion::LmbDrag, ""),
					HintInfo::mouse(MouseMotion::MmbDrag, "Pan").prepend_slash(),
				]),
				HintGroup(vec![HintInfo::keys_and_mouse([Key::Alt], MouseMotion::LmbDrag, "Tilt")]),
			]),
			NavigateToolFsmState::Tilting => HintData(vec![HintGroup(vec![HintInfo::keys([Key::Control], "Snap 15°")])]),
			NavigateToolFsmState::Zooming => HintData(vec![HintGroup(vec![HintInfo::keys([Key::Control], "Increments")])]),
			_ => HintData(Vec::new()),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		let cursor = match *self {
			NavigateToolFsmState::Ready => MouseCursorIcon::ZoomIn,
			NavigateToolFsmState::Panning => MouseCursorIcon::Grabbing,
			NavigateToolFsmState::Tilting => MouseCursorIcon::Default,
			NavigateToolFsmState::Zooming => MouseCursorIcon::ZoomIn,
		};

		responses.add(FrontendMessage::UpdateMouseCursor { cursor });
	}
}
