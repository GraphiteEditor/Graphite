use super::tools::*;
use crate::communication::message_handler::MessageHandler;
use crate::document::DocumentMessageHandler;
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::widgets::{IconButton, LayoutRow, PropertyHolder, Separator, SeparatorDirection, SeparatorType, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
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

pub trait ToolCommon: for<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> + PropertyHolder {}
impl<T> ToolCommon for T where T: for<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> + PropertyHolder {}

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

impl PropertyHolder for ToolData {
	fn properties(&self) -> WidgetLayout {
		let tool_groups_layout = ToolType::list_tools_in_groups()
			.iter()
			.flat_map(|group| {
				let separator = std::iter::once(WidgetHolder::new(Widget::Separator(Separator {
					direction: SeparatorDirection::Vertical,
					separator_type: SeparatorType::Section,
				})));
				let buttons = group.iter().map(|tool_type| {
					WidgetHolder::new(Widget::IconButton(IconButton {
						icon: tool_type.icon_name(),
						size: 32,
						tooltip: tool_type.tooltip(),
						active: self.active_tool_type == *tool_type,
						on_update: WidgetCallback::new(|_| {
							if !tool_type.tooltip().contains("Coming Soon") {
								ToolMessage::ActivateTool { tool_type: *tool_type }.into()
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

		WidgetLayout {
			layout: vec![LayoutRow::Column { widgets: tool_groups_layout }],
		}
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
				tools: gen_tools_hash_map! {
					// General
					Select => select_tool::SelectTool,
					Artboard => artboard_tool::ArtboardTool,
					Navigate => navigate_tool::NavigateTool,
					Eyedropper => eyedropper_tool::EyedropperTool,
					Fill => fill_tool::FillTool,
					Gradient => gradient_tool::GradientTool,

					// Vector
					Path => path_tool::PathTool,
					Pen => pen_tool::PenTool,
					Freehand => freehand_tool::FreehandTool,
					Spline => spline_tool::SplineTool,
					Line => line_tool::LineTool,
					Rectangle => rectangle_tool::RectangleTool,
					Ellipse => ellipse_tool::EllipseTool,
					Shape => shape_tool::ShapeTool,
					Text => text_tool::TextTool,

					// Raster
					// Brush => brush_tool::BrushTool,
					// Heal => heal_tool::HealTool,
					// Clone => clone_tool:::CloneTool,
					// Patch => patch_tool:::PatchTool,
					// Relight => relight_tool:::RelightTool,
					// Detail => detail_tool:::DetailTool,
				},
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

impl ToolType {
	/// List of all the tools in their conventional ordering and grouping.
	pub fn list_tools_in_groups() -> [&'static [ToolType]; 3] {
		[
			&[
				// General tool group
				ToolType::Select,
				ToolType::Artboard,
				ToolType::Navigate,
				ToolType::Eyedropper,
				ToolType::Fill,
				ToolType::Gradient,
			],
			&[
				// Vector tool group
				ToolType::Path,
				ToolType::Pen,
				ToolType::Freehand,
				ToolType::Spline,
				ToolType::Line,
				ToolType::Rectangle,
				ToolType::Ellipse,
				ToolType::Shape,
				ToolType::Text,
			],
			&[
				// Raster tool group
				ToolType::Brush,
				ToolType::Heal,
				ToolType::Clone,
				ToolType::Patch,
				ToolType::Detail,
				ToolType::Relight,
			],
		]
	}

	pub fn icon_name(&self) -> String {
		match self {
			// General tool group
			ToolType::Select => "GeneralSelectTool".into(),
			ToolType::Artboard => "GeneralArtboardTool".into(),
			ToolType::Navigate => "GeneralNavigateTool".into(),
			ToolType::Eyedropper => "GeneralEyedropperTool".into(),
			ToolType::Fill => "GeneralFillTool".into(),
			ToolType::Gradient => "GeneralGradientTool".into(),

			// Vector tool group
			ToolType::Path => "VectorPathTool".into(),
			ToolType::Pen => "VectorPenTool".into(),
			ToolType::Freehand => "VectorFreehandTool".into(),
			ToolType::Spline => "VectorSplineTool".into(),
			ToolType::Line => "VectorLineTool".into(),
			ToolType::Rectangle => "VectorRectangleTool".into(),
			ToolType::Ellipse => "VectorEllipseTool".into(),
			ToolType::Shape => "VectorShapeTool".into(),
			ToolType::Text => "VectorTextTool".into(),

			// Raster tool group
			ToolType::Brush => "RasterBrushTool".into(),
			ToolType::Heal => "RasterHealTool".into(),
			ToolType::Clone => "RasterCloneTool".into(),
			ToolType::Patch => "RasterPatchTool".into(),
			ToolType::Detail => "RasterDetailTool".into(),
			ToolType::Relight => "RasterRelightTool".into(),
		}
	}

	pub fn tooltip(&self) -> String {
		match self {
			// General tool group
			ToolType::Select => "Select Tool (V)".into(),
			ToolType::Artboard => "Artboard Tool".into(),
			ToolType::Navigate => "Navigate Tool (Z)".into(),
			ToolType::Eyedropper => "Eyedropper Tool (I)".into(),
			ToolType::Fill => "Fill Tool (F)".into(),
			ToolType::Gradient => "Gradient Tool (H)".into(),

			// Vector tool group
			ToolType::Path => "Path Tool (A)".into(),
			ToolType::Pen => "Pen Tool (P)".into(),
			ToolType::Freehand => "Freehand Tool (N)".into(),
			ToolType::Spline => "Spline Tool".into(),
			ToolType::Line => "Line Tool (L)".into(),
			ToolType::Rectangle => "Rectangle Tool (M)".into(),
			ToolType::Ellipse => "Ellipse Tool (E)".into(),
			ToolType::Shape => "Shape Tool (Y)".into(),
			ToolType::Text => "Text Tool (T)".into(),

			// Raster tool group
			ToolType::Brush => "Coming Soon: Brush Tool (B)".into(),
			ToolType::Heal => "Coming Soon: Heal Tool (J)".into(),
			ToolType::Clone => "Coming Soon: Clone Tool (C)".into(),
			ToolType::Patch => "Coming Soon: Patch Tool".into(),
			ToolType::Detail => "Coming Soon: Detail Tool (D)".into(),
			ToolType::Relight => "Coming Soon: Relight Tool (O)".into(),
		}
	}
}

impl fmt::Display for ToolType {
	fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		use ToolType::*;

		let name = match_variant_name!(match (self) {
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
		});

		formatter.write_str(name)
	}
}

pub enum StandardToolMessageType {
	Abort,
	DocumentIsDirty,
	SelectionChanged,
}

// TODO: Find a nicer way in Rust to make this generic so we don't have to manually map to enum variants
pub fn standard_tool_message(tool: ToolType, message_type: StandardToolMessageType) -> Option<ToolMessage> {
	match message_type {
		StandardToolMessageType::DocumentIsDirty => match tool {
			// General tool group
			ToolType::Select => Some(SelectToolMessage::DocumentIsDirty.into()),
			ToolType::Artboard => Some(ArtboardToolMessage::DocumentIsDirty.into()),
			ToolType::Navigate => None,   // Some(NavigateToolMessage::DocumentIsDirty.into()),
			ToolType::Eyedropper => None, // Some(EyedropperToolMessage::DocumentIsDirty.into()),
			ToolType::Fill => None,       // Some(FillToolMessage::DocumentIsDirty.into()),
			ToolType::Gradient => Some(GradientToolMessage::DocumentIsDirty.into()),

			// Vector tool group
			ToolType::Path => Some(PathToolMessage::DocumentIsDirty.into()),
			ToolType::Pen => Some(PenToolMessage::DocumentIsDirty.into()),
			ToolType::Freehand => None,  // Some(FreehandToolMessage::DocumentIsDirty.into()),
			ToolType::Spline => None,    // Some(SplineToolMessage::DocumentIsDirty.into()),
			ToolType::Line => None,      // Some(LineToolMessage::DocumentIsDirty.into()),
			ToolType::Rectangle => None, // Some(RectangleToolMessage::DocumentIsDirty.into()),
			ToolType::Ellipse => None,   // Some(EllipseToolMessage::DocumentIsDirty.into()),
			ToolType::Shape => None,     // Some(ShapeToolMessage::DocumentIsDirty.into()),
			ToolType::Text => Some(TextMessage::DocumentIsDirty.into()),

			// Raster tool group
			ToolType::Brush => None,   // Some(BrushMessage::DocumentIsDirty.into()),
			ToolType::Heal => None,    // Some(HealMessage::DocumentIsDirty.into()),
			ToolType::Clone => None,   // Some(CloneMessage::DocumentIsDirty.into()),
			ToolType::Patch => None,   // Some(PatchMessage::DocumentIsDirty.into()),
			ToolType::Detail => None,  // Some(DetailToolMessage::DocumentIsDirty.into()),
			ToolType::Relight => None, // Some(RelightMessage::DocumentIsDirty.into()),
		},
		StandardToolMessageType::Abort => match tool {
			// General tool group
			ToolType::Select => Some(SelectToolMessage::Abort.into()),
			ToolType::Artboard => Some(ArtboardToolMessage::Abort.into()),
			ToolType::Navigate => Some(NavigateToolMessage::Abort.into()),
			ToolType::Eyedropper => Some(EyedropperToolMessage::Abort.into()),
			ToolType::Fill => Some(FillToolMessage::Abort.into()),
			ToolType::Gradient => Some(GradientToolMessage::Abort.into()),

			// Vector tool group
			ToolType::Path => Some(PathToolMessage::Abort.into()),
			ToolType::Pen => Some(PenToolMessage::Abort.into()),
			ToolType::Freehand => Some(FreehandToolMessage::Abort.into()),
			ToolType::Spline => Some(SplineToolMessage::Abort.into()),
			ToolType::Line => Some(LineToolMessage::Abort.into()),
			ToolType::Rectangle => Some(RectangleToolMessage::Abort.into()),
			ToolType::Ellipse => Some(EllipseToolMessage::Abort.into()),
			ToolType::Shape => Some(ShapeToolMessage::Abort.into()),
			ToolType::Text => Some(TextMessage::Abort.into()),

			// Raster tool group
			ToolType::Brush => None,   // Some(BrushMessage::Abort.into()),
			ToolType::Heal => None,    // Some(HealMessage::Abort.into()),
			ToolType::Clone => None,   // Some(CloneMessage::Abort.into()),
			ToolType::Patch => None,   // Some(PatchMessage::Abort.into()),
			ToolType::Detail => None,  // Some(DetailToolMessage::Abort.into()),
			ToolType::Relight => None, // Some(RelightMessage::Abort.into()),
		},
		StandardToolMessageType::SelectionChanged => match tool {
			ToolType::Path => Some(PathToolMessage::SelectionChanged.into()),
			_ => None,
		},
	}
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
		_ => panic!("Conversion from message to tool type impossible because the given ToolMessage does not belong to a tool"),
	}
}

pub fn update_working_colors(document_data: &DocumentToolData, responses: &mut VecDeque<Message>) {
	responses.push_back(
		FrontendMessage::UpdateWorkingColors {
			primary: document_data.primary_color,
			secondary: document_data.secondary_color,
		}
		.into(),
	);
}
