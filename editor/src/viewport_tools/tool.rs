use super::tools::*;
use crate::communication::message_handler::MessageHandler;
use crate::document::DocumentMessageHandler;
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::widgets::{IconButton, Layout, LayoutGroup, PropertyHolder, Separator, SeparatorDirection, SeparatorType, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::message_prelude::*;

use graphene::color::Color;
use graphene::layers::text_layer::FontCache;

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fmt::{self, Debug};

pub type ToolActionHandlerData<'a> = (&'a DocumentMessageHandler, &'a DocumentToolData, &'a InputPreprocessorMessageHandler, &'a FontCache);

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

#[derive(Clone, Debug)]
pub struct SignalToMessageMap {
	pub document_dirty: Option<ToolMessage>,
	pub selection_changed: Option<ToolMessage>,
	pub tool_abort: Option<ToolMessage>,
}

pub trait ToolTransition {
	fn signal_to_message_map(&self) -> SignalToMessageMap;
	fn activate(&self, responses: &mut VecDeque<Message>) {
		let shared_messages = self.signal_to_message_map();
		if let Some(document_is_dirty_message) = shared_messages.document_dirty {
			responses.push_back(
				BroadcastMessage::SubscribeSignal {
					on: BroadcastSignal::DocumentIsDirty,
					send: Box::new(document_is_dirty_message.into()),
				}
				.into(),
			);
		}

		if let Some(tool_abort_message) = shared_messages.tool_abort {
			responses.push_back(
				BroadcastMessage::SubscribeSignal {
					on: BroadcastSignal::ToolAbort,
					send: Box::new(tool_abort_message.into()),
				}
				.into(),
			);
		}

		if let Some(selection_changed_message) = shared_messages.selection_changed {
			responses.push_back(
				BroadcastMessage::SubscribeSignal {
					on: BroadcastSignal::SelectionChanged,
					send: Box::new(selection_changed_message.into()),
				}
				.into(),
			);
		}
	}

	fn deactivate(&self, responses: &mut VecDeque<Message>) {
		let shared_messages = self.signal_to_message_map();
		if let Some(document_is_dirty_message) = shared_messages.document_dirty {
			responses.push_back(
				BroadcastMessage::UnsubscribeSignal {
					on: BroadcastSignal::DocumentIsDirty,
					message: Box::new(document_is_dirty_message.into()),
				}
				.into(),
			);
		}

		if let Some(tool_abort_message) = shared_messages.tool_abort {
			responses.push_back(
				BroadcastMessage::UnsubscribeSignal {
					on: BroadcastSignal::ToolAbort,
					message: Box::new(tool_abort_message.into()),
				}
				.into(),
			);
		}

		if let Some(selection_changed_message) = shared_messages.selection_changed {
			responses.push_back(
				BroadcastMessage::UnsubscribeSignal {
					on: BroadcastSignal::SelectionChanged,
					message: Box::new(selection_changed_message.into()),
				}
				.into(),
			);
		}
	}
}
pub trait ToolMetadata {
	fn icon_name(&self) -> String;
	fn tooltip(&self) -> String;
	fn tool_type(&self) -> ToolType;
}

pub trait ToolCommon: for<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> + PropertyHolder + ToolTransition + ToolMetadata {}
impl<T> ToolCommon for T where T: for<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> + PropertyHolder + ToolTransition + ToolMetadata {}

type Tool = dyn ToolCommon;

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

#[derive(Debug)]
pub struct ToolBarMetadataGroup {
	pub tooltip: String,
	pub icon_name: String,
	pub tool_type: ToolType,
}

impl PropertyHolder for ToolData {
	fn properties(&self) -> Layout {
		let tool_groups_layout = list_tools_in_groups()
			.iter()
			.map(|tool_group| tool_group.iter().map(|tool| ToolBarMetadataGroup {tooltip: tool.tooltip(), icon_name: tool.icon_name(), tool_type: tool.tool_type()}).collect::<Vec<_>>())
			.chain(coming_soon_tools())
			.flat_map(|group| {
				let separator = std::iter::once(WidgetHolder::new(Widget::Separator(Separator {
					direction: SeparatorDirection::Vertical,
					separator_type: SeparatorType::Section,
				})));
				let buttons = group.into_iter().map(|ToolBarMetadataGroup {tooltip, tool_type, icon_name}| {
					WidgetHolder::new(Widget::IconButton(IconButton {
						icon: icon_name,
						size: 32,
						tooltip: tooltip.clone(),
						active: self.active_tool_type == tool_type,
						on_update: WidgetCallback::new(move |_| {
							if !tooltip.contains("Coming Soon") {
								ToolMessage::ActivateTool { tool_type }.into()
							} else {
								DialogMessage::RequestComingSoonDialog { issue: None }.into()
							}
						}),
						..Default::default()
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

	pub fn swap_colors(&mut self) {
		std::mem::swap(&mut self.document_tool_data.primary_color, &mut self.document_tool_data.secondary_color);
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

pub fn coming_soon_tools() -> Vec<Vec<ToolBarMetadataGroup>> {
	vec![vec![
		ToolBarMetadataGroup {
			tool_type: ToolType::Brush,
			icon_name: "RasterBrushTool".into(),
			tooltip: "Coming Soon: Brush Tool (B)".into(),
		},
		ToolBarMetadataGroup {
			tool_type: ToolType::Heal,
			icon_name: "RasterHealTool".into(),
			tooltip: "Coming Soon: Heal Tool (J)".into(),
		},
		ToolBarMetadataGroup {
			tool_type: ToolType::Clone,
			icon_name: "RasterCloneTool".into(),
			tooltip: "Coming Soon: Clone Tool (C))".into(),
		},
		ToolBarMetadataGroup {
			tool_type: ToolType::Patch,
			icon_name: "RasterPatchTool".into(),
			tooltip: "Coming Soon: Patch Tool".into(),
		},
		ToolBarMetadataGroup {
			tool_type: ToolType::Detail,
			icon_name: "RasterDetailTool".into(),
			tooltip: "Coming Soon: Detail Tool (D)".into(),
		},
		ToolBarMetadataGroup {
			tool_type: ToolType::Relight,
			icon_name: "RasterRelightTool".into(),
			tooltip: "Coming Soon: Relight Tool (O".into(),
		},
	]]
}

pub fn message_to_tool_type(message: &ToolMessage) -> ToolType {
	use ToolMessage::*;

	match message {
		// General tool group
		Select(_) => ToolType::Select,
		Artboard(_) => ToolType::Artboard,
		Navigate(_) => ToolType::Navigate,
		Eyedropper(_) => ToolType::Eyedropper,
		Fill(_) => ToolType::Fill,
		Gradient(_) => ToolType::Gradient,

		// Vector tool group
		Path(_) => ToolType::Path,
		Pen(_) => ToolType::Pen,
		Freehand(_) => ToolType::Freehand,
		Spline(_) => ToolType::Spline,
		Line(_) => ToolType::Line,
		Rectangle(_) => ToolType::Rectangle,
		Ellipse(_) => ToolType::Ellipse,
		Shape(_) => ToolType::Shape,
		Text(_) => ToolType::Text,

		// Raster tool group
		// Brush(_) => ToolType::Brush,
		// Heal(_) => ToolType::Heal,
		// Clone(_) => ToolType::Clone,
		// Patch(_) => ToolType::Patch,
		// Detail(_) => ToolType::Detail,
		// Relight(_) => ToolType::Relight,
		_ => panic!(
			"Conversion from message to tool type impossible because the given ToolMessage does not belong to a tool. Got: {:?}",
			message
		),
	}
}
