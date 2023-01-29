use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::PropertyHolder;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use glam::DVec2;
use serde::{Deserialize, Serialize};

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

impl PropertyHolder for NavigateTool {}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for NavigateTool {
	fn process_message(&mut self, message: ToolMessage, tool_data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
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
			document_dirty: None,
			tool_abort: Some(NavigateToolMessage::Abort.into()),
			selection_changed: None,
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
		(_document, _document_id, _global_tool_data, input, _font_cache): ToolActionHandlerData,
		_tool_options: &Self::ToolOptions,
		messages: &mut VecDeque<Message>,
	) -> Self {
		if let ToolMessage::Navigate(navigate) = message {
			use NavigateToolMessage::*;

			match navigate {
				ClickZoom { zoom_in } => {
					messages.push_front(NavigationMessage::TransformCanvasEnd.into());

					// Mouse has not moved from pointerdown to pointerup
					if tool_data.drag_start == input.mouse.position {
						messages.push_front(if zoom_in {
							NavigationMessage::IncreaseCanvasZoom { center_on_mouse: true }.into()
						} else {
							NavigationMessage::DecreaseCanvasZoom { center_on_mouse: true }.into()
						});
					}

					NavigateToolFsmState::Ready
				}
				PointerMove { snap_angle, snap_zoom } => {
					messages.push_front(
						NavigationMessage::PointerMove {
							snap_angle,
							wait_for_snap_angle_release: false,
							snap_zoom,
							zoom_from_viewport: Some(tool_data.drag_start),
						}
						.into(),
					);
					self
				}
				TranslateCanvasBegin => {
					tool_data.drag_start = input.mouse.position;
					messages.push_front(NavigationMessage::TranslateCanvasBegin.into());
					NavigateToolFsmState::Panning
				}
				RotateCanvasBegin => {
					tool_data.drag_start = input.mouse.position;
					messages.push_front(NavigationMessage::RotateCanvasBegin.into());
					NavigateToolFsmState::Tilting
				}
				ZoomCanvasBegin => {
					tool_data.drag_start = input.mouse.position;
					messages.push_front(NavigationMessage::ZoomCanvasBegin.into());
					NavigateToolFsmState::Zooming
				}
				TransformCanvasEnd => {
					messages.push_front(NavigationMessage::TransformCanvasEnd.into());
					NavigateToolFsmState::Ready
				}
				Abort => {
					messages.push_front(NavigationMessage::TransformCanvasEnd.into());
					NavigateToolFsmState::Ready
				}
			}
		} else {
			self
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			NavigateToolFsmState::Ready => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Zoom In"), HintInfo::keys([Key::Shift], "Zoom Out").prepend_plus()]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Zoom"), HintInfo::keys([Key::Control], "Snap Increments").prepend_plus()]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::MmbDrag, "Pan")]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::RmbDrag, "Tilt"), HintInfo::keys([Key::Control], "Snap 15°").prepend_plus()]),
			]),
			NavigateToolFsmState::Tilting => HintData(vec![HintGroup(vec![HintInfo::keys([Key::Control], "Snap 15°")])]),
			NavigateToolFsmState::Zooming => HintData(vec![HintGroup(vec![HintInfo::keys([Key::Control], "Snap Increments")])]),
			_ => HintData(Vec::new()),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		let cursor = match *self {
			NavigateToolFsmState::Ready => MouseCursorIcon::ZoomIn,
			NavigateToolFsmState::Panning => MouseCursorIcon::Grabbing,
			NavigateToolFsmState::Tilting => MouseCursorIcon::Default,
			NavigateToolFsmState::Zooming => MouseCursorIcon::ZoomIn,
		};

		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor }.into());
	}
}
