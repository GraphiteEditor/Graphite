use super::tool_messages::*;
use crate::messages::input_mapper::utility_types::input_keyboard::KeysGroup;
use crate::messages::input_mapper::utility_types::input_keyboard::MouseMotion;
use crate::messages::input_mapper::utility_types::macros::action_keys;
use crate::messages::input_mapper::utility_types::misc::ActionKeys;
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widgets::button_widgets::IconButton;
use crate::messages::layout::utility_types::widgets::input_widgets::SwatchPairInput;
use crate::messages::layout::utility_types::widgets::label_widgets::{Separator, SeparatorDirection, SeparatorType};
use crate::messages::prelude::*;

use graphene::color::Color;
use graphene::layers::text_layer::FontCache;

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fmt::{self, Debug};

pub type ToolActionHandlerData<'a> = (&'a DocumentMessageHandler, &'a DocumentToolData, &'a InputPreprocessorMessageHandler, &'a FontCache);

pub trait ToolCommon: for<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> + PropertyHolder + ToolTransition + ToolMetadata {}
impl<T> ToolCommon for T where T: for<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> + PropertyHolder + ToolTransition + ToolMetadata {}

type Tool = dyn ToolCommon;

pub trait Fsm {
	type ToolData;
	type ToolOptions;

	#[must_use]
	fn transition(self, message: ToolMessage, tool_data: &mut Self::ToolData, transition_data: ToolActionHandlerData, options: &Self::ToolOptions, messages: &mut VecDeque<Message>) -> Self;

	fn update_hints(&self, responses: &mut VecDeque<Message>);
	fn update_cursor(&self, responses: &mut VecDeque<Message>);
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
						icon: "ResetColors".into(),
						tooltip: "Reset".into(),
						tooltip_shortcut: action_keys!(ToolMessageDiscriminant::ResetColors),
						on_update: WidgetCallback::new(|_| ToolMessage::ResetColors.into()),
						..Default::default()
					})),
				],
			},
		]);

		responses.push_back(
			LayoutMessage::SendLayout {
				layout: Layout::WidgetLayout(layout),
				layout_target: LayoutTarget::WorkingColors,
			}
			.into(),
		);
	}
}

#[derive(Clone, Debug)]
pub struct EventToMessageMap {
	pub document_dirty: Option<ToolMessage>,
	pub selection_changed: Option<ToolMessage>,
	pub tool_abort: Option<ToolMessage>,
}

pub trait ToolTransition {
	fn event_to_message_map(&self) -> EventToMessageMap;

	fn activate(&self, responses: &mut VecDeque<Message>) {
		let mut subscribe_message = |broadcast_to_tool_mapping: Option<ToolMessage>, event: BroadcastEvent| {
			if let Some(mapping) = broadcast_to_tool_mapping {
				responses.push_back(
					BroadcastMessage::SubscribeEvent {
						on: event,
						send: Box::new(mapping.into()),
					}
					.into(),
				);
			};
		};

		let event_to_tool_map = self.event_to_message_map();
		subscribe_message(event_to_tool_map.document_dirty, BroadcastEvent::DocumentIsDirty);
		subscribe_message(event_to_tool_map.tool_abort, BroadcastEvent::ToolAbort);
		subscribe_message(event_to_tool_map.selection_changed, BroadcastEvent::SelectionChanged);
	}

	fn deactivate(&self, responses: &mut VecDeque<Message>) {
		let mut unsubscribe_message = |broadcast_to_tool_mapping: Option<ToolMessage>, event: BroadcastEvent| {
			if let Some(mapping) = broadcast_to_tool_mapping {
				responses.push_back(
					BroadcastMessage::UnsubscribeEvent {
						on: event,
						message: Box::new(mapping.into()),
					}
					.into(),
				);
			};
		};

		let event_to_tool_map = self.event_to_message_map();
		unsubscribe_message(event_to_tool_map.document_dirty, BroadcastEvent::DocumentIsDirty);
		unsubscribe_message(event_to_tool_map.tool_abort, BroadcastEvent::ToolAbort);
		unsubscribe_message(event_to_tool_map.selection_changed, BroadcastEvent::SelectionChanged);
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
			.map(|tool_group| tool_group.iter().map(|tool| ToolEntry {
				tooltip: tool.tooltip(),
				tooltip_shortcut: action_keys!(tool_type_to_activate_tool_message(tool.tool_type())),
				icon_name: tool.icon_name(),
				tool_type: tool.tool_type(),
			}).collect::<Vec<_>>())
			.chain(coming_soon_tools())
			.flat_map(|group| {
				let separator = std::iter::once(WidgetHolder::new(Widget::Separator(Separator {
					direction: SeparatorDirection::Vertical,
					separator_type: SeparatorType::Section,
				})));
				let buttons = group.into_iter().map(|ToolEntry { tooltip, tooltip_shortcut, tool_type, icon_name }| {
					WidgetHolder::new(Widget::IconButton(IconButton {
						icon: icon_name,
						size: 32,
						tooltip: tooltip.clone(),
						tooltip_shortcut,
						active: self.active_tool_type == tool_type,
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

#[derive(Debug)]
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
		ToolFsmState {
			tool_data: ToolData {
				active_tool_type: ToolType::Select,
				tools: list_tools_in_groups().into_iter().flatten().map(|tool| (tool.tool_type(), tool)).collect(),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
}

/// List of all the tools in their conventional ordering and grouping.
pub fn list_tools_in_groups() -> Vec<Vec<Box<Tool>>> {
	vec![
		vec![
			// General tool group
			Box::new(select_tool::SelectTool::default()),
			Box::new(artboard_tool::ArtboardTool::default()),
			Box::new(navigate_tool::NavigateTool::default()),
			Box::new(eyedropper_tool::EyedropperTool::default()),
			Box::new(fill_tool::FillTool::default()),
			Box::new(gradient_tool::GradientTool::default()),
		],
		vec![
			// Vector tool group
			Box::new(path_tool::PathTool::default()),
			Box::new(pen_tool::PenTool::default()),
			Box::new(freehand_tool::FreehandTool::default()),
			Box::new(spline_tool::SplineTool::default()),
			Box::new(line_tool::LineTool::default()),
			Box::new(rectangle_tool::RectangleTool::default()),
			Box::new(ellipse_tool::EllipseTool::default()),
			Box::new(shape_tool::ShapeTool::default()),
			Box::new(text_tool::TextTool::default()),
		],
	]
}

pub fn coming_soon_tools() -> Vec<Vec<ToolEntry>> {
	vec![vec![
		ToolEntry {
			tool_type: ToolType::Brush,
			icon_name: "RasterBrushTool".into(),
			tooltip: "Coming Soon: Brush Tool (B)".into(),
			tooltip_shortcut: None,
		},
		ToolEntry {
			tool_type: ToolType::Heal,
			icon_name: "RasterHealTool".into(),
			tooltip: "Coming Soon: Heal Tool (J)".into(),
			tooltip_shortcut: None,
		},
		ToolEntry {
			tool_type: ToolType::Clone,
			icon_name: "RasterCloneTool".into(),
			tooltip: "Coming Soon: Clone Tool (C)".into(),
			tooltip_shortcut: None,
		},
		ToolEntry {
			tool_type: ToolType::Patch,
			icon_name: "RasterPatchTool".into(),
			tooltip: "Coming Soon: Patch Tool".into(),
			tooltip_shortcut: None,
		},
		ToolEntry {
			tool_type: ToolType::Detail,
			icon_name: "RasterDetailTool".into(),
			tooltip: "Coming Soon: Detail Tool (D)".into(),
			tooltip_shortcut: None,
		},
		ToolEntry {
			tool_type: ToolType::Relight,
			icon_name: "RasterRelightTool".into(),
			tooltip: "Coming Soon: Relight Tool (O)".into(),
			tooltip_shortcut: None,
		},
	]]
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
		// ToolMessage::Brush(_) => ToolType::Brush,
		// ToolMessage::Heal(_) => ToolType::Heal,
		// ToolMessage::Clone(_) => ToolType::Clone,
		// ToolMessage::Patch(_) => ToolType::Patch,
		// ToolMessage::Detail(_) => ToolType::Detail,
		// ToolMessage::Relight(_) => ToolType::Relight,
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
		// ToolType::Brush => ToolMessageDiscriminant::ActivateToolBrush,
		// ToolType::Heal => ToolMessageDiscriminant::ActivateToolHeal,
		// ToolType::Clone => ToolMessageDiscriminant::ActivateToolClone,
		// ToolType::Patch => ToolMessageDiscriminant::ActivateToolPatch,
		// ToolType::Detail => ToolMessageDiscriminant::ActivateToolDetail,
		// ToolType::Relight => ToolMessageDiscriminant::ActivateToolRelight,
		_ => panic!(
			"Conversion from ToolType to ToolMessage impossible because the given ToolType does not have a matching ToolMessage. Got: {:?}",
			tool_type
		),
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HintData(pub Vec<HintGroup>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HintGroup(pub Vec<HintInfo>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HintInfo {
	/// A `KeysGroup` specifies all the keys pressed simultaneously to perform an action (like "Ctrl C" to copy).
	/// Usually at most one is given, but less commonly, multiple can be used to describe additional hotkeys not used simultaneously (like the four different arrow keys to nudge a layer).
	#[serde(rename = "keyGroups")]
	pub key_groups: Vec<KeysGroup>,
	/// `None` means that the regular `key_groups` should be used for all platforms, `Some` is an override for a Mac-only input hint.
	#[serde(rename = "keyGroupsMac")]
	pub key_groups_mac: Option<Vec<KeysGroup>>,
	/// An optional `MouseMotion` that can indicate the mouse action, like which mouse button is used and whether a drag occurs.
	/// No such icon is shown if `None` is given, and it can be combined with `key_groups` if desired.
	pub mouse: Option<MouseMotion>,
	/// The text describing what occurs with this input combination.
	pub label: String,
	/// Draws a prepended "+" symbol which indicates that this is a refinement upon a previous hint in the group.
	pub plus: bool,
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
