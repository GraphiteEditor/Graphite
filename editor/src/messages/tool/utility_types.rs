#![allow(clippy::too_many_arguments)]
use super::common_functionality::overlay_renderer::OverlayRenderer;
use super::common_functionality::shape_editor::ShapeState;
use super::tool_messages::*;
use crate::messages::broadcast::broadcast_event::BroadcastEvent;
use crate::messages::broadcast::BroadcastMessage;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, KeysGroup, LayoutKeysGroup, MouseMotion};
use crate::messages::input_mapper::utility_types::macros::action_keys;
use crate::messages::input_mapper::utility_types::misc::ActionKeys;
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widgets::button_widgets::IconButton;
use crate::messages::layout::utility_types::widgets::input_widgets::SwatchPairInput;
use crate::messages::layout::utility_types::widgets::label_widgets::{Separator, SeparatorDirection, SeparatorType};
use crate::messages::prelude::*;
use crate::node_graph_executor::NodeGraphExecutor;

use document_legacy::layers::style::RenderData;
use graphene_core::raster::color::Color;

use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug};

pub struct ToolActionHandlerData<'a> {
	pub document: &'a DocumentMessageHandler,
	pub document_id: u64,
	pub global_tool_data: &'a DocumentToolData,
	pub input: &'a InputPreprocessorMessageHandler,
	pub render_data: &'a RenderData<'a>,
	pub shape_overlay: &'a mut OverlayRenderer,
	pub shape_editor: &'a mut ShapeState,
	pub node_graph: &'a NodeGraphExecutor,
}
impl<'a> ToolActionHandlerData<'a> {
	pub fn new(
		document: &'a DocumentMessageHandler,
		document_id: u64,
		global_tool_data: &'a DocumentToolData,
		input: &'a InputPreprocessorMessageHandler,
		render_data: &'a RenderData<'a>,
		shape_overlay: &'a mut OverlayRenderer,
		shape_editor: &'a mut ShapeState,
		node_graph: &'a NodeGraphExecutor,
	) -> Self {
		Self {
			document,
			document_id,
			global_tool_data,
			input,
			render_data,
			shape_overlay,
			shape_editor,
			node_graph,
		}
	}
}

pub trait ToolCommon: for<'a, 'b> MessageHandler<ToolMessage, &'b mut ToolActionHandlerData<'a>> + PropertyHolder + ToolTransition + ToolMetadata {}
impl<T> ToolCommon for T where T: for<'a, 'b> MessageHandler<ToolMessage, &'b mut ToolActionHandlerData<'a>> + PropertyHolder + ToolTransition + ToolMetadata {}

type Tool = dyn ToolCommon + Send + Sync;

/// The FSM (finite state machine) is a flowchart between different operating states that a specific tool might be in.
/// It is the central "core" logic area of each tool which is in charge of maintaining the state of the tool and responding to events coming from outside (like user input).
/// For example, a tool might be `Ready` or `Drawing` depending on if the user is idle or actively drawing with the mouse held down.
/// The FSM keeps track of what the tool is doing and allows the tool to take action when events are directed at the FSM.
/// Every tool, which implements this trait, must implement the `transition()` function.
/// That is where new events are sent, and where the flowchart transition logic occurs to respond to events and end in a new state.
pub trait Fsm {
	/// The implementing tool must set this to a struct designed to store the internal values stored in the tool.
	/// For example, it might be used to store the starting location of a point when a drag began so the displacement distance can be calculated.
	type ToolData;
	/// The implementing tool must set this to a struct (or `()` if none) designed to store the values of the tool options set by the user in the Options Bar
	/// (located above the viewport, below the document's tab).
	type ToolOptions;

	/// Implementing this mandatory trait function lets a specific tool react accordingly (and potentially change its state or internal variables) upon receiving an event to do something.
	/// Based on its current state, and what the event is, the FSM (finite state machine) should direct the tool to an appropriate outcome.
	/// For example, if the tool's FSM is in a `Ready` state and receives a `DragStart` message as its event, it may decide to send some messages,
	/// update some internal tool variables, and end by transitioning to a `Drawing` state.
	#[must_use]
	fn transition(self, message: ToolMessage, tool_data: &mut Self::ToolData, transition_data: &mut ToolActionHandlerData, options: &Self::ToolOptions, responses: &mut VecDeque<Message>) -> Self;

	/// Implementing this trait function lets a specific tool provide a list of hints (user input actions presently available) to draw in the footer bar.
	fn update_hints(&self, responses: &mut VecDeque<Message>);
	/// Implementing this trait function lets a specific tool set the current mouse cursor icon.
	fn update_cursor(&self, responses: &mut VecDeque<Message>);

	/// If this message is a standard tool message, process it and return true. Standard tool messages are those which are common across every tool.
	fn standard_tool_messages(&self, message: &ToolMessage, responses: &mut VecDeque<Message>, _tool_data: &mut Self::ToolData) -> bool {
		// Check for standard hits or cursor events
		match message {
			ToolMessage::UpdateHints => {
				self.update_hints(responses);
				true
			}
			ToolMessage::UpdateCursor => {
				self.update_cursor(responses);
				true
			}
			_ => false,
		}
	}

	/// When an event makes the tool change or do something, it is processed here to perform a step (transition) on the tool's finite state machine (FSM).
	/// This function is called by the specific tool's message handler when the dispatcher routes a message to the active tool.
	fn process_event(
		&mut self,
		message: ToolMessage,
		tool_data: &mut Self::ToolData,
		transition_data: &mut ToolActionHandlerData,
		options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
		update_cursor_on_transition: bool,
	) where
		Self: PartialEq + Sized + Copy,
	{
		// If this message is one of the standard tool messages, process it and exit early
		if self.standard_tool_messages(&message, responses, tool_data) {
			return;
		}

		// Transition the tool
		let new_state = self.transition(message, tool_data, transition_data, options, responses);

		// Update state
		if *self != new_state {
			*self = new_state;
			self.update_hints(responses);
			if update_cursor_on_transition {
				self.update_cursor(responses);
			}
		}
	}
}

#[derive(Debug, Clone)]
pub struct DocumentToolData {
	pub primary_color: Color,
	pub secondary_color: Color,
}

impl DocumentToolData {
	pub fn update_working_colors(&self, responses: &mut VecDeque<Message>) {
		let layout = WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::SwatchPairInput(SwatchPairInput {
					primary: self.primary_color,
					secondary: self.secondary_color,
				}))],
			},
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::IconButton(IconButton {
						size: 16,
						icon: "Swap".into(),
						tooltip: "Swap".into(),
						tooltip_shortcut: action_keys!(ToolMessageDiscriminant::SwapColors),
						on_update: WidgetCallback::new(|_| ToolMessage::SwapColors.into()),
						..Default::default()
					})),
					WidgetHolder::new(Widget::IconButton(IconButton {
						size: 16,
						icon: "WorkingColors".into(),
						tooltip: "Reset".into(),
						tooltip_shortcut: action_keys!(ToolMessageDiscriminant::ResetColors),
						on_update: WidgetCallback::new(|_| ToolMessage::ResetColors.into()),
						..Default::default()
					})),
				],
			},
		]);

		responses.add(LayoutMessage::SendLayout {
			layout: Layout::WidgetLayout(layout),
			layout_target: LayoutTarget::WorkingColors,
		});

		responses.add(BroadcastMessage::TriggerEvent(BroadcastEvent::WorkingColorChanged));
	}
}

#[derive(Clone, Debug, Default)]
pub struct EventToMessageMap {
	pub document_dirty: Option<ToolMessage>,
	pub selection_changed: Option<ToolMessage>,
	pub tool_abort: Option<ToolMessage>,
	pub working_color_changed: Option<ToolMessage>,
}

pub trait ToolTransition {
	fn event_to_message_map(&self) -> EventToMessageMap;

	fn activate(&self, responses: &mut VecDeque<Message>) {
		let mut subscribe_message = |broadcast_to_tool_mapping: Option<ToolMessage>, event: BroadcastEvent| {
			if let Some(mapping) = broadcast_to_tool_mapping {
				responses.add(BroadcastMessage::SubscribeEvent {
					on: event,
					send: Box::new(mapping.into()),
				});
			};
		};

		let event_to_tool_map = self.event_to_message_map();
		subscribe_message(event_to_tool_map.document_dirty, BroadcastEvent::DocumentIsDirty);
		subscribe_message(event_to_tool_map.tool_abort, BroadcastEvent::ToolAbort);
		subscribe_message(event_to_tool_map.selection_changed, BroadcastEvent::SelectionChanged);
		subscribe_message(event_to_tool_map.working_color_changed, BroadcastEvent::WorkingColorChanged);
	}

	fn deactivate(&self, responses: &mut VecDeque<Message>) {
		let mut unsubscribe_message = |broadcast_to_tool_mapping: Option<ToolMessage>, event: BroadcastEvent| {
			if let Some(mapping) = broadcast_to_tool_mapping {
				responses.add(BroadcastMessage::UnsubscribeEvent {
					on: event,
					message: Box::new(mapping.into()),
				});
			};
		};

		let event_to_tool_map = self.event_to_message_map();
		unsubscribe_message(event_to_tool_map.document_dirty, BroadcastEvent::DocumentIsDirty);
		unsubscribe_message(event_to_tool_map.tool_abort, BroadcastEvent::ToolAbort);
		unsubscribe_message(event_to_tool_map.selection_changed, BroadcastEvent::SelectionChanged);
		unsubscribe_message(event_to_tool_map.working_color_changed, BroadcastEvent::WorkingColorChanged);
	}
}

pub trait ToolMetadata {
	fn icon_name(&self) -> String;
	fn tooltip(&self) -> String;
	fn tool_type(&self) -> ToolType;
}

pub struct ToolData {
	pub active_tool_type: ToolType,
	pub tools: HashMap<ToolType, Box<Tool>>,
}

impl fmt::Debug for ToolData {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ToolData").field("active_tool_type", &self.active_tool_type).field("tool_options", &"[…]").finish()
	}
}

impl ToolData {
	pub fn active_tool_mut(&mut self) -> &mut Box<Tool> {
		self.tools.get_mut(&self.active_tool_type).expect("The active tool is not initialized")
	}

	pub fn active_tool(&self) -> &Tool {
		self.tools.get(&self.active_tool_type).map(|x| x.as_ref()).expect("The active tool is not initialized")
	}
}

impl PropertyHolder for ToolData {
	fn properties(&self) -> Layout {
		let tool_groups_layout = list_tools_in_groups()
			.iter()
			.map(|tool_group| tool_group.iter().map(|tool_availability| {
				match tool_availability {
					ToolAvailability::Available(tool) => ToolEntry {
						tooltip: tool.tooltip(),
						tooltip_shortcut: action_keys!(tool_type_to_activate_tool_message(tool.tool_type())),
						icon_name: tool.icon_name(),
						tool_type: tool.tool_type(),
					},
					ToolAvailability::ComingSoon(tool) => tool.clone(),
				}
			}).collect::<Vec<_>>())
			.flat_map(|group| {
				let separator = std::iter::once(WidgetHolder::new(Widget::Separator(Separator {
					direction: SeparatorDirection::Vertical,
					separator_type: SeparatorType::Section,
				})));
				let buttons = group.into_iter().map(|ToolEntry { tooltip, tooltip_shortcut, tool_type, icon_name }| {
					WidgetHolder::new(Widget::IconButton(IconButton {
						icon: icon_name,
						size: 32,
						disabled: false,
						active: self.active_tool_type == tool_type,
						tooltip: tooltip.clone(),
						tooltip_shortcut,
						on_update: WidgetCallback::new(move |_| {
							if !tooltip.contains("Coming Soon") {
								ToolMessage::ActivateTool { tool_type }.into()
							} else {
								DialogMessage::RequestComingSoonDialog { issue: None }.into()
							}
						}),
					}))
				});

				separator.chain(buttons)
			})
			// Skip the initial separator
			.skip(1)
			.collect();

		Layout::WidgetLayout(WidgetLayout {
			layout: vec![LayoutGroup::Column { widgets: tool_groups_layout }],
		})
	}
}

#[derive(Debug, Clone)]
pub struct ToolEntry {
	pub tooltip: String,
	pub tooltip_shortcut: Option<ActionKeys>,
	pub icon_name: String,
	pub tool_type: ToolType,
}

#[derive(Debug)]
pub struct ToolFsmState {
	pub document_tool_data: DocumentToolData,
	pub tool_data: ToolData,
}

impl Default for ToolFsmState {
	fn default() -> Self {
		Self {
			tool_data: ToolData {
				active_tool_type: ToolType::Select,
				tools: list_tools_in_groups()
					.into_iter()
					.flatten()
					.filter_map(|tool| match tool {
						ToolAvailability::Available(tool) => Some((tool.tool_type(), tool)),
						ToolAvailability::ComingSoon(_) => None,
					})
					.collect(),
			},
			document_tool_data: DocumentToolData {
				primary_color: Color::BLACK,
				secondary_color: Color::WHITE,
			},
		}
	}
}

impl ToolFsmState {
	pub fn new() -> Self {
		Self::default()
	}
}

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
pub enum ToolType {
	// General tool group
	Select,
	Artboard,
	Navigate,
	Eyedropper,
	Fill,
	Gradient,

	// Vector tool group
	Path,
	Pen,
	Freehand,
	Spline,
	Line,
	Rectangle,
	Ellipse,
	Shape,
	Text,

	// Raster tool group
	Brush,
	Heal,
	Clone,
	Patch,
	Detail,
	Relight,
	Imaginate,
	Frame,
}

enum ToolAvailability {
	Available(Box<Tool>),
	ComingSoon(ToolEntry),
}

/// List of all the tools in their conventional ordering and grouping.
fn list_tools_in_groups() -> Vec<Vec<ToolAvailability>> {
	vec![
		vec![
			// General tool group
			ToolAvailability::Available(Box::<select_tool::SelectTool>::default()),
			ToolAvailability::Available(Box::<artboard_tool::ArtboardTool>::default()),
			ToolAvailability::Available(Box::<navigate_tool::NavigateTool>::default()),
			ToolAvailability::Available(Box::<eyedropper_tool::EyedropperTool>::default()),
			ToolAvailability::Available(Box::<fill_tool::FillTool>::default()),
			ToolAvailability::Available(Box::<gradient_tool::GradientTool>::default()),
		],
		vec![
			// Vector tool group
			ToolAvailability::Available(Box::<path_tool::PathTool>::default()),
			ToolAvailability::Available(Box::<pen_tool::PenTool>::default()),
			ToolAvailability::Available(Box::<freehand_tool::FreehandTool>::default()),
			ToolAvailability::Available(Box::<spline_tool::SplineTool>::default()),
			ToolAvailability::Available(Box::<line_tool::LineTool>::default()),
			ToolAvailability::Available(Box::<rectangle_tool::RectangleTool>::default()),
			ToolAvailability::Available(Box::<ellipse_tool::EllipseTool>::default()),
			ToolAvailability::Available(Box::<shape_tool::ShapeTool>::default()),
			ToolAvailability::Available(Box::<text_tool::TextTool>::default()),
		],
		vec![
			// Raster tool group
			ToolAvailability::Available(Box::<frame_tool::FrameTool>::default()),
			ToolAvailability::Available(Box::<imaginate_tool::ImaginateTool>::default()),
			ToolAvailability::Available(Box::<brush_tool::BrushTool>::default()),
			ToolAvailability::ComingSoon(ToolEntry {
				tool_type: ToolType::Heal,
				icon_name: "RasterHealTool".into(),
				tooltip: "Coming Soon: Heal Tool (J)".into(),
				tooltip_shortcut: None,
			}),
			ToolAvailability::ComingSoon(ToolEntry {
				tool_type: ToolType::Clone,
				icon_name: "RasterCloneTool".into(),
				tooltip: "Coming Soon: Clone Tool (C)".into(),
				tooltip_shortcut: None,
			}),
			ToolAvailability::ComingSoon(ToolEntry {
				tool_type: ToolType::Patch,
				icon_name: "RasterPatchTool".into(),
				tooltip: "Coming Soon: Patch Tool".into(),
				tooltip_shortcut: None,
			}),
			ToolAvailability::ComingSoon(ToolEntry {
				tool_type: ToolType::Detail,
				icon_name: "RasterDetailTool".into(),
				tooltip: "Coming Soon: Detail Tool (D)".into(),
				tooltip_shortcut: None,
			}),
			ToolAvailability::ComingSoon(ToolEntry {
				tool_type: ToolType::Relight,
				icon_name: "RasterRelightTool".into(),
				tooltip: "Coming Soon: Relight Tool (O)".into(),
				tooltip_shortcut: None,
			}),
		],
	]
}

pub fn tool_message_to_tool_type(tool_message: &ToolMessage) -> ToolType {
	match tool_message {
		// General tool group
		ToolMessage::Select(_) => ToolType::Select,
		ToolMessage::Artboard(_) => ToolType::Artboard,
		ToolMessage::Navigate(_) => ToolType::Navigate,
		ToolMessage::Eyedropper(_) => ToolType::Eyedropper,
		ToolMessage::Fill(_) => ToolType::Fill,
		ToolMessage::Gradient(_) => ToolType::Gradient,

		// Vector tool group
		ToolMessage::Path(_) => ToolType::Path,
		ToolMessage::Pen(_) => ToolType::Pen,
		ToolMessage::Freehand(_) => ToolType::Freehand,
		ToolMessage::Spline(_) => ToolType::Spline,
		ToolMessage::Line(_) => ToolType::Line,
		ToolMessage::Rectangle(_) => ToolType::Rectangle,
		ToolMessage::Ellipse(_) => ToolType::Ellipse,
		ToolMessage::Shape(_) => ToolType::Shape,
		ToolMessage::Text(_) => ToolType::Text,

		// Raster tool group
		ToolMessage::Brush(_) => ToolType::Brush,
		// ToolMessage::Heal(_) => ToolType::Heal,
		// ToolMessage::Clone(_) => ToolType::Clone,
		// ToolMessage::Patch(_) => ToolType::Patch,
		// ToolMessage::Detail(_) => ToolType::Detail,
		// ToolMessage::Relight(_) => ToolType::Relight,
		ToolMessage::Imaginate(_) => ToolType::Imaginate,
		ToolMessage::Frame(_) => ToolType::Frame,
		_ => panic!(
			"Conversion from ToolMessage to ToolType impossible because the given ToolMessage does not have a matching ToolType. Got: {:?}",
			tool_message
		),
	}
}

pub fn tool_type_to_activate_tool_message(tool_type: ToolType) -> ToolMessageDiscriminant {
	match tool_type {
		// General tool group
		ToolType::Select => ToolMessageDiscriminant::ActivateToolSelect,
		ToolType::Artboard => ToolMessageDiscriminant::ActivateToolArtboard,
		ToolType::Navigate => ToolMessageDiscriminant::ActivateToolNavigate,
		ToolType::Eyedropper => ToolMessageDiscriminant::ActivateToolEyedropper,
		ToolType::Fill => ToolMessageDiscriminant::ActivateToolFill,
		ToolType::Gradient => ToolMessageDiscriminant::ActivateToolGradient,

		// Vector tool group
		ToolType::Path => ToolMessageDiscriminant::ActivateToolPath,
		ToolType::Pen => ToolMessageDiscriminant::ActivateToolPen,
		ToolType::Freehand => ToolMessageDiscriminant::ActivateToolFreehand,
		ToolType::Spline => ToolMessageDiscriminant::ActivateToolSpline,
		ToolType::Line => ToolMessageDiscriminant::ActivateToolLine,
		ToolType::Rectangle => ToolMessageDiscriminant::ActivateToolRectangle,
		ToolType::Ellipse => ToolMessageDiscriminant::ActivateToolEllipse,
		ToolType::Shape => ToolMessageDiscriminant::ActivateToolShape,
		ToolType::Text => ToolMessageDiscriminant::ActivateToolText,

		// Raster tool group
		ToolType::Brush => ToolMessageDiscriminant::ActivateToolBrush,
		// ToolType::Heal => ToolMessageDiscriminant::ActivateToolHeal,
		// ToolType::Clone => ToolMessageDiscriminant::ActivateToolClone,
		// ToolType::Patch => ToolMessageDiscriminant::ActivateToolPatch,
		// ToolType::Detail => ToolMessageDiscriminant::ActivateToolDetail,
		// ToolType::Relight => ToolMessageDiscriminant::ActivateToolRelight,
		ToolType::Imaginate => ToolMessageDiscriminant::ActivateToolImaginate,
		ToolType::Frame => ToolMessageDiscriminant::ActivateToolFrame,
		_ => panic!(
			"Conversion from ToolType to ToolMessage impossible because the given ToolType does not have a matching ToolMessage. Got: {:?}",
			tool_type
		),
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct HintData(pub Vec<HintGroup>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct HintGroup(pub Vec<HintInfo>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct HintInfo {
	/// A `KeysGroup` specifies all the keys pressed simultaneously to perform an action (like "Ctrl C" to copy).
	/// Usually at most one is given, but less commonly, multiple can be used to describe additional hotkeys not used simultaneously (like the four different arrow keys to nudge a layer).
	#[serde(rename = "keyGroups")]
	pub key_groups: Vec<LayoutKeysGroup>,
	/// `None` means that the regular `key_groups` should be used for all platforms, `Some` is an override for a Mac-only input hint.
	#[serde(rename = "keyGroupsMac")]
	pub key_groups_mac: Option<Vec<LayoutKeysGroup>>,
	/// An optional `MouseMotion` that can indicate the mouse action, like which mouse button is used and whether a drag occurs.
	/// No such icon is shown if `None` is given, and it can be combined with `key_groups` if desired.
	pub mouse: Option<MouseMotion>,
	/// The text describing what occurs with this input combination.
	pub label: String,
	/// Draws a prepended "+" symbol which indicates that this is a refinement upon a previous hint in the group.
	pub plus: bool,
}

impl HintInfo {
	pub fn keys(keys: impl IntoIterator<Item = Key>, label: impl Into<String>) -> Self {
		let keys: Vec<_> = keys.into_iter().collect();
		Self {
			key_groups: vec![KeysGroup(keys).into()],
			key_groups_mac: None,
			mouse: None,
			label: label.into(),
			plus: false,
		}
	}

	pub fn mouse(mouse_motion: MouseMotion, label: impl Into<String>) -> Self {
		Self {
			key_groups: vec![],
			key_groups_mac: None,
			mouse: Some(mouse_motion),
			label: label.into(),
			plus: false,
		}
	}

	pub fn label(label: impl Into<String>) -> Self {
		Self {
			key_groups: vec![],
			key_groups_mac: None,
			mouse: None,
			label: label.into(),
			plus: false,
		}
	}

	pub fn keys_and_mouse(keys: impl IntoIterator<Item = Key>, mouse_motion: MouseMotion, label: impl Into<String>) -> Self {
		let keys: Vec<_> = keys.into_iter().collect();
		Self {
			key_groups: vec![KeysGroup(keys).into()],
			key_groups_mac: None,
			mouse: Some(mouse_motion),
			label: label.into(),
			plus: false,
		}
	}

	pub fn arrow_keys(label: impl Into<String>) -> Self {
		HintInfo {
			key_groups: vec![
				KeysGroup(vec![Key::ArrowUp]).into(),
				KeysGroup(vec![Key::ArrowRight]).into(),
				KeysGroup(vec![Key::ArrowDown]).into(),
				KeysGroup(vec![Key::ArrowLeft]).into(),
			],
			key_groups_mac: None,
			mouse: None,
			label: label.into(),
			plus: false,
		}
	}

	pub fn prepend_plus(mut self) -> Self {
		self.plus = true;
		self
	}

	pub fn add_mac_keys(mut self, keys: impl IntoIterator<Item = Key>) -> Self {
		let mac_keys: Vec<_> = keys.into_iter().collect();
		self.key_groups_mac = Some(vec![KeysGroup(mac_keys).into()]);
		self
	}
}

#[cfg(test)]
mod tool_crash_on_layer_delete_tests {
	use crate::application::{set_uuid_seed, Editor};
	use crate::messages::portfolio::document::DocumentMessage;
	use crate::messages::tool::utility_types::ToolType;
	use crate::test_utils::EditorTestUtils;

	use test_case::test_case;

	#[test_case(ToolType::Pen; "while using pen tool")]
	#[test_case(ToolType::Freehand; "while using freehand tool")]
	#[test_case(ToolType::Spline; "while using spline tool")]
	#[test_case(ToolType::Line; "while using line tool")]
	#[test_case(ToolType::Rectangle; "while using rectangle tool")]
	#[test_case(ToolType::Ellipse; "while using ellipse tool")]
	#[test_case(ToolType::Shape; "while using shape tool")]
	#[test_case(ToolType::Path; "while using path tool")]
	fn should_not_crash_when_layer_is_deleted(tool: ToolType) {
		set_uuid_seed(0);
		let mut test_editor = Editor::new();

		test_editor.select_tool(tool);
		test_editor.lmb_mousedown(0.0, 0.0);
		test_editor.move_mouse(100.0, 100.0);

		test_editor.handle_message(DocumentMessage::DeleteSelectedLayers);
	}
}
