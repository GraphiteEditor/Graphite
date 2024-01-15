#![allow(clippy::too_many_arguments)]

use super::common_functionality::shape_editor::ShapeState;
use super::tool_messages::*;
use crate::messages::broadcast::broadcast_event::BroadcastEvent;
use crate::messages::broadcast::BroadcastMessage;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, KeysGroup, LayoutKeysGroup, MouseMotion};
use crate::messages::input_mapper::utility_types::macros::action_keys;
use crate::messages::input_mapper::utility_types::misc::ActionKeys;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::overlays::utility_types::OverlayProvider;
use crate::messages::prelude::*;
use crate::node_graph_executor::NodeGraphExecutor;

use graphene_core::raster::color::Color;
use graphene_std::text::FontCache;

use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug};

pub struct ToolActionHandlerData<'a> {
	pub document: &'a DocumentMessageHandler,
	pub document_id: DocumentId,
	pub global_tool_data: &'a DocumentToolData,
	pub input: &'a InputPreprocessorMessageHandler,
	pub font_cache: &'a FontCache,
	pub shape_editor: &'a mut ShapeState,
	pub node_graph: &'a NodeGraphExecutor,
}
impl<'a> ToolActionHandlerData<'a> {
	pub fn new(
		document: &'a DocumentMessageHandler,
		document_id: DocumentId,
		global_tool_data: &'a DocumentToolData,
		input: &'a InputPreprocessorMessageHandler,
		font_cache: &'a FontCache,
		shape_editor: &'a mut ShapeState,
		node_graph: &'a NodeGraphExecutor,
	) -> Self {
		Self {
			document,
			document_id,
			global_tool_data,
			input,
			font_cache,
			shape_editor,
			node_graph,
		}
	}
}

pub trait ToolCommon: for<'a, 'b> MessageHandler<ToolMessage, &'b mut ToolActionHandlerData<'a>> + LayoutHolder + ToolTransition + ToolMetadata {}
impl<T> ToolCommon for T where T: for<'a, 'b> MessageHandler<ToolMessage, &'b mut ToolActionHandlerData<'a>> + LayoutHolder + ToolTransition + ToolMetadata {}

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
				widgets: vec![WorkingColorsInput::new(self.primary_color, self.secondary_color).widget_holder()],
			},
			LayoutGroup::Row {
				widgets: vec![
					IconButton::new("Swap", 16)
						.tooltip("Swap")
						.tooltip_shortcut(action_keys!(ToolMessageDiscriminant::SwapColors))
						.on_update(|_| ToolMessage::SwapColors.into())
						.widget_holder(),
					IconButton::new("WorkingColors", 16)
						.tooltip("Reset")
						.tooltip_shortcut(action_keys!(ToolMessageDiscriminant::ResetColors))
						.on_update(|_| ToolMessage::ResetColors.into())
						.widget_holder(),
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
	pub canvas_transformed: Option<ToolMessage>,
	pub selection_changed: Option<ToolMessage>,
	pub tool_abort: Option<ToolMessage>,
	pub working_color_changed: Option<ToolMessage>,
	pub overlay_provider: Option<OverlayProvider>,
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
		subscribe_message(event_to_tool_map.canvas_transformed, BroadcastEvent::CanvasTransformed);
		subscribe_message(event_to_tool_map.tool_abort, BroadcastEvent::ToolAbort);
		subscribe_message(event_to_tool_map.selection_changed, BroadcastEvent::SelectionChanged);
		subscribe_message(event_to_tool_map.working_color_changed, BroadcastEvent::WorkingColorChanged);
		if let Some(overlay_provider) = event_to_tool_map.overlay_provider {
			responses.add(OverlaysMessage::AddProvider(overlay_provider));
		}
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
		unsubscribe_message(event_to_tool_map.canvas_transformed, BroadcastEvent::CanvasTransformed);
		unsubscribe_message(event_to_tool_map.tool_abort, BroadcastEvent::ToolAbort);
		unsubscribe_message(event_to_tool_map.selection_changed, BroadcastEvent::SelectionChanged);
		unsubscribe_message(event_to_tool_map.working_color_changed, BroadcastEvent::WorkingColorChanged);
		if let Some(overlay_provider) = event_to_tool_map.overlay_provider {
			responses.add(OverlaysMessage::RemoveProvider(overlay_provider));
		}
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

impl LayoutHolder for ToolData {
	fn layout(&self) -> Layout {
		let tool_groups_layout = list_tools_in_groups()
			.iter()
			.map(|tool_group| tool_group.iter().map(|tool_availability| {
				match tool_availability {
					ToolAvailability::Available(tool) => ToolEntry::new( tool.tool_type(),  tool.icon_name())
						.tooltip( tool.tooltip())
						.tooltip_shortcut(action_keys!(tool_type_to_activate_tool_message(tool.tool_type())))

					,
					ToolAvailability::ComingSoon(tool) => tool.clone(),
				}
			}).collect::<Vec<_>>())
			.flat_map(|group| {
				let separator = std::iter::once(Separator::new(SeparatorType::Section).direction(SeparatorDirection::Vertical).widget_holder());
				let buttons = group.into_iter().map(|ToolEntry { tooltip, tooltip_shortcut, tool_type, icon_name }| {
					IconButton::new(icon_name, 32)
						.disabled( false)
						.active( self.active_tool_type == tool_type)
						.tooltip( tooltip.clone())
						.tooltip_shortcut(tooltip_shortcut)
						.on_update(move |_| {
							if !tooltip.contains("Coming Soon") {
								ToolMessage::ActivateTool { tool_type }.into()
							} else {
								DialogMessage::RequestComingSoonDialog { issue: None }.into()
							}
						}).widget_holder()
				});

				separator.chain(buttons)
			})
			// Skip the initial separator
			.skip(1)
			.collect();

		Layout::WidgetLayout(WidgetLayout {
			layout: vec![LayoutGroup::Row { widgets: tool_groups_layout }],
		})
	}
}

#[derive(Debug, Clone, Default, WidgetBuilder)]
#[widget_builder(not_widget_holder)]
pub struct ToolEntry {
	#[widget_builder(constructor)]
	pub tool_type: ToolType,
	#[widget_builder(constructor)]
	pub icon_name: String,
	pub tooltip: String,
	pub tooltip_shortcut: Option<ActionKeys>,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default, specta::Type)]
pub enum ToolType {
	// General tool group
	#[default]
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
	Polygon,
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
			ToolAvailability::Available(Box::<polygon_tool::PolygonTool>::default()),
			ToolAvailability::Available(Box::<text_tool::TextTool>::default()),
		],
		vec![
			// Raster tool group
			// ToolAvailability::Available(Box::<imaginate_tool::ImaginateTool>::default()), // TODO: Fix and reenable ASAP
			ToolAvailability::ComingSoon(
				ToolEntry::new(ToolType::Heal, "RasterImaginateTool").tooltip("Coming Soon: Imaginate Tool - Temporarily disabled, please use Imaginate node directly from graph"),
			),
			ToolAvailability::Available(Box::<brush_tool::BrushTool>::default()),
			ToolAvailability::ComingSoon(ToolEntry::new(ToolType::Heal, "RasterHealTool").tooltip("Coming Soon: Heal Tool (J)")),
			ToolAvailability::ComingSoon(ToolEntry::new(ToolType::Clone, "RasterCloneTool").tooltip("Coming Soon: Clone Tool (C)")),
			ToolAvailability::ComingSoon(ToolEntry::new(ToolType::Patch, "RasterPatchTool").tooltip("Coming Soon: Patch Tool")),
			ToolAvailability::ComingSoon(ToolEntry::new(ToolType::Detail, "RasterDetailTool").tooltip("Coming Soon: Detail Tool (D)")),
			ToolAvailability::ComingSoon(ToolEntry::new(ToolType::Relight, "RasterRelightTool").tooltip("Coming Soon: Relight Tool (O)")),
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
		ToolMessage::Polygon(_) => ToolType::Polygon,
		ToolMessage::Text(_) => ToolType::Text,

		// Raster tool group
		ToolMessage::Brush(_) => ToolType::Brush,
		// ToolMessage::Heal(_) => ToolType::Heal,
		// ToolMessage::Clone(_) => ToolType::Clone,
		// ToolMessage::Patch(_) => ToolType::Patch,
		// ToolMessage::Detail(_) => ToolType::Detail,
		// ToolMessage::Relight(_) => ToolType::Relight,
		ToolMessage::Imaginate(_) => ToolType::Imaginate,
		_ => panic!("Conversion from ToolMessage to ToolType impossible because the given ToolMessage does not have a matching ToolType. Got: {tool_message:?}"),
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
		ToolType::Polygon => ToolMessageDiscriminant::ActivateToolPolygon,
		ToolType::Text => ToolMessageDiscriminant::ActivateToolText,

		// Raster tool group
		ToolType::Brush => ToolMessageDiscriminant::ActivateToolBrush,
		// ToolType::Heal => ToolMessageDiscriminant::ActivateToolHeal,
		// ToolType::Clone => ToolMessageDiscriminant::ActivateToolClone,
		// ToolType::Patch => ToolMessageDiscriminant::ActivateToolPatch,
		// ToolType::Detail => ToolMessageDiscriminant::ActivateToolDetail,
		// ToolType::Relight => ToolMessageDiscriminant::ActivateToolRelight,
		ToolType::Imaginate => ToolMessageDiscriminant::ActivateToolImaginate,
		_ => panic!("Conversion from ToolType to ToolMessage impossible because the given ToolType does not have a matching ToolMessage. Got: {tool_type:?}"),
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
	/// Draws a prepended "/" symbol which indicates that this is an alternative to a previous hint in the group.
	pub slash: bool,
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
			slash: false,
		}
	}

	pub fn multi_keys(multi_keys: impl IntoIterator<Item = impl IntoIterator<Item = Key>>, label: impl Into<String>) -> Self {
		let key_groups = multi_keys.into_iter().map(|keys| KeysGroup(keys.into_iter().collect()).into()).collect();
		Self {
			key_groups,
			key_groups_mac: None,
			mouse: None,
			label: label.into(),
			plus: false,
			slash: false,
		}
	}

	pub fn mouse(mouse_motion: MouseMotion, label: impl Into<String>) -> Self {
		Self {
			key_groups: vec![],
			key_groups_mac: None,
			mouse: Some(mouse_motion),
			label: label.into(),
			plus: false,
			slash: false,
		}
	}

	pub fn label(label: impl Into<String>) -> Self {
		Self {
			key_groups: vec![],
			key_groups_mac: None,
			mouse: None,
			label: label.into(),
			plus: false,
			slash: false,
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
			slash: false,
		}
	}

	pub fn multi_keys_and_mouse(multi_keys: impl IntoIterator<Item = impl IntoIterator<Item = Key>>, mouse_motion: MouseMotion, label: impl Into<String>) -> Self {
		let key_groups = multi_keys.into_iter().map(|keys| KeysGroup(keys.into_iter().collect()).into()).collect();
		Self {
			key_groups,
			key_groups_mac: None,
			mouse: Some(mouse_motion),
			label: label.into(),
			plus: false,
			slash: false,
		}
	}

	pub fn arrow_keys(label: impl Into<String>) -> Self {
		let multi_keys = [[Key::ArrowUp], [Key::ArrowRight], [Key::ArrowDown], [Key::ArrowLeft]];
		Self::multi_keys(multi_keys, label)
	}

	pub fn prepend_plus(mut self) -> Self {
		self.plus = true;
		self
	}

	pub fn prepend_slash(mut self) -> Self {
		self.slash = true;
		self
	}

	pub fn add_mac_keys(mut self, keys: impl IntoIterator<Item = Key>) -> Self {
		let mac_keys: Vec<_> = keys.into_iter().collect();
		self.key_groups_mac = Some(vec![KeysGroup(mac_keys).into()]);
		self
	}
}
