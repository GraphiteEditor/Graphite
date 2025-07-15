use super::tool_prelude::*;

#[derive(Default, ExtractField)]
pub struct NavigateTool {
	fsm_state: NavigateToolFsmState,
	tool_data: NavigateToolData,
}

#[impl_message(Message, ToolMessage, Navigate)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum NavigateToolMessage {
	// Standard messages
	Abort,

	// Tool-specific messages
	PointerUp { zoom_in: bool },
	PointerMove { snap: Key },
	TiltCanvasBegin,
	ZoomCanvasBegin,
	End,
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

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for NavigateTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		self.fsm_state.process_event(message, &mut self.tool_data, context, &(), responses, true);
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			NavigateToolFsmState::Ready => actions!(NavigateToolMessageDiscriminant;
				TiltCanvasBegin,
				ZoomCanvasBegin,
			),
			NavigateToolFsmState::Tilting | NavigateToolFsmState::Zooming => actions!(NavigateToolMessageDiscriminant;
				PointerMove,
			),
			NavigateToolFsmState::ZoomOrClickZooming => actions!(NavigateToolMessageDiscriminant;
				PointerUp,
				PointerMove,
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
	Tilting,
	ZoomOrClickZooming,
	Zooming,
}

#[derive(Clone, Debug, Default)]
struct NavigateToolData {
	drag_start: Option<DVec2>,
}

impl Fsm for NavigateToolFsmState {
	type ToolData = NavigateToolData;
	type ToolOptions = ();

	fn transition(
		self,
		message: ToolMessage,
		tool_data: &mut Self::ToolData,
		ToolActionMessageContext { input, .. }: &mut ToolActionMessageContext,
		_tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let ToolMessage::Navigate(navigate) = message else { return self };
		match navigate {
			NavigateToolMessage::PointerUp { zoom_in } => {
				if self == NavigateToolFsmState::ZoomOrClickZooming {
					// Mouse has not moved from pointerdown to pointerup
					if tool_data.drag_start == Some(input.mouse.position) {
						responses.add_front(if zoom_in {
							NavigationMessage::CanvasZoomIncrease { center_on_mouse: true }
						} else {
							NavigationMessage::CanvasZoomDecrease { center_on_mouse: true }
						});
					}
				} else {
					responses.add_front(NavigationMessage::EndCanvasPTZ { abort_transform: false });
				}

				tool_data.drag_start = None;
				NavigateToolFsmState::Ready
			}
			NavigateToolMessage::PointerMove { snap } => {
				if self == NavigateToolFsmState::ZoomOrClickZooming {
					responses.add_front(NavigationMessage::BeginCanvasZoom);
					NavigateToolFsmState::Zooming
				} else {
					responses.add_front(NavigationMessage::PointerMove { snap });
					self
				}
			}
			NavigateToolMessage::TiltCanvasBegin => {
				responses.add_front(NavigationMessage::BeginCanvasTilt { was_dispatched_from_menu: false });
				NavigateToolFsmState::Tilting
			}
			NavigateToolMessage::ZoomCanvasBegin => {
				// Wait to decide between zooming and click zooming based on whether the next event is a PointerMove or PointerUp
				tool_data.drag_start = Some(input.mouse.position);
				NavigateToolFsmState::ZoomOrClickZooming
			}
			NavigateToolMessage::End => {
				tool_data.drag_start = None;
				NavigateToolFsmState::Ready
			}
			NavigateToolMessage::Abort => {
				responses.add_front(NavigationMessage::EndCanvasPTZ { abort_transform: false });
				tool_data.drag_start = None;
				NavigateToolFsmState::Ready
			}
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			NavigateToolFsmState::Ready | NavigateToolFsmState::ZoomOrClickZooming => HintData(vec![
				HintGroup(vec![
					HintInfo::mouse(MouseMotion::MmbDrag, ""),
					HintInfo::keys_and_mouse([Key::Space], MouseMotion::LmbDrag, "Pan").prepend_slash(),
				]),
				HintGroup(vec![HintInfo::keys_and_mouse([Key::Alt], MouseMotion::LmbDrag, "Tilt")]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Zoom"), HintInfo::keys([Key::Shift], "Increments").prepend_plus()]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Zoom In"), HintInfo::keys([Key::Shift], "Zoom Out").prepend_plus()]),
			]),
			NavigateToolFsmState::Tilting => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "15° Increments")]),
			]),
			NavigateToolFsmState::Zooming => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "Increments")]),
			]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		let cursor = match *self {
			NavigateToolFsmState::Ready => MouseCursorIcon::ZoomIn,
			NavigateToolFsmState::Tilting => MouseCursorIcon::Default,
			NavigateToolFsmState::Zooming | NavigateToolFsmState::ZoomOrClickZooming => MouseCursorIcon::ZoomIn,
		};

		responses.add(FrontendMessage::UpdateMouseCursor { cursor });
	}
}
