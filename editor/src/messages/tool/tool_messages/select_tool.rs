#![allow(clippy::too_many_arguments)]

use super::tool_prelude::*;
use crate::consts::*;
use crate::messages::input_mapper::utility_types::pointer::ViewportPosition;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::misc::{AlignAggregate, AlignAxis, FlipAxis, GroupFolderType};
use crate::messages::portfolio::document::utility_types::network_interface::{FlowType, NodeNetworkInterface, NodeTemplate};
use crate::messages::portfolio::document::utility_types::nodes::SelectedNodes;
use crate::messages::preferences::SelectionMode;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{
	DrawingToolState, apply_fill_color_pick, apply_fill_enabled, apply_stroke_color_pick, apply_stroke_enabled, apply_working_colors, has_paintable_selection, swap_fill_and_stroke, sync_drawing_state,
};
use crate::messages::tool::common_functionality::compass_rose::{Axis, CompassRose};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::measure;
use crate::messages::tool::common_functionality::pivot::{Pivot, PivotGizmo, PivotGizmoType, PivotToolSource, pin_pivot_widget, pivot_gizmo_type_widget, pivot_reference_point_widget};
use crate::messages::tool::common_functionality::shape_editor::SelectionShapeType;
use crate::messages::tool::common_functionality::snap_session::{BoxFeatures, FeatureMap, Locus, Moving, PointOptions, SnapSession, SnapView, snapping_hints};
use crate::messages::tool::common_functionality::stroke_options::{StrokeOptionsUpdate, apply_stroke_option, create_stroke_options_popover_widget};
use crate::messages::tool::common_functionality::transformation_cage::*;
use crate::messages::tool::common_functionality::utility_functions::{push_nearest_anchors, resize_bounds, rotate_bounds, skew_bounds, text_bounding_box, transforming_transform_cage};
use glam::DMat2;
use graph_craft::document::NodeId;
use graphene_std::Color;
use graphene_std::renderer::Quad;
use graphene_std::renderer::Rect;
use graphene_std::transform::ReferencePoint;
use graphene_std::vector::algorithms::shapes::polyline_bezpath;
use graphene_std::vector::misc::BooleanOperation;
use graphene_std::vector::style::FillChoice;
use std::fmt;

#[derive(ExtractField)]
pub struct SelectTool {
	fsm_state: SelectToolFsmState,
	tool_data: SelectToolData,
	drawing: DrawingToolState,
}

impl Default for SelectTool {
	fn default() -> Self {
		Self {
			fsm_state: SelectToolFsmState::default(),
			tool_data: SelectToolData::default(),
			drawing: DrawingToolState::new(true),
		}
	}
}

#[allow(dead_code)]
#[derive(Default)]
pub struct SelectOptions {
	nested_selection_behavior: NestedSelectionBehavior,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum SelectOptionsUpdate {
	NestedSelectionBehavior(NestedSelectionBehavior),
	PivotGizmoType(PivotGizmoType),
	SetPivotGizmoEnabled(bool),
	TogglePivotPinned,
	FillColor(FillChoice),
	FillEnabled(bool),
	StrokeColor(Option<Color>),
	StrokeEnabled(bool),
	SwapFillAndStroke,
	StrokeOption(StrokeOptionsUpdate),
	WorkingColorsChanged,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, serde::Serialize, serde::Deserialize)]
pub enum NestedSelectionBehavior {
	#[default]
	Shallowest,
	Deepest,
}

impl fmt::Display for NestedSelectionBehavior {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			NestedSelectionBehavior::Deepest => write!(f, "Deep Select"),
			NestedSelectionBehavior::Shallowest => write!(f, "Shallow Select"),
		}
	}
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SelectToolPointerKeys {
	pub axis_align: Key,
	pub snap_angle: Key,
	pub center: Key,
	pub duplicate: Key,
}

#[impl_message(Message, ToolMessage, Select)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum SelectToolMessage {
	// Standard messages
	Abort,
	SelectionChanged,
	WorkingColorChanged,
	Overlays {
		context: OverlayContext,
	},

	// Tool-specific messages
	DragStart {
		extend_selection: Key,
		remove_from_selection: Key,
		select_deepest: Key,
		lasso_select: Key,
		skew: Key,
	},
	DragStop {
		remove_from_selection: Key,
	},
	EditLayer,
	EditLayerExec,
	Enter,
	PointerMove {
		modifier_keys: SelectToolPointerKeys,
	},
	PointerOutsideViewport {
		modifier_keys: SelectToolPointerKeys,
	},
	SelectOptions {
		options: SelectOptionsUpdate,
	},
	SetPivot {
		position: ReferencePoint,
	},
	SyncHistory,
	ShiftSelectedNodes {
		offset: DVec2,
	},
	PivotShift {
		offset: Option<DVec2>,
		flush: bool,
	},
}

impl ToolMetadata for SelectTool {
	fn icon_name(&self) -> String {
		"GeneralSelectTool".into()
	}
	fn tooltip_label(&self) -> String {
		"Select Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Select
	}
	fn default_line_weight(&self) -> Option<f64> {
		Some(self.drawing.default_line_weight)
	}
}

impl ToolEditTarget for SelectTool {
	fn edit_target(&self) -> EditTarget {
		EditTarget::transform()
	}
}

impl SelectTool {
	fn deep_selection_widget(&self) -> WidgetInstance {
		let layer_selection_behavior_entries = [NestedSelectionBehavior::Shallowest, NestedSelectionBehavior::Deepest]
			.iter()
			.map(|mode| {
				MenuListEntry::new(format!("{mode:?}")).label(mode.to_string()).on_commit(move |_| {
					SelectToolMessage::SelectOptions {
						options: SelectOptionsUpdate::NestedSelectionBehavior(*mode),
					}
					.into()
				})
			})
			.collect();

		DropdownInput::new(vec![layer_selection_behavior_entries])
			.selected_index(Some((self.tool_data.nested_selection_behavior == NestedSelectionBehavior::Deepest) as u32))
			.tooltip_label("Selection Mode")
			.tooltip_description(
				"Shallow Select: clicks initially select the least-nested layers and double clicks drill deeper into the folder hierarchy.\n\
				Deep Select: clicks directly select the most-nested layers in the folder hierarchy.",
			)
			.widget_instance()
	}

	fn alignment_widgets(&self, disabled: bool) -> impl Iterator<Item = WidgetInstance> + use<> {
		[AlignAxis::X, AlignAxis::Y]
			.into_iter()
			.flat_map(|axis| [(axis, AlignAggregate::Min), (axis, AlignAggregate::Center), (axis, AlignAggregate::Max)])
			.map(move |(axis, aggregate)| {
				let (icon, label) = match (axis, aggregate) {
					(AlignAxis::X, AlignAggregate::Min) => ("AlignLeft", "Align Left"),
					(AlignAxis::X, AlignAggregate::Center) => ("AlignHorizontalCenter", "Align Horizontal Center"),
					(AlignAxis::X, AlignAggregate::Max) => ("AlignRight", "Align Right"),
					(AlignAxis::Y, AlignAggregate::Min) => ("AlignTop", "Align Top"),
					(AlignAxis::Y, AlignAggregate::Center) => ("AlignVerticalCenter", "Align Vertical Center"),
					(AlignAxis::Y, AlignAggregate::Max) => ("AlignBottom", "Align Bottom"),
				};
				IconButton::new(icon, 24)
					.tooltip_label(label)
					.on_update(move |_| DocumentMessage::AlignSelectedLayers { axis, aggregate }.into())
					.disabled(disabled)
					.widget_instance()
			})
	}

	fn flip_widgets(&self, disabled: bool) -> impl Iterator<Item = WidgetInstance> + use<> {
		[(FlipAxis::X, "FlipHorizontal", "Flip Horizontal"), (FlipAxis::Y, "FlipVertical", "Flip Vertical")]
			.into_iter()
			.map(move |(flip_axis, icon, label)| {
				IconButton::new(icon, 24)
					.tooltip_label(label)
					.on_update(move |_| DocumentMessage::FlipSelectedLayers { flip_axis }.into())
					.disabled(disabled)
					.widget_instance()
			})
	}

	fn boolean_widgets(&self, selected_count: usize) -> impl Iterator<Item = WidgetInstance> + use<> {
		let list = <BooleanOperation as graphene_std::choice_type::ChoiceTypeStatic>::list();
		list.iter().flat_map(|i| i.iter()).map(move |(operation, info)| {
			IconButton::new(info.icon.unwrap(), 24)
				.tooltip_label(info.label)
				.tooltip_description(info.description.unwrap_or_default())
				.disabled(selected_count == 0)
				.on_update(move |_| {
					let group_folder_type = GroupFolderType::BooleanOperation(*operation);
					DocumentMessage::GroupSelectedLayers { group_folder_type }.into()
				})
				.widget_instance()
		})
	}
}

impl LayoutHolder for SelectTool {
	fn layout(&self) -> Layout {
		let mut widgets = Vec::new();

		// Fill/Stroke widget set (only shown when there's a paintable selection to apply edits to)
		if self.tool_data.paintable_layers_selected {
			widgets.append(&mut self.drawing.fill.create_widgets(
				"Fill:",
				|checkbox: &CheckboxInput| {
					SelectToolMessage::SelectOptions {
						options: SelectOptionsUpdate::FillEnabled(checkbox.checked),
					}
					.into()
				},
				|color: &ColorInput| {
					SelectToolMessage::SelectOptions {
						options: SelectOptionsUpdate::FillColor(FillChoice::from(&color.value)),
					}
					.into()
				},
			));

			widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
			widgets.push(
				IconButton::new("SwapHorizontal", 16)
					.tooltip_label("Swap Fill/Stroke Colors")
					.tooltip_shortcut(action_shortcut!(ToolMessageDiscriminant::SwapFillAndStroke))
					.on_update(|_| {
						SelectToolMessage::SelectOptions {
							options: SelectOptionsUpdate::SwapFillAndStroke,
						}
						.into()
					})
					.widget_instance(),
			);
			widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());

			widgets.append(&mut self.drawing.stroke.create_widgets(
				"Stroke:",
				|checkbox: &CheckboxInput| {
					SelectToolMessage::SelectOptions {
						options: SelectOptionsUpdate::StrokeEnabled(checkbox.checked),
					}
					.into()
				},
				|color: &ColorInput| {
					SelectToolMessage::SelectOptions {
						options: SelectOptionsUpdate::StrokeColor(color.value.as_solid().map(Color::from)),
					}
					.into()
				},
			));

			let weight_disabled = self.drawing.stroke.enabled == Some(false);
			widgets.push(create_stroke_options_popover_widget(&self.drawing, weight_disabled, |update| {
				SelectToolMessage::SelectOptions {
					options: SelectOptionsUpdate::StrokeOption(update),
				}
				.into()
			}));

			widgets.push(Separator::new(SeparatorStyle::Section).widget_instance());
		}

		// Select mode (Deep/Shallow)
		widgets.push(self.deep_selection_widget());

		// Pivot gizmo type (checkbox + dropdown for pivot/origin)
		widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
		widgets.extend(pivot_gizmo_type_widget(self.tool_data.pivot_gizmo.state, PivotToolSource::Select));

		if self.tool_data.pivot_gizmo.state.is_pivot() {
			// Nine-position reference point widget
			widgets.push(Separator::new(SeparatorStyle::Related).widget_instance());
			widgets.push(pivot_reference_point_widget(
				self.tool_data.selected_layers_count == 0 || !self.tool_data.pivot_gizmo.state.is_pivot(),
				self.tool_data.pivot_gizmo.pivot.to_pivot_position(),
				PivotToolSource::Select,
			));

			// Pivot pin button
			widgets.push(Separator::new(SeparatorStyle::Related).widget_instance());

			let pin_active = self.tool_data.pivot_gizmo.pin_active();
			let pin_enabled = self.tool_data.pivot_gizmo.pivot.old_pivot_position == ReferencePoint::None && self.tool_data.pivot_gizmo.state.enabled;

			if pin_active || pin_enabled {
				widgets.push(pin_pivot_widget(pin_active, pin_enabled, PivotToolSource::Select));
			}
		}

		// Align
		let disabled = self.tool_data.selected_layers_count < 2;
		widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
		widgets.extend(self.alignment_widgets(disabled));

		// Flip
		let disabled = self.tool_data.selected_layers_count == 0;
		widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
		widgets.extend(self.flip_widgets(disabled));

		// Boolean
		widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
		widgets.extend(self.boolean_widgets(self.tool_data.selected_layers_count));

		Layout(vec![LayoutGroup::row(widgets)])
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for SelectTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		let mut redraw_reference_pivot = false;
		let mut drawing_options_changed = false;

		if matches!(&message, ToolMessage::Select(SelectToolMessage::SelectionChanged)) && sync_drawing_state(&mut self.drawing, true, true, context.global_tool_data, context.document) {
			self.send_layout(responses, LayoutTarget::ToolOptions);
		}

		if matches!(&message, ToolMessage::Select(SelectToolMessage::WorkingColorChanged)) {
			responses.add(SelectToolMessage::SelectOptions {
				options: SelectOptionsUpdate::WorkingColorsChanged,
			});
		}

		if let ToolMessage::Select(SelectToolMessage::SelectOptions { options: option_update }) = &message {
			match option_update {
				SelectOptionsUpdate::NestedSelectionBehavior(nested_selection_behavior) => {
					self.tool_data.nested_selection_behavior = *nested_selection_behavior;
					responses.add(ToolMessage::UpdateHints);
				}
				SelectOptionsUpdate::PivotGizmoType(gizmo_type) => {
					if self.tool_data.pivot_gizmo.state.enabled {
						self.tool_data.pivot_gizmo.state.gizmo_type = *gizmo_type;
						responses.add(ToolMessage::UpdateHints);
						let pivot_gizmo = self.tool_data.pivot_gizmo();
						responses.add(TransformLayerMessage::SetPivotGizmo { pivot_gizmo });
						responses.add(NodeGraphMessage::RunDocumentGraph);
						redraw_reference_pivot = true;
					}
				}
				SelectOptionsUpdate::SetPivotGizmoEnabled(enabled) => {
					self.tool_data.pivot_gizmo.state.enabled = *enabled;
					responses.add(ToolMessage::UpdateHints);
					responses.add(NodeGraphMessage::RunDocumentGraph);
					redraw_reference_pivot = true;
				}
				SelectOptionsUpdate::TogglePivotPinned => {
					self.tool_data.pivot_gizmo.pivot.pinned = !self.tool_data.pivot_gizmo.pivot.pinned;
					responses.add(ToolMessage::UpdateHints);
					responses.add(NodeGraphMessage::RunDocumentGraph);
					redraw_reference_pivot = true;
				}
				SelectOptionsUpdate::FillColor(fill_choice) => {
					apply_fill_color_pick(&mut self.drawing, fill_choice.clone(), context.document, responses);
					drawing_options_changed = true;
				}
				SelectOptionsUpdate::FillEnabled(enabled) => {
					apply_fill_enabled(&mut self.drawing, *enabled, context.global_tool_data, context.document, responses);
					drawing_options_changed = true;
				}
				SelectOptionsUpdate::StrokeColor(color) => {
					apply_stroke_color_pick(&mut self.drawing, *color, context.document, responses);
					drawing_options_changed = true;
				}
				SelectOptionsUpdate::StrokeEnabled(enabled) => {
					apply_stroke_enabled(&mut self.drawing, *enabled, context.global_tool_data, context.document, responses);
					drawing_options_changed = true;
				}
				SelectOptionsUpdate::SwapFillAndStroke => {
					swap_fill_and_stroke(&mut self.drawing, context.document, responses);
					drawing_options_changed = true;
				}
				SelectOptionsUpdate::StrokeOption(update) => {
					apply_stroke_option(&mut self.drawing, update.clone(), context.document, responses);
					drawing_options_changed = true;
				}
				SelectOptionsUpdate::WorkingColorsChanged => {
					apply_working_colors(&mut self.drawing, context.global_tool_data, context.document);
					drawing_options_changed = true;
				}
			}
		}

		let previous_state = self.fsm_state;
		self.fsm_state.process_event(message, &mut self.tool_data, context, &(), responses, false);

		// However one of these drags ends, its snapping gesture ends with it
		let snapping = matches!(
			previous_state,
			SelectToolFsmState::Dragging { .. }
				| SelectToolFsmState::ResizingBounds
				| SelectToolFsmState::SkewingBounds { .. }
				| SelectToolFsmState::RotatingBounds
				| SelectToolFsmState::DraggingPivot
		);
		if snapping && std::mem::discriminant(&previous_state) != std::mem::discriminant(&self.fsm_state) {
			self.tool_data.slide = None;
			if let Some(bounds) = &mut self.tool_data.bounding_box_manager {
				(bounds.turning, bounds.skewing) = (None, None);
			}
			context.snap_session.end_gesture();
			responses.add(OverlaysMessage::Draw);
		}

		if self.tool_data.pivot_gizmo.pivot.should_refresh_pivot_position() || self.tool_data.selected_layers_changed || redraw_reference_pivot || drawing_options_changed {
			// Send the layout containing the updated pivot position (a bit ugly to do it here not in the fsm but that doesn't have SelectTool)
			self.send_layout(responses, LayoutTarget::ToolOptions);
			self.tool_data.selected_layers_changed = false;
		}
	}

	fn actions(&self) -> ActionList {
		let mut common = actions!(SelectToolMessageDiscriminant;
			PointerMove,
			Abort,
			EditLayer,
			EditLayerExec,
			Enter,
		);

		let additional = match self.fsm_state {
			SelectToolFsmState::Ready { .. } => actions!(SelectToolMessageDiscriminant; DragStart),
			_ => actions!(SelectToolMessageDiscriminant; DragStop),
		};
		common.extend(additional);

		common
	}
}

impl ToolTransition for SelectTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(SelectToolMessage::Abort.into()),
			selection_changed: Some(SelectToolMessage::SelectionChanged.into()),
			graph_changed: Some(SelectToolMessage::SelectionChanged.into()),
			working_color_changed: Some(SelectToolMessage::WorkingColorChanged.into()),
			overlay_provider: Some(|context| SelectToolMessage::Overlays { context }.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SelectToolFsmState {
	Ready {
		selection: NestedSelectionBehavior,
	},
	Drawing {
		selection_shape: SelectionShapeType,
		has_drawn: bool,
	},
	Dragging {
		axis: Axis,
		using_compass: bool,
		has_dragged: bool,
		deepest: bool,
		remove: bool,
	},
	ResizingBounds,
	SkewingBounds {
		skew: Key,
	},
	RotatingBounds,
	DraggingPivot,
}

impl Default for SelectToolFsmState {
	fn default() -> Self {
		let selection = NestedSelectionBehavior::Deepest;
		SelectToolFsmState::Ready { selection }
	}
}

/// The layers being dragged, which slide together by however far the pointer asks from where it took hold.
#[derive(Clone, Debug)]
struct Slide {
	/// Where the pointer took hold, in document space.
	grip: DVec2,
	/// The corners, the middles of the edges, and the center of the box around the dragged layers, then some of their anchors, each from where it began in document space.
	features: Vec<FeatureMap>,
	/// The corner nearest where the pointer took hold, which is the feature that lands on the grid.
	lead: usize,
	/// Which of the features make up the box, if the dragged layers have one.
	box_features: Option<BoxFeatures>,
	/// How far the dragged layers have been moved so far, in document space.
	slid: DVec2,
	/// Whether the copies being dragged have yet to render.
	copies_pending: bool,
}

#[derive(Clone, Debug, Default)]
struct SelectToolData {
	drag_start: ViewportPosition,
	drag_current: ViewportPosition,
	slide: Option<Slide>,
	lasso_polygon: Vec<ViewportPosition>,
	selection_mode: Option<SelectionMode>,
	layers_dragging: Vec<LayerNodeIdentifier>, // Unordered, often used as temporary buffer
	ordered_layers: Vec<LayerNodeIdentifier>,  // Ordered list of layers
	layer_selected_on_start: Option<LayerNodeIdentifier>,
	select_single_layer: Option<LayerNodeIdentifier>,
	axis_align: bool,
	non_duplicated_layers: Option<Vec<LayerNodeIdentifier>>,
	bounding_box_manager: Option<BoundingBoxManager>,
	cursor: MouseCursorIcon,
	pivot_gizmo: PivotGizmo,
	/// The pivot as it was when a drag of it began, restored if the drag is aborted.
	pivot_before_drag: Option<Pivot>,
	pivot_gizmo_start: Option<DVec2>,
	pivot_gizmo_shift: Option<DVec2>,
	compass_rose: CompassRose,
	line_center: DVec2,
	skew_edge: EdgeBool,
	nested_selection_behavior: NestedSelectionBehavior,
	selected_layers_count: usize,
	/// Whether any selected layer can take paint, controlling the visibility of the fill/stroke widget row.
	paintable_layers_selected: bool,
	selected_layers_changed: bool,
	auto_panning: AutoPanning,
}

impl SelectToolData {
	/// Readies the dragged layers to slide together from where the pointer took hold.
	fn prepare_slide(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler) {
		let grip = document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position);
		let mut slide = Slide {
			grip,
			features: Vec::new(),
			lead: 0,
			box_features: None,
			slid: DVec2::ZERO,
			copies_pending: false,
		};

		let Some(corners) = self.dragged_box(document) else {
			self.slide = Some(slide);
			return;
		};

		let middles = [0, 1, 2, 3].map(|i| (corners[i] + corners[(i + 1) % 4]) / 2.);
		let mut starts: Vec<DVec2> = corners.into_iter().chain(middles).collect();
		starts.push(corners.iter().sum::<DVec2>() / 4.);

		// Along each document axis, the corner furthest toward either end carries that side of the box
		let side = |axis: usize, toward_high: bool| {
			let along = |&index: &usize| if toward_high { corners[index][axis] } else { -corners[index][axis] };
			(0..4).max_by(|a, b| along(a).total_cmp(&along(b)))
		};
		slide.box_features = Some(BoxFeatures {
			corners: [0, 1, 2, 3],
			sides: [0, 1].map(|axis| [side(axis, false), side(axis, true)]),
		});
		slide.lead = (0..4)
			.min_by(|&a, &b| corners[a].distance_squared(grip).total_cmp(&corners[b].distance_squared(grip)))
			.unwrap_or_default();

		push_nearest_anchors(&mut starts, document, &self.layers_dragging, grip);

		slide.features = starts.into_iter().map(FeatureMap::sliding_from).collect();
		self.slide = Some(slide);
	}

	/// The corners of the box the transform cage draws around the dragged layers, in document space and in order around it.
	fn dragged_box(&self, document: &DocumentMessageHandler) -> Option<[DVec2; 4]> {
		let metadata = document.metadata();
		let viewport_to_document = metadata.rendered_document_to_viewport.inverse();
		let artwork: Vec<_> = self.layers_dragging.iter().copied().filter(not_artboard(document)).collect();

		// The cage is upright in the frame of the first layer it surrounds, which turns with a turned selection
		let frame = artwork
			.first()
			.map(|&layer| metadata.transform_to_viewport_with_first_transform_node_if_group(layer, &document.network_interface))
			.filter(|frame| frame.matrix2.determinant() != 0.);
		if let Some(frame) = frame
			&& let Some(bounds) = artwork
				.iter()
				.filter_map(|&layer| metadata.bounding_box_with_transform(layer, frame.inverse() * metadata.transform_to_viewport(layer)))
				.reduce(Quad::combine_bounds)
		{
			return Some((viewport_to_document * frame * Quad::from_box(bounds)).0);
		}

		// Without a cage to follow, the upright box around them in the document
		let as_rendered = |layer: LayerNodeIdentifier| metadata.bounding_box_with_transform(layer, metadata.transform_to_document_as_rendered(layer));
		let bounds = self.layers_dragging.iter().copied().filter_map(as_rendered).reduce(Quad::combine_bounds)?;
		Some(Quad::from_box(bounds).0)
	}

	/// Starts the snapping over to suit what is being dragged, as when a drag begins or Alt swaps copies in or out.
	fn begin_snapping_gesture(&mut self, snap_session: &mut SnapSession, view: SnapView, removed_copies: &[LayerNodeIdentifier]) {
		let metadata = view.document.metadata();
		let dragging_copies = self.non_duplicated_layers.is_some();
		let copies_pending = dragging_copies && !self.layers_dragging.iter().all(|&copy| metadata.click_targets(copy).is_some());
		if let Some(slide) = &mut self.slide {
			slide.copies_pending = copies_pending;
		}

		// Until the next render, the scene still shows copies being taken away, and originals where their new copies are
		let originals = self.non_duplicated_layers.clone().unwrap_or_default();
		let mut layers = self.layers_dragging.clone();
		layers.extend(removed_copies);
		if copies_pending {
			layers.extend(&originals);
		}

		let moving = Moving {
			layers,
			copied: originals,
			..Default::default()
		};

		// Once Alt has swapped layers in or out, the scene must take in the renders that show where they now are
		if dragging_copies || !removed_copies.is_empty() {
			snap_session.begin_gesture_following_edits(view, moving);
		} else {
			snap_session.begin_gesture(view, moving);
		}
	}

	/// Slides the dragged layers by however far the pointer asks from where it took hold, and returns how far they move this time in the viewport.
	fn slide_dragged_layers(&mut self, snap_session: &mut SnapSession, view: SnapView, held_direction: Option<DVec2>) -> DVec2 {
		let metadata = view.document.metadata();
		let document_to_viewport = metadata.document_to_viewport;
		let Some(slide) = &mut self.slide else { return DVec2::ZERO };

		let pointer = document_to_viewport.inverse().transform_point2(view.input.mouse.position);
		let free = pointer - slide.grip;

		// A held direction keeps the slide on its line through where the drag began, where it can still stop at what the line crosses
		let required = held_direction.and_then(|direction| Locus::line(DVec2::ZERO, document_to_viewport.inverse().transform_vector2(direction)));

		let slid = if !slide.features.is_empty() {
			let options = PointOptions {
				required,
				box_features: slide.box_features,
				..Default::default()
			};
			snap_session.snap_features(view, &slide.features, slide.lead, free, options)
		} else {
			required.map_or(free, |line| line.nearest(free))
		};

		// Only what has not yet been applied moves the layers, so the total stays right however the canvas has moved since
		let moved = slid - slide.slid;
		slide.slid = slid;
		self.drag_current = document_to_viewport.transform_point2(slide.grip + slid);

		// Moves in the viewport are read through the view the latest render was made with, which can lag a zoom or tilt
		metadata.rendered_document_to_viewport.transform_vector2(moved)
	}

	/// Puts the drag's viewport positions where the canvas now shows where it began and where it has slid to.
	fn follow_canvas(&mut self, document: &DocumentMessageHandler) {
		let Some(slide) = &self.slide else { return };
		let document_to_viewport = document.metadata().document_to_viewport;

		self.drag_start = document_to_viewport.transform_point2(slide.grip);
		self.drag_current = document_to_viewport.transform_point2(slide.grip + slide.slid);
	}

	/// The center of the combined viewport-space bounding box of the layers currently being dragged.
	fn dragging_layers_center(&self, document: &DocumentMessageHandler) -> Option<ViewportPosition> {
		self.layers_dragging
			.iter()
			.filter_map(|&layer| document.metadata().bounding_box_viewport(layer).map(Rect::from_box))
			.reduce(Rect::combine_bounds)
			.map(|bounds| bounds.center())
	}

	pub fn selection_quad(&self) -> Quad {
		let bbox = self.selection_box();
		Quad::from_box(bbox)
	}

	pub fn calculate_selection_mode_from_direction(&mut self) -> SelectionMode {
		let bbox: [DVec2; 2] = self.selection_box();
		let above_threshold = bbox[1].distance_squared(bbox[0]) > DRAG_DIRECTION_MODE_DETERMINATION_THRESHOLD.powi(2);

		if self.selection_mode.is_none() && above_threshold {
			let mode = if bbox[1].x < bbox[0].x {
				SelectionMode::Touched
			} else {
				// This also covers the case where they're equal: the area is zero, so we use `Enclosed` to ensure the selection ends up empty, as nothing will be enclosed by an empty area
				SelectionMode::Enclosed
			};
			self.selection_mode = Some(mode);
		}

		self.selection_mode.unwrap_or(SelectionMode::Touched)
	}

	pub fn selection_box(&self) -> [DVec2; 2] {
		if self.drag_current == self.drag_start {
			let tolerance = DVec2::splat(SELECTION_TOLERANCE);
			[self.drag_start - tolerance, self.drag_start + tolerance]
		} else {
			[self.drag_start, self.drag_current]
		}
	}

	pub fn intersect_lasso_no_artboards(&self, document: &DocumentMessageHandler, viewport: &ViewportMessageHandler) -> Vec<LayerNodeIdentifier> {
		if self.lasso_polygon.len() < 2 {
			return Vec::new();
		}
		let polygon = polyline_bezpath(self.lasso_polygon.iter().copied(), true);
		document.intersect_polygon_no_artboards(polygon, viewport).collect()
	}

	pub fn is_layer_inside_lasso_polygon(&self, layer: &LayerNodeIdentifier, document: &DocumentMessageHandler, viewport: &ViewportMessageHandler) -> bool {
		if self.lasso_polygon.len() < 2 {
			return false;
		}
		let polygon = polyline_bezpath(self.lasso_polygon.iter().copied(), true);
		document.is_layer_fully_inside_polygon(layer, viewport, polygon)
	}

	/// Duplicates the currently dragging layers. Called when Alt is pressed and the layers have not yet been duplicated.
	fn start_duplicates(&mut self, document: &mut DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		self.non_duplicated_layers = Some(self.layers_dragging.clone());
		let mut new_dragging = Vec::new();

		// Get the shallowest unique layers and sort by their index relative to parent for ordered processing
		let mut layers = document.network_interface.shallowest_unique_layers(&[]).collect::<Vec<_>>();

		layers.sort_by_key(|layer| {
			let Some(parent) = layer.parent(document.metadata()) else { return usize::MAX };
			DocumentMessageHandler::get_calculated_insert_index(document.metadata(), &SelectedNodes(vec![layer.to_node()]), parent)
		});

		for layer in layers.into_iter().rev() {
			let Some(parent) = layer.parent(document.metadata()) else { continue };

			// Moves the layer back to its starting position.
			responses.add(GraphOperationMessage::TransformChange {
				layer,
				transform: DAffine2::from_translation(self.drag_start - self.drag_current),
				transform_in: TransformIn::Viewport,
				skip_rerender: true,
			});

			// Copy the layer
			let mut copy_ids = HashMap::new();
			let node_id = layer.to_node();
			copy_ids.insert(node_id, NodeId(0));

			document
				.network_interface
				.upstream_flow_back_from_nodes(vec![layer.to_node()], &[], FlowType::LayerChildrenUpstreamFlow)
				.enumerate()
				.for_each(|(index, node_id)| {
					copy_ids.insert(node_id, NodeId((index + 1) as u64));
				});

			let nodes = document.network_interface.copy_nodes(&copy_ids, &[]).collect::<Vec<(NodeId, NodeTemplate)>>();

			let insert_index = DocumentMessageHandler::get_calculated_insert_index(document.metadata(), &SelectedNodes(vec![layer.to_node()]), parent);

			let new_ids: HashMap<_, _> = nodes.iter().map(|(id, _)| (*id, NodeId::new())).collect();

			let layer_id = *new_ids.get(&NodeId(0)).expect("Node Id 0 should be a layer");
			let layer = LayerNodeIdentifier::new_unchecked(layer_id);
			new_dragging.push(layer);
			responses.add(NodeGraphMessage::AddNodes { nodes, new_ids });
			responses.add(NodeGraphMessage::MoveLayerToStack { layer, parent, insert_index });
		}
		let nodes = new_dragging.iter().map(|layer| layer.to_node()).collect();
		responses.add(NodeGraphMessage::SelectedNodesSet { nodes });
		responses.add(NodeGraphMessage::RunDocumentGraph);
		self.layers_dragging = new_dragging;
	}

	/// Removes the duplicated layers. Called when Alt is released and the layers have previously been duplicated.
	fn stop_duplicates(&mut self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let Some(original) = self.non_duplicated_layers.take() else {
			return;
		};

		// Delete the duplicated layers
		for layer in document.network_interface.shallowest_unique_layers(&[]) {
			responses.add(NodeGraphMessage::DeleteNodes {
				node_ids: vec![layer.to_node()],
				delete_children: true,
			});
		}

		for &layer in &original {
			responses.add(GraphOperationMessage::TransformChange {
				layer,
				transform: DAffine2::from_translation(self.drag_current - self.drag_start),
				transform_in: TransformIn::Viewport,
				skip_rerender: true,
			});
		}
		let nodes = original
			.iter()
			.filter_map(|layer| {
				if *layer != LayerNodeIdentifier::ROOT_PARENT {
					Some(layer.to_node())
				} else {
					log::error!("ROOT_PARENT cannot be part of non_duplicated_layers");
					None
				}
			})
			.collect();
		responses.add(NodeGraphMessage::SelectedNodesSet { nodes });
		responses.add(NodeGraphMessage::RunDocumentGraph);
		responses.add(NodeGraphMessage::SelectedNodesUpdated);
		responses.add(NodeGraphMessage::SendGraph);
		self.layers_dragging = original;
	}

	fn state_from_pivot_gizmo(&self, mouse: DVec2) -> Option<SelectToolFsmState> {
		match self.pivot_gizmo.state.gizmo_type {
			PivotGizmoType::Pivot if self.pivot_gizmo.state.is_pivot() => self.pivot_gizmo.pivot.is_over(mouse).then_some(SelectToolFsmState::DraggingPivot),
			_ => None,
		}
	}

	fn pivot_gizmo(&self) -> PivotGizmo {
		self.pivot_gizmo.clone()
	}

	fn sync_history(&mut self, document: &DocumentMessageHandler) {
		let layers: Vec<_> = document.network_interface.selected_nodes().selected_visible_and_unlocked_layers(&document.network_interface).collect();
		self.ordered_layers.retain(|layer| layers.contains(layer));
		self.ordered_layers.extend(layers.iter().find(|&layer| !self.ordered_layers.contains(layer)));
		self.pivot_gizmo.layer = self.ordered_layers.last().copied()
	}
}

/// Draws the hover/selection outline for a layer. When a Path node is upstream, mirrors the Path tool's view
/// (e.g. the pre-solidified centerline for a Solidify Stroke layer); otherwise uses the layer's recorded outlines.
pub fn draw_layer_outline(overlay_context: &mut OverlayContext, document: &DocumentMessageHandler, layer: LayerNodeIdentifier, color: Option<&str>) {
	if let Some(targets) = document.network_interface.path_aware_outline_targets(layer) {
		let layer_to_viewport = document.metadata().transform_to_viewport_if_feeds(layer, &document.network_interface);
		overlay_context.outline(targets.iter(), layer_to_viewport, color);
	} else {
		let layer_to_viewport = document.metadata().transform_to_viewport(layer);
		overlay_context.outline(document.metadata().layer_with_free_points_outline(layer), layer_to_viewport, color);
	}
}

/// Bounding boxes are unfortunately not axis aligned. The bounding boxes are found after a transformation is applied to all of the layers.
/// This uses some rather confusing logic to determine what transform that should be.
pub fn create_bounding_box_transform(document: &DocumentMessageHandler) -> DAffine2 {
	// Update bounds
	document
		.network_interface
		.selected_nodes()
		.selected_visible_and_unlocked_layers(&document.network_interface)
		.find(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]))
		.map(|layer| document.metadata().transform_to_viewport_with_first_transform_node_if_group(layer, &document.network_interface))
		.unwrap_or_default()
}

impl Fsm for SelectToolFsmState {
	type ToolData = SelectToolData;
	type ToolOptions = ();

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionMessageContext, _tool_options: &(), responses: &mut VecDeque<Message>) -> Self {
		let ToolActionMessageContext {
			document,
			document_id,
			input,
			viewport,
			fonts,
			snap_session,
			..
		} = tool_action_data;

		let ToolMessage::Select(event) = event else { return self };
		match (self, event) {
			(_, SelectToolMessage::Overlays { context: mut overlay_context }) => {
				crate::messages::tool::common_functionality::layer_origin_cross::draw_for_selected_layers(&mut overlay_context, document);

				let selected_layers_count = document.network_interface.selected_nodes().selected_unlocked_layers(&document.network_interface).count();
				let paintable_layers_selected = has_paintable_selection(document);
				tool_data.selected_layers_changed = selected_layers_count != tool_data.selected_layers_count || paintable_layers_selected != tool_data.paintable_layers_selected;
				tool_data.selected_layers_count = selected_layers_count;
				tool_data.paintable_layers_selected = paintable_layers_selected;

				// Outline selected layers, but not artboards
				if overlay_context.visibility_settings.selection_outline() {
					for layer in document
						.network_interface
						.selected_nodes()
						.selected_visible_and_unlocked_layers(&document.network_interface)
						.filter(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]))
					{
						draw_layer_outline(&mut overlay_context, document, layer, None);

						if document.metadata().is_text_layer(layer) {
							let layer_to_viewport = document.metadata().transform_to_viewport(layer);
							let transformed_quad = layer_to_viewport * text_bounding_box(layer, document, fonts, responses);
							overlay_context.dashed_quad(transformed_quad, None, None, Some(7.), Some(5.), None);
						}
					}
				}

				let mut transform = create_bounding_box_transform(document);

				// Check if the matrix is not invertible
				let mut transform_tampered = false;
				if transform.matrix2.determinant() == 0. {
					transform.matrix2 += DMat2::IDENTITY * 1e-4; // TODO: Is this the cleanest way to handle this?
					transform_tampered = true;
				}

				let bounds = document
					.network_interface
					.selected_nodes()
					.selected_visible_and_unlocked_layers(&document.network_interface)
					.filter(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]))
					.filter_map(|layer| {
						document
							.metadata()
							.bounding_box_with_transform(layer, transform.inverse() * document.metadata().transform_to_viewport(layer))
					})
					.reduce(graphene_std::renderer::Quad::combine_bounds);

				// When not in Drawing State
				// Only highlight layers if the viewport is not being panned (middle mouse button is pressed)
				// TODO: Don't use `Key::MouseMiddle` directly, instead take it as a variable from the input mappings list like in all other places; or find a better way than checking the key state
				if !matches!(self, Self::Drawing { .. }) && !input.keyboard.get(Key::MouseMiddle as usize) {
					// Get the layer the user is hovering over
					// Artboards are included since they're needed for quick measurement, but will be filtered out for selection later on
					let click = document.click_list_with_artboards(input, viewport).last();
					let not_selected_click = click.filter(|&hovered_layer| !document.network_interface.selected_nodes().selected_layers_contains(hovered_layer, document.metadata()));
					if let Some(clicked) = not_selected_click {
						let clicked_artboard = document.network_interface.is_artboard(&clicked.to_node(), &[]);
						let outlining = overlay_context.visibility_settings.hover_outline() && !clicked_artboard;

						// Measure with Alt held down
						// TODO: Don't use `Key::Alt` directly, instead take it as a variable from the input mappings list like in all other places
						let measuring = overlay_context.visibility_settings.quick_measurement() && !matches!(self, Self::ResizingBounds { .. }) && input.keyboard.get(Key::Alt as usize);

						// The layer the hover outline shows, which quick measurement also measures to, found only when either is drawn
						let layer = (outlining || measuring).then(|| {
							if clicked_artboard {
								return clicked;
							}
							match tool_data.nested_selection_behavior {
								NestedSelectionBehavior::Deepest => document.find_deepest(&[clicked]),
								NestedSelectionBehavior::Shallowest => layer_selected_shallowest(clicked, document),
							}
							.unwrap_or(clicked)
						});

						if outlining && let Some(layer) = layer {
							let mut hover_overlay_draw = |layer: LayerNodeIdentifier, color: Option<&str>| {
								if layer.has_children(document.metadata()) {
									if let Some(bounds) = document.metadata().bounding_box_viewport(layer) {
										overlay_context.quad(Quad::from_box(bounds), color, None);
									}
								} else {
									draw_layer_outline(&mut overlay_context, document, layer, color);
								}
							};
							hover_overlay_draw(layer, None);
							if matches!(tool_data.nested_selection_behavior, NestedSelectionBehavior::Shallowest) {
								let mut selected = document.network_interface.selected_nodes();
								selected.add_selected_nodes(vec![layer.to_node()]);
								if let Some(new_selected) = clicked.ancestors(document.metadata()).filter(not_artboard(document)).find(|ancestor| {
									ancestor
										.parent(document.metadata())
										.is_some_and(|parent| selected.selected_layers_contains(parent, document.metadata()))
								}) {
									hover_overlay_draw(new_selected, Some(COLOR_OVERLAY_BLUE_50));
								}
							}
						}

						if measuring && let Some(layer) = layer {
							// Compute document-space bounding box (AABB) of all selected visible & unlocked layers
							let selected_bounds_doc_space = document
								.network_interface
								.selected_nodes()
								.selected_visible_and_unlocked_layers(&document.network_interface)
								// Exclude layers that are artboards from the selection bounding box
								.filter(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]))
								// For each remaining layer, try to get its document-space bounding box and convert it to a Rect
								.filter_map(|layer| document.metadata().bounding_box_document(layer).map(Rect::from_box))
								// Combine all individual bounding boxes into one overall bounding box that contains all selected layers
								.reduce(Rect::combine_bounds);

							// Compute document-space bounding box (AABB) of the hovered layer, or of the clicked layer within it if it's part of the selection
							let hovered_selected = document.network_interface.selected_nodes().selected_layers_contains(layer, document.metadata());
							let hovered_bounds_doc_space = document.metadata().bounding_box_document(if hovered_selected { clicked } else { layer });

							// If both selected and hovered bounds exist, overlay measurement lines
							if let (Some(selected_bounds), Some(hovered_bounds)) = (selected_bounds_doc_space, hovered_bounds_doc_space.map(Rect::from_box)) {
								let document_to_viewport = document.metadata().document_to_viewport;
								measure::overlay(selected_bounds, hovered_bounds, document_to_viewport, &mut overlay_context);
							}
						}
					}
				}

				if let Some(bounds) = bounds {
					let bounding_box_manager = tool_data.bounding_box_manager.get_or_insert(BoundingBoxManager::default());

					// TODO: Don't perform bounding box calculations here because the user can disable overlays which breaks bbox-based resizing
					bounding_box_manager.bounds = bounds;
					bounding_box_manager.transform = transform;
					bounding_box_manager.transform_tampered = transform_tampered;
					if overlay_context.visibility_settings.transform_cage() {
						bounding_box_manager.render_overlays(&mut overlay_context, true);
					}
				} else {
					tool_data.bounding_box_manager.take();
				}

				let angle = bounds
					.map(|bounds| transform * Quad::from_box(bounds))
					.map_or(0., |quad| (quad.top_left() - quad.top_right()).to_angle());

				let mouse_position = input.mouse.position;
				let compass_rose_state = tool_data.compass_rose.compass_rose_state(mouse_position, angle);

				let show_hover_ring = if let SelectToolFsmState::Dragging { axis, using_compass, .. } = self {
					using_compass && !axis.is_constraint()
				} else {
					compass_rose_state.is_ring()
				};

				let dragging_bounds = tool_data
					.bounding_box_manager
					.as_mut()
					.and_then(|bounding_box| bounding_box.check_selected_edges(input.mouse.position))
					.is_some();

				let rotating_bounds = tool_data
					.bounding_box_manager
					.as_ref()
					.map(|bounding_box| bounding_box.check_rotate(input.mouse.position))
					.unwrap_or_default();

				let is_resizing_or_rotating = matches!(self, SelectToolFsmState::ResizingBounds | SelectToolFsmState::SkewingBounds { .. } | SelectToolFsmState::RotatingBounds);

				if overlay_context.visibility_settings.transform_cage()
					&& let Some(bounds) = tool_data.bounding_box_manager.as_mut()
				{
					let edges = bounds.check_selected_edges(input.mouse.position);
					let is_skewing = matches!(self, SelectToolFsmState::SkewingBounds { .. });
					let is_near_square = edges.is_some_and(|hover_edge| bounds.over_extended_edge_midpoint(input.mouse.position, hover_edge));
					if is_skewing || (dragging_bounds && is_near_square && !is_resizing_or_rotating) {
						bounds.render_skew_gizmos(&mut overlay_context, tool_data.skew_edge);
					}
					if !is_skewing
						&& dragging_bounds && let Some(edges) = edges
					{
						tool_data.skew_edge = bounds.get_closest_edge(edges, input.mouse.position);
					}
				}

				let might_resize_or_rotate = dragging_bounds || rotating_bounds;
				let can_get_into_other_states = might_resize_or_rotate && !matches!(self, SelectToolFsmState::Dragging { .. });

				let show_compass = !(can_get_into_other_states || is_resizing_or_rotating);
				let show_compass_with_ring = bounds.map(|bounds| transform * Quad::from_box(bounds)).and_then(|quad| {
					const MIN_ARROWS_TO_RESIZE_HANDLE_DISTANCE: f64 = 4.;
					(show_compass && quad.all_sides_at_least_width(COMPASS_ROSE_HOVER_RING_DIAMETER + RESIZE_HANDLE_SIZE + MIN_ARROWS_TO_RESIZE_HANDLE_DISTANCE))
						.then_some(
							matches!(self, SelectToolFsmState::Dragging { .. })
								.then_some(show_hover_ring)
								.or((quad.contains(mouse_position)).then_some(show_hover_ring)),
						)
						.flatten()
				});

				let mut active_origin = None;
				let mut origin_angle = 0.;
				if overlay_context.visibility_settings.origin() && !tool_data.pivot_gizmo.state.is_pivot_type() {
					let get_angle = |layer: LayerNodeIdentifier| -> f64 {
						let quad = Quad::from_box([DVec2::ZERO, DVec2::ONE]);
						let bounds = document.metadata().transform_to_viewport_with_first_transform_node_if_group(layer, &document.network_interface) * quad;
						(bounds.top_left() - bounds.top_right()).to_angle()
					};
					if tool_data.pivot_gizmo.state.gizmo_type == PivotGizmoType::Average {
						let mut count = 0_usize;

						let sum: f64 = document
							.network_interface
							.selected_nodes()
							.selected_visible_and_unlocked_layers(&document.network_interface)
							.map(get_angle)
							.inspect(|_| count += 1)
							.sum();
						if count > 0 {
							origin_angle = sum / count as f64;
						}
					} else if tool_data.pivot_gizmo.state.gizmo_type == PivotGizmoType::Active {
						origin_angle = document
							.network_interface
							.selected_nodes()
							.selected_visible_and_unlocked_layers(&document.network_interface)
							.find(|&layer| Some(layer) == tool_data.pivot_gizmo.layer)
							.iter()
							.map(|&layer| get_angle(layer))
							.sum();
					}

					for layer in document.network_interface.selected_nodes().selected_visible_and_unlocked_layers(&document.network_interface) {
						let origin = graph_modification_utils::get_viewport_origin(layer, &document.network_interface);
						if Some(layer) == tool_data.pivot_gizmo.layer {
							active_origin = Some(origin);
							continue;
						}
						overlay_context.dowel_pin(origin, origin_angle, None);
					}
				}
				if let Some(origin) = active_origin {
					overlay_context.dowel_pin(origin, origin_angle, Some(COLOR_OVERLAY_YELLOW));
				}

				let has_layers = document.network_interface.selected_nodes().has_selected_nodes();
				let draw_pivot = tool_data.pivot_gizmo.state.is_pivot() && overlay_context.visibility_settings.pivot() && has_layers;
				tool_data.pivot_gizmo.pivot.recalculate_pivot(document);
				let pivot = draw_pivot.then_some(tool_data.pivot_gizmo.pivot.pivot).flatten();
				if let Some(pivot) = pivot {
					let offset = tool_data
						.pivot_gizmo_start
						.map(|offset| {
							if tool_data.pivot_gizmo.pivot_disconnected() {
								tool_data.drag_current - offset
							} else {
								Default::default()
							}
						})
						.unwrap_or_default();
					let shift = tool_data.pivot_gizmo_shift.unwrap_or_default();
					overlay_context.pivot(pivot + offset + shift, angle);
				}

				// Update compass rose
				if overlay_context.visibility_settings.compass_rose() {
					tool_data.compass_rose.refresh_position(document);
					let compass_center = tool_data.compass_rose.compass_rose_position();
					if !matches!(self, Self::Dragging { .. }) {
						tool_data.line_center = compass_center;
					}

					overlay_context.compass_rose(compass_center, angle, show_compass_with_ring);

					let axis_state = if let SelectToolFsmState::Dragging { axis, .. } = self {
						Some((axis, false))
					} else {
						compass_rose_state.axis_type().and_then(|axis| axis.is_constraint().then_some((axis, true)))
					};

					if show_compass_with_ring.is_some()
						&& let Some((axis, hover)) = axis_state
						&& axis.is_constraint()
					{
						let e0 = tool_data
							.bounding_box_manager
							.as_ref()
							.map(|bounding_box_manager| bounding_box_manager.transform * Quad::from_box(bounding_box_manager.bounds))
							.map_or(DVec2::X, |quad| (quad.top_left() - quad.top_right()).normalize_or(DVec2::X));

						let (direction, color, color_faded) = match axis {
							Axis::X => (e0, COLOR_OVERLAY_RED, COLOR_OVERLAY_RED_25),
							Axis::Y => (e0.perp(), COLOR_OVERLAY_GREEN, COLOR_OVERLAY_GREEN_25),
							_ => unreachable!(),
						};

						let viewport_diagonal = viewport.size().into_dvec2().length();

						let color = if !hover { color } else { color_faded };
						let line_center = tool_data.line_center;
						overlay_context.line(line_center - direction * viewport_diagonal, line_center + direction * viewport_diagonal, Some(color), None);
					}

					if let Some(angle) = DVec2::X.try_angle_to(mouse_position - tool_data.drag_start)
						&& axis_state.is_none_or(|(axis, _)| !axis.is_constraint())
						&& tool_data.axis_align
					{
						let snap_resolution = SELECTION_DRAG_ANGLE.to_radians();
						let snapped_angle = (angle / snap_resolution).round() * snap_resolution;

						// Anchor the guide to the dragged layers' center at the moment the drag began. It's computed live (rather than
						// snapshotted at drag start) so it stays correct as the layers move and the viewport auto-pans: their current
						// center minus the drag displacement recovers the start center in the present viewport frame. Sourcing it from
						// the layers being dragged (not the prior selection's pivot) is what keeps it on the layer actually grabbed.
						let origin = if matches!(self, Self::Dragging { .. }) {
							tool_data
								.dragging_layers_center(document)
								.map(|center| center - (tool_data.drag_current - tool_data.drag_start))
								.unwrap_or_else(|| tool_data.pivot_gizmo().position(document))
						} else {
							tool_data.pivot_gizmo().position(document)
						};
						let viewport_diagonal = viewport.size().into_dvec2().length();

						let edge = DVec2::from_angle(snapped_angle).normalize_or(DVec2::X);
						let perp = edge.perp();

						let (edge_color, perp_color) = if edge.x.abs() > edge.y.abs() {
							(COLOR_OVERLAY_RED, COLOR_OVERLAY_GREEN_25)
						} else {
							(COLOR_OVERLAY_GREEN, COLOR_OVERLAY_RED_25)
						};
						overlay_context.line(origin - edge * viewport_diagonal, origin + edge * viewport_diagonal, Some(edge_color), None);
						overlay_context.line(origin - perp * viewport_diagonal, origin + perp * viewport_diagonal, Some(perp_color), None);
					}
				}

				// Check if the tool is in selection mode
				if let Self::Drawing { selection_shape, .. } = self {
					// Get the updated selection box bounds
					let quad = Quad::from_box([tool_data.drag_start, tool_data.drag_current]);

					let current_selection_mode = match tool_action_data.preferences.get_selection_mode() {
						SelectionMode::Directional => tool_data.calculate_selection_mode_from_direction(),
						SelectionMode::Touched => SelectionMode::Touched,
						SelectionMode::Enclosed => SelectionMode::Enclosed,
					};

					// Draw outline visualizations on the layers to be selected
					let intersected_layers = match selection_shape {
						SelectionShapeType::Box => document.intersect_quad_no_artboards(quad, viewport).collect(),
						SelectionShapeType::Lasso => tool_data.intersect_lasso_no_artboards(document, viewport),
					};
					let layers_to_outline = intersected_layers.into_iter().filter(|layer| match current_selection_mode {
						SelectionMode::Enclosed => match selection_shape {
							SelectionShapeType::Box => document.is_layer_fully_inside(layer, quad),
							SelectionShapeType::Lasso => tool_data.is_layer_inside_lasso_polygon(layer, document, viewport),
						},
						SelectionMode::Touched => match tool_data.nested_selection_behavior {
							NestedSelectionBehavior::Deepest => !layer.has_children(document.metadata()),
							NestedSelectionBehavior::Shallowest => true,
						},
						SelectionMode::Directional => unreachable!(),
					});

					if overlay_context.visibility_settings.selection_outline() {
						// Draws a temporary outline on the layers that will be selected by the current box/lasso area
						for layer in layers_to_outline {
							draw_layer_outline(&mut overlay_context, document, layer, None);
						}
					}

					// Update the selection box
					let fill_color = Some(COLOR_OVERLAY_BLUE_05);

					let polygon = &tool_data.lasso_polygon;

					match (selection_shape, current_selection_mode) {
						(SelectionShapeType::Box, SelectionMode::Enclosed) => overlay_context.dashed_quad(quad, None, fill_color, Some(4.), Some(4.), Some(0.5)),
						(SelectionShapeType::Lasso, SelectionMode::Enclosed) => overlay_context.dashed_polygon(polygon, None, fill_color, Some(4.), Some(4.), Some(0.5)),
						(SelectionShapeType::Box, _) => overlay_context.quad(quad, None, fill_color),
						(SelectionShapeType::Lasso, _) => overlay_context.polygon(polygon, None, fill_color),
					}
				}

				if let Self::Dragging { .. } = self {
					let quad = Quad::from_box([tool_data.drag_start, tool_data.drag_current]);
					let document_start = document.metadata().document_to_viewport.inverse().transform_point2(quad.top_left());
					let document_current = document.metadata().document_to_viewport.inverse().transform_point2(quad.bottom_right());

					overlay_context.translation_box(document_current - document_start, quad, None);
				}

				self
			}
			(_, SelectToolMessage::EditLayer) => {
				responses.add(DeferMessage::AfterGraphRun {
					messages: vec![SelectToolMessage::EditLayerExec.into()],
				});

				self
			}
			(_, SelectToolMessage::EditLayerExec) => {
				if let Some(intersect) = document.click(input, viewport) {
					match tool_data.nested_selection_behavior {
						NestedSelectionBehavior::Shallowest => edit_layer_shallowest_manipulation(document, intersect, responses),
						NestedSelectionBehavior::Deepest => edit_layer_deepest_manipulation(intersect, &document.network_interface, responses),
					}
				}
				self
			}
			(
				SelectToolFsmState::Ready { .. },
				SelectToolMessage::DragStart {
					extend_selection,
					remove_from_selection,
					select_deepest,
					lasso_select,
					..
				},
			) => {
				tool_data.drag_start = input.mouse.position;
				tool_data.drag_current = input.mouse.position;
				tool_data.selection_mode = None;

				let mut selected: Vec<_> = document.network_interface.selected_nodes().selected_visible_and_unlocked_layers(&document.network_interface).collect();
				let intersection_list = document.click_list(input, viewport).collect::<Vec<_>>();
				let intersection = document.find_deepest(&intersection_list);

				let position = tool_data.pivot_gizmo().position(document);
				let (resize, rotate, skew) = transforming_transform_cage(document, &mut tool_data.bounding_box_manager, input, responses, &mut tool_data.layers_dragging, Some(position));

				// If the user is dragging the bounding box bounds, go into ResizingBounds mode.
				// If the user is dragging the rotate trigger, go into RotatingBounds mode.
				// If the user clicks on a layer that is in their current selection, go into the dragging mode.
				// If the user clicks on new shape, make that layer their new selection.
				// Otherwise enter the box select mode
				let bounds = tool_data
					.bounding_box_manager
					.as_ref()
					.map(|bounding_box_manager| bounding_box_manager.transform * Quad::from_box(bounding_box_manager.bounds));

				let angle = bounds.map_or(0., |quad| (quad.top_left() - quad.top_right()).to_angle());
				let mouse_position = input.mouse.position;
				let compass_rose_state = tool_data.compass_rose.compass_rose_state(mouse_position, angle);

				let show_compass = bounds.is_some_and(|quad| quad.all_sides_at_least_width(COMPASS_ROSE_HOVER_RING_DIAMETER) && quad.contains(mouse_position));
				let can_grab_compass_rose = compass_rose_state.can_grab() && (show_compass || bounds.is_none());

				let state = if let Some(state) = tool_data.state_from_pivot_gizmo(input.mouse.position) {
					responses.add(DocumentMessage::StartTransaction);
					tool_data.pivot_before_drag = Some(tool_data.pivot_gizmo.pivot.clone());

					// The pivot snaps to the selected layers too, since they stay put while it moves
					snap_session.begin_gesture(SnapView::new(document, *document_id, input), Moving::default());

					state
				}
				// Dragging one (or two, forming a corner) of the transform cage bounding box edges
				else if resize {
					let moving = Moving {
						layers: tool_data.layers_dragging.clone(),
						..Default::default()
					};
					snap_session.begin_gesture(SnapView::new(document, *document_id, input), moving);
					SelectToolFsmState::ResizingBounds
				} else if skew {
					if let Some(bounds) = &mut tool_data.bounding_box_manager {
						bounds.begin_skewing(SnapView::new(document, *document_id, input), snap_session, &tool_data.layers_dragging);
					}
					SelectToolFsmState::SkewingBounds { skew: Key::Control }
				}
				// Dragging the selected layers around to transform them
				else if can_grab_compass_rose || intersection.is_some_and(|intersection| selected.iter().any(|selected_layer| intersection.starts_with(*selected_layer, document.metadata()))) {
					responses.add(DocumentMessage::StartTransaction);

					if input.keyboard.key(select_deepest) || tool_data.nested_selection_behavior == NestedSelectionBehavior::Deepest {
						tool_data.select_single_layer = intersection;
					} else {
						tool_data.select_single_layer = intersection.and_then(|intersection| intersection.ancestors(document.metadata()).find(|ancestor| selected.contains(ancestor)));
					}

					tool_data.layers_dragging = selected;
					let (axis, using_compass) = {
						let axis_state = compass_rose_state.axis_type().filter(|_| can_grab_compass_rose);
						(axis_state.unwrap_or_default(), axis_state.is_some())
					};

					tool_data.pivot_gizmo_start = Some(tool_data.drag_current);

					SelectToolFsmState::Dragging {
						axis,
						using_compass,
						has_dragged: false,
						deepest: input.keyboard.key(select_deepest),
						remove: input.keyboard.key(extend_selection),
					}
				}
				// Dragging near the transform cage bounding box to rotate it
				else if rotate {
					if let Some(bounds) = &mut tool_data.bounding_box_manager {
						bounds.begin_turning(SnapView::new(document, *document_id, input), snap_session, &tool_data.layers_dragging);
					}
					SelectToolFsmState::RotatingBounds
				}
				// Dragging a selection box
				else {
					tool_data.layers_dragging = selected;
					let extend = input.keyboard.key(extend_selection);
					if !extend && !input.keyboard.key(remove_from_selection) {
						responses.add(DocumentMessage::DeselectAllLayers);

						if !tool_data.pivot_gizmo.pivot.pinned {
							let position = tool_data.pivot_gizmo.pivot.last_non_none_reference_point;
							responses.add(SelectToolMessage::SetPivot { position });
						}

						tool_data.layers_dragging.clear();
					}

					if let Some(intersection) = intersection {
						tool_data.layer_selected_on_start = Some(intersection);
						selected = intersection_list;

						match tool_data.nested_selection_behavior {
							NestedSelectionBehavior::Shallowest if !input.keyboard.key(select_deepest) => drag_shallowest_manipulation(responses, selected, tool_data, document, false, extend),
							_ => drag_deepest_manipulation(responses, selected, tool_data, document, false),
						}

						responses.add(DocumentMessage::StartTransaction);

						tool_data.pivot_gizmo_start = Some(tool_data.drag_current);

						SelectToolFsmState::Dragging {
							axis: Axis::None,
							using_compass: false,
							has_dragged: false,
							deepest: input.keyboard.key(select_deepest),
							remove: input.keyboard.key(extend_selection),
						}
					} else {
						let selection_shape = if input.keyboard.key(lasso_select) { SelectionShapeType::Lasso } else { SelectionShapeType::Box };
						SelectToolFsmState::Drawing { selection_shape, has_drawn: false }
					}
				};
				tool_data.non_duplicated_layers = None;

				if matches!(state, SelectToolFsmState::Dragging { .. }) {
					tool_data.prepare_slide(document, input);
					tool_data.begin_snapping_gesture(snap_session, SnapView::new(document, *document_id, input), &[]);
				}

				state
			}
			(SelectToolFsmState::DraggingPivot, SelectToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);

				// The pivot lives in the tool rather than the document, so aborting the transaction doesn't put it back
				if let Some(pivot) = tool_data.pivot_before_drag.take() {
					let reported = tool_data.pivot_gizmo.pivot.old_pivot_position;
					tool_data.pivot_gizmo.pivot = pivot;

					let pivot_gizmo = tool_data.pivot_gizmo();
					responses.add(TransformLayerMessage::SetPivotGizmo { pivot_gizmo });
					responses.add(OverlaysMessage::Draw);

					// Only the tool keeps its last reported reference point, which makes the control bar refresh
					tool_data.pivot_gizmo.pivot.old_pivot_position = reported;
				}

				let selection = tool_data.nested_selection_behavior;
				SelectToolFsmState::Ready { selection }
			}
			(
				SelectToolFsmState::Dragging {
					axis,
					using_compass,
					has_dragged,
					deepest,
					remove,
				},
				SelectToolMessage::PointerMove { modifier_keys },
			) => {
				if !has_dragged {
					responses.add(ToolMessage::UpdateHints);
				}
				tool_data.follow_canvas(document);

				// Alt swaps the dragged layers for copies or puts the originals back, and the snapping starts over to suit
				let duplicate = input.keyboard.key(modifier_keys.duplicate);
				let swapped = duplicate != tool_data.non_duplicated_layers.is_some();
				let mut removed_copies = Vec::new();
				if duplicate && tool_data.non_duplicated_layers.is_none() {
					tool_data.start_duplicates(document, responses);
				} else if !duplicate && tool_data.non_duplicated_layers.is_some() {
					removed_copies = tool_data.layers_dragging.clone();
					tool_data.stop_duplicates(document, responses);
				}
				let pending_copies_rendered =
					tool_data.slide.as_ref().is_some_and(|slide| slide.copies_pending) && tool_data.layers_dragging.iter().all(|&layer| document.metadata().click_targets(layer).is_some());
				if swapped || pending_copies_rendered {
					tool_data.begin_snapping_gesture(snap_session, SnapView::new(document, *document_id, input), &removed_copies);
				}

				tool_data.axis_align = input.keyboard.key(modifier_keys.axis_align);

				let e0 = tool_data
					.bounding_box_manager
					.as_ref()
					.map(|bounding_box_manager| bounding_box_manager.transform * Quad::from_box(bounding_box_manager.bounds))
					.map_or(DVec2::X, |quad| (quad.top_left() - quad.top_right()).normalize_or(DVec2::X));

				// Constrain the drag to the selection's local axis (not the world axis) so a rotated object's arrows track the mouse one-to-one
				let axis_constraint = match axis {
					Axis::X => Some(e0),
					Axis::Y => Some(e0.perp()),
					Axis::None => None,
				};

				// Shift holds the drag to the nearest axis from where it began, as seen in the viewport
				let axis_aligned = || {
					let snap_resolution = SELECTION_DRAG_ANGLE.to_radians();
					let angle = DVec2::X.try_angle_to(input.mouse.position - tool_data.drag_start)?;
					Some(DVec2::from_angle((angle / snap_resolution).round() * snap_resolution))
				};
				let held_direction = axis_constraint.or_else(|| tool_data.axis_align.then(axis_aligned).flatten());

				// The move that swaps copies in or out slides nothing, as the swap only takes effect once its messages are processed
				if !swapped {
					let mouse_delta = tool_data.slide_dragged_layers(snap_session, SnapView::new(document, *document_id, input), held_direction);

					// TODO: Cache the result of `shallowest_unique_layers` to avoid this heavy computation every frame of movement, see https://github.com/GraphiteEditor/Graphite/pull/481
					for layer in document.network_interface.shallowest_unique_layers(&[]) {
						responses.add_front(GraphOperationMessage::TransformChange {
							layer,
							transform: DAffine2::from_translation(mouse_delta),
							transform_in: TransformIn::Viewport,
							skip_rerender: false,
						});
					}
				}

				// Auto-panning
				let messages = [
					SelectToolMessage::PointerOutsideViewport { modifier_keys: modifier_keys.clone() }.into(),
					SelectToolMessage::PointerMove { modifier_keys }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);

				SelectToolFsmState::Dragging {
					axis,
					using_compass,
					has_dragged: true,
					deepest,
					remove,
				}
			}
			(SelectToolFsmState::ResizingBounds, SelectToolMessage::PointerMove { modifier_keys }) => {
				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					resize_bounds(
						SnapView::new(document, *document_id, input),
						snap_session,
						responses,
						bounds,
						&mut tool_data.layers_dragging,
						input.keyboard.key(modifier_keys.center),
						input.keyboard.key(modifier_keys.axis_align),
						ToolType::Select,
					);
					let messages = [
						SelectToolMessage::PointerOutsideViewport { modifier_keys: modifier_keys.clone() }.into(),
						SelectToolMessage::PointerMove { modifier_keys }.into(),
					];
					tool_data.auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);
				}
				SelectToolFsmState::ResizingBounds
			}
			(SelectToolFsmState::SkewingBounds { skew }, SelectToolMessage::PointerMove { .. }) => {
				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					skew_bounds(
						SnapView::new(document, *document_id, input),
						snap_session,
						responses,
						bounds,
						input.keyboard.key(skew),
						&mut tool_data.layers_dragging,
						ToolType::Select,
					);
				}
				SelectToolFsmState::SkewingBounds { skew }
			}
			(SelectToolFsmState::RotatingBounds, SelectToolMessage::PointerMove { .. }) => {
				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					rotate_bounds(
						SnapView::new(document, *document_id, input),
						snap_session,
						responses,
						bounds,
						&mut tool_data.layers_dragging,
						tool_data.drag_start,
						input.keyboard.key(Key::Shift),
						ToolType::Select,
					);
				}

				SelectToolFsmState::RotatingBounds
			}
			(SelectToolFsmState::DraggingPivot, SelectToolMessage::PointerMove { modifier_keys }) => {
				let document_to_viewport = document.metadata().document_to_viewport;
				let pointer = document_to_viewport.inverse().transform_point2(input.mouse.position);
				let snapped = snap_session.snap_point(SnapView::new(document, *document_id, input), pointer, PointOptions::default());

				tool_data.pivot_gizmo.pivot.set_viewport_position(document_to_viewport.transform_point2(snapped));

				responses.add(NodeGraphMessage::RunDocumentGraph);

				// Auto-panning
				let messages = [
					SelectToolMessage::PointerOutsideViewport { modifier_keys: modifier_keys.clone() }.into(),
					SelectToolMessage::PointerMove { modifier_keys }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);

				SelectToolFsmState::DraggingPivot
			}
			(SelectToolFsmState::Drawing { selection_shape, has_drawn }, SelectToolMessage::PointerMove { modifier_keys }) => {
				if !has_drawn {
					responses.add(ToolMessage::UpdateHints);
				}

				tool_data.drag_current = input.mouse.position;
				responses.add(OverlaysMessage::Draw);

				if selection_shape == SelectionShapeType::Lasso {
					extend_lasso(&mut tool_data.lasso_polygon, tool_data.drag_current);
				}

				// Auto-panning
				let messages = [
					SelectToolMessage::PointerOutsideViewport { modifier_keys: modifier_keys.clone() }.into(),
					SelectToolMessage::PointerMove { modifier_keys }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);

				SelectToolFsmState::Drawing { selection_shape, has_drawn: true }
			}
			(SelectToolFsmState::Ready { .. }, SelectToolMessage::PointerMove { .. }) => {
				let dragging_bounds = tool_data
					.bounding_box_manager
					.as_mut()
					.and_then(|bounding_box| bounding_box.check_selected_edges(input.mouse.position))
					.is_some();

				let mut cursor = tool_data
					.bounding_box_manager
					.as_ref()
					.map_or(MouseCursorIcon::Default, |bounds| bounds.get_cursor(input, true, dragging_bounds, Some(tool_data.skew_edge)));

				// Dragging the pivot overrules the other operations
				if tool_data.state_from_pivot_gizmo(input.mouse.position).is_some() {
					cursor = MouseCursorIcon::Move;
				}

				// Generate the hover outline
				responses.add(OverlaysMessage::Draw);

				if tool_data.cursor != cursor {
					tool_data.cursor = cursor;
					responses.add(FrontendMessage::UpdateMouseCursor { cursor });
				}

				let selection = tool_data.nested_selection_behavior;
				SelectToolFsmState::Ready { selection }
			}
			(
				SelectToolFsmState::Dragging {
					axis,
					using_compass,
					has_dragged,
					deepest,
					remove,
				},
				SelectToolMessage::PointerOutsideViewport { .. },
			) => {
				// Auto-panning
				if let Some(shift) = tool_data.auto_panning.shift_viewport(input, viewport, responses) {
					tool_data.drag_current += shift;
					tool_data.drag_start += shift;
				}

				SelectToolFsmState::Dragging {
					axis,
					using_compass,
					has_dragged,
					deepest,
					remove,
				}
			}
			(SelectToolFsmState::ResizingBounds | SelectToolFsmState::SkewingBounds { .. }, SelectToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				if let Some(shift) = tool_data.auto_panning.shift_viewport(input, viewport, responses)
					&& let Some(bounds) = &mut tool_data.bounding_box_manager
				{
					bounds.center_of_transformation += shift;
					bounds.original_bound_transform.translation += shift;
				}

				self
			}
			(SelectToolFsmState::DraggingPivot, SelectToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				let _ = tool_data.auto_panning.shift_viewport(input, viewport, responses);

				self
			}
			(SelectToolFsmState::Drawing { .. }, SelectToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				if let Some(shift) = tool_data.auto_panning.shift_viewport(input, viewport, responses) {
					tool_data.drag_start += shift;
				}

				self
			}
			(state, SelectToolMessage::PointerOutsideViewport { modifier_keys }) => {
				// Auto-panning
				let messages = [
					SelectToolMessage::PointerOutsideViewport { modifier_keys: modifier_keys.clone() }.into(),
					SelectToolMessage::PointerMove { modifier_keys }.into(),
				];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(SelectToolFsmState::Dragging { has_dragged, remove, deepest, .. }, SelectToolMessage::DragStop { remove_from_selection }) => {
				// Deselect layer if not snap dragging
				responses.add(DocumentMessage::EndTransaction);
				tool_data.axis_align = false;

				if !has_dragged && input.keyboard.key(remove_from_selection) && tool_data.layer_selected_on_start.is_none() {
					// When you click on the layer with remove from selection key (shift) pressed, we deselect all nodes that are children.
					let quad = tool_data.selection_quad();
					let intersection = document.intersect_quad_no_artboards(quad, viewport);

					if let Some(path) = intersection.last() {
						let replacement_selected_layers: Vec<_> = document
							.network_interface
							.selected_nodes()
							.selected_layers(document.metadata())
							.filter(|&layer| !path.starts_with(layer, document.metadata()))
							.collect();

						tool_data.layers_dragging.clear();
						tool_data.layers_dragging.extend(replacement_selected_layers.iter());

						responses.add(NodeGraphMessage::SelectedNodesSet {
							nodes: replacement_selected_layers
								.iter()
								.filter_map(|layer| {
									if *layer != LayerNodeIdentifier::ROOT_PARENT {
										Some(layer.to_node())
									} else {
										log::error!("ROOT_PARENT cannot be part of replacement_selected_layers");
										None
									}
								})
								.collect(),
						});
					}
				} else if tool_data.select_single_layer.take().is_some() {
					// Previously, we may have had many layers selected. If the user clicks without dragging, we should just select the one layer that has been clicked.
					if !has_dragged {
						let selected = document.click_list(input, viewport).collect::<Vec<_>>();
						let intersection = document.find_deepest(&selected);
						if let Some(intersection) = intersection {
							tool_data.layer_selected_on_start = Some(intersection);

							match tool_data.nested_selection_behavior {
								NestedSelectionBehavior::Shallowest if remove && !deepest => drag_shallowest_manipulation(responses, selected, tool_data, document, true, true),
								NestedSelectionBehavior::Deepest if remove => drag_deepest_manipulation(responses, selected, tool_data, document, true),
								NestedSelectionBehavior::Shallowest if !deepest => drag_shallowest_manipulation(responses, selected, tool_data, document, false, true),
								_ => {
									// Narrow multi-selection to just the clicked layer (no-op if it's already the sole selection)
									let currently_selected = document.network_interface.selected_nodes().selected_layers(document.metadata()).collect::<Vec<_>>();
									let already_only_selection = currently_selected.as_slice() == [intersection];

									if !already_only_selection {
										responses.add(DocumentMessage::DeselectAllLayers);
										tool_data.layers_dragging.clear();
										drag_deepest_manipulation(responses, selected, tool_data, document, false)
									}
								}
							}
						}
					}
				}

				tool_data.layer_selected_on_start = None;
				tool_data.select_single_layer = None;

				if let Some(start) = tool_data.pivot_gizmo_start {
					let offset = if tool_data.pivot_gizmo.pivot_disconnected() {
						tool_data.drag_current - start
					} else {
						Default::default()
					};
					if let Some(v) = tool_data.pivot_gizmo.pivot.pivot.as_mut() {
						*v += offset;
					}
				}
				tool_data.pivot_gizmo_start = None;

				let pivot_gizmo = tool_data.pivot_gizmo();
				responses.add(TransformLayerMessage::SetPivotGizmo { pivot_gizmo });

				let selection = tool_data.nested_selection_behavior;
				SelectToolFsmState::Ready { selection }
			}
			(
				SelectToolFsmState::ResizingBounds | SelectToolFsmState::SkewingBounds { .. } | SelectToolFsmState::RotatingBounds | SelectToolFsmState::DraggingPivot,
				SelectToolMessage::DragStop { .. } | SelectToolMessage::Enter,
			) => {
				let drag_too_small = input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON;
				let response = if drag_too_small { DocumentMessage::AbortTransaction } else { DocumentMessage::EndTransaction };

				tool_data.pivot_before_drag = None;
				let pivot_gizmo = tool_data.pivot_gizmo();
				responses.add(TransformLayerMessage::SetPivotGizmo { pivot_gizmo });

				responses.add(response);

				tool_data.axis_align = false;
				responses.add(OverlaysMessage::Draw);

				if !matches!(self, SelectToolFsmState::DraggingPivot)
					&& let Some(bounds) = &mut tool_data.bounding_box_manager
				{
					bounds.original_transforms.clear();
				}

				let selection = tool_data.nested_selection_behavior;
				SelectToolFsmState::Ready { selection }
			}
			(SelectToolFsmState::Drawing { selection_shape, .. }, SelectToolMessage::DragStop { remove_from_selection }) => {
				let quad = tool_data.selection_quad();

				let selection_mode = match tool_action_data.preferences.get_selection_mode() {
					SelectionMode::Directional => tool_data.calculate_selection_mode_from_direction(),
					selection_mode => selection_mode,
				};

				let intersection: Vec<LayerNodeIdentifier> = match selection_shape {
					SelectionShapeType::Box => document.intersect_quad_no_artboards(quad, viewport).collect(),
					SelectionShapeType::Lasso => tool_data.intersect_lasso_no_artboards(document, viewport),
				};
				let new_selected: HashSet<_> = if selection_mode == SelectionMode::Enclosed {
					let is_inside = |layer: &LayerNodeIdentifier| match selection_shape {
						SelectionShapeType::Box => document.is_layer_fully_inside(layer, quad),
						SelectionShapeType::Lasso => tool_data.is_layer_inside_lasso_polygon(layer, document, viewport),
					};
					intersection.into_iter().filter(is_inside).collect()
				} else {
					intersection.into_iter().collect()
				};

				let current_selected: HashSet<_> = document.network_interface.selected_nodes().selected_layers(document.metadata()).collect();
				let negative_selection = input.keyboard.key(remove_from_selection);
				let selection_modified = new_selected != current_selected;

				// Negative selection when both Shift and Ctrl are pressed
				if negative_selection {
					let updated_selection = current_selected
						.into_iter()
						.filter(|layer| !new_selected.iter().any(|selected| layer.starts_with(*selected, document.metadata())))
						.collect();
					tool_data.layers_dragging = updated_selection;
				} else if selection_modified {
					match tool_data.nested_selection_behavior {
						NestedSelectionBehavior::Deepest => {
							let filtered_selections = filter_nested_selection(document.metadata(), &new_selected);
							tool_data.layers_dragging.extend(filtered_selections);
						}
						NestedSelectionBehavior::Shallowest => {
							// Find each new_selected's parent node
							let parent_selected: HashSet<_> = new_selected
								.into_iter()
								.map(|layer| layer.ancestors(document.metadata()).filter(not_artboard(document)).last().unwrap_or(layer))
								.collect();
							tool_data.layers_dragging.extend(parent_selected.iter().copied());
						}
					}
				}

				if negative_selection || selection_modified {
					responses.add(NodeGraphMessage::SelectedNodesSet {
						nodes: tool_data
							.layers_dragging
							.iter()
							.filter_map(|layer| {
								if *layer != LayerNodeIdentifier::ROOT_PARENT {
									Some(layer.to_node())
								} else {
									log::error!("ROOT_PARENT cannot be part of tool_data.layers_dragging");
									None
								}
							})
							.collect(),
					});
				}

				tool_data.lasso_polygon.clear();

				responses.add(OverlaysMessage::Draw);

				let selection = tool_data.nested_selection_behavior;
				SelectToolFsmState::Ready { selection }
			}
			(SelectToolFsmState::Ready { .. }, SelectToolMessage::Enter) => {
				let selected_nodes = document.network_interface.selected_nodes();
				let mut selected_layers = selected_nodes.selected_layers(document.metadata());

				if let Some(layer) = selected_layers.next() {
					// Check that only one layer is selected
					if selected_layers.next().is_none() && document.metadata().is_text_layer(layer) {
						responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Text });
						responses.add(TextToolMessage::EditSelected);
					}
				}

				let selection = tool_data.nested_selection_behavior;
				SelectToolFsmState::Ready { selection }
			}
			(SelectToolFsmState::Dragging { .. }, SelectToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.axis_align = false;
				tool_data.lasso_polygon.clear();
				responses.add(OverlaysMessage::Draw);

				let selection = tool_data.nested_selection_behavior;
				SelectToolFsmState::Ready { selection }
			}
			(_, SelectToolMessage::Abort) => {
				tool_data.layers_dragging.retain(|layer| {
					if *layer != LayerNodeIdentifier::ROOT_PARENT {
						document.network_interface.document_network().nodes.contains_key(&layer.to_node())
					} else {
						false
					}
				});

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				responses.add(DocumentMessage::AbortTransaction);
				tool_data.lasso_polygon.clear();
				responses.add(OverlaysMessage::Draw);

				// Escape with nothing under way puts back to sleep whatever resting the pointer had woken
				if matches!(self, SelectToolFsmState::Ready { .. }) {
					snap_session.sleep_all();
				}

				let selection = tool_data.nested_selection_behavior;
				SelectToolFsmState::Ready { selection }
			}
			(_, SelectToolMessage::SetPivot { position }) => {
				tool_data.pivot_gizmo.pivot.last_non_none_reference_point = position;
				tool_data.pivot_gizmo.pivot.pinned = false;

				let pos: Option<DVec2> = position.into();

				tool_data.pivot_gizmo.pivot.set_normalized_position(pos.unwrap());

				let pivot_gizmo = tool_data.pivot_gizmo();
				responses.add(TransformLayerMessage::SetPivotGizmo { pivot_gizmo });

				responses.add(NodeGraphMessage::RunDocumentGraph);

				self
			}
			(_, SelectToolMessage::SyncHistory) => {
				tool_data.sync_history(document);

				self
			}
			(_, SelectToolMessage::ShiftSelectedNodes { offset }) => {
				let offset = document.metadata().document_to_viewport.transform_vector2(offset);
				if tool_data.pivot_gizmo.pivot_disconnected() {
					if let Some(v) = tool_data.pivot_gizmo.pivot.pivot.as_mut() {
						*v += offset;
					}

					let pivot_gizmo = tool_data.pivot_gizmo();
					responses.add(TransformLayerMessage::SetPivotGizmo { pivot_gizmo });
				}

				self
			}
			(_, SelectToolMessage::PivotShift { offset, flush }) => {
				if flush {
					if let Some(v) = tool_data.pivot_gizmo.pivot.pivot.as_mut() {
						*v += tool_data.pivot_gizmo_shift.take().unwrap_or_default();
					}
					let pivot_gizmo = tool_data.pivot_gizmo();
					responses.add(TransformLayerMessage::SetPivotGizmo { pivot_gizmo });
					return self;
				}
				if tool_data.pivot_gizmo.pivot_disconnected() {
					tool_data.pivot_gizmo_shift = offset;
				}

				self
			}
			_ => self,
		}
	}

	fn standard_tool_messages(&self, message: &ToolMessage, responses: &mut VecDeque<Message>) -> bool {
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

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		match self {
			SelectToolFsmState::Ready { selection } => {
				let hint_data = HintData(vec![
					HintGroup({
						let mut hints = vec![HintInfo::mouse(MouseMotion::Lmb, "Select Object"), HintInfo::keys([Key::Shift], "Extend").prepend_plus()];
						if *selection == NestedSelectionBehavior::Shallowest {
							hints.extend([HintInfo::keys([Key::Accel], "Deepest").prepend_plus(), HintInfo::mouse(MouseMotion::LmbDouble, "Deepen")]);
						}
						hints
					}),
					HintGroup(vec![
						HintInfo::mouse(MouseMotion::LmbDrag, "Select Area"),
						HintInfo::keys([Key::Shift], "Extend").prepend_plus(),
						HintInfo::keys([Key::Alt], "Subtract").prepend_plus(),
						HintInfo::keys([Key::Control], "Lasso").prepend_plus(),
					]),
					// TODO: Make all the following hints only appear if there is at least one selected layer
					HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Drag Selected")]),
					HintGroup(vec![HintInfo::multi_keys([[Key::KeyG], [Key::KeyR], [Key::KeyS]], "Grab/Rotate/Scale Selected")]),
					HintGroup(vec![
						HintInfo::arrow_keys("Nudge Selected"),
						HintInfo::keys([Key::Shift], "10x").prepend_plus(),
						HintInfo::keys([Key::Alt], "Resize Corner").prepend_plus(),
						HintInfo::keys([Key::Control], "Other Corner").prepend_plus(),
					]),
					HintGroup(vec![
						HintInfo::keys_and_mouse([Key::Alt], MouseMotion::LmbDrag, "Move Duplicate"),
						HintInfo::keys([Key::Control, Key::KeyD], "Duplicate").add_mac_keys([Key::Command, Key::KeyD]),
					]),
				]);
				hint_data.send_layout(responses);
			}
			SelectToolFsmState::Dragging { axis, using_compass, has_dragged, .. } if *has_dragged => {
				let mut hint_data = vec![
					HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
					HintGroup(vec![
						HintInfo::keys([Key::Alt], "Move Duplicate"),
						HintInfo::keys([Key::Control, Key::KeyD], "Place Duplicate").add_mac_keys([Key::Command, Key::KeyD]),
					]),
				];

				if !(*using_compass && axis.is_constraint()) {
					hint_data.push(HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain to Axis")]));
				};
				hint_data.push(snapping_hints());
				let hint_data = HintData(hint_data);
				hint_data.send_layout(responses);
			}
			SelectToolFsmState::Drawing { has_drawn, .. } if *has_drawn => {
				let hint_data = HintData(vec![
					HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
					HintGroup(vec![HintInfo::keys([Key::Shift], "Extend"), HintInfo::keys([Key::Alt], "Subtract")]),
					// TODO: Re-select deselected layers during drag when Shift is pressed, and re-deselect if Shift is released before drag ends.
					// TODO: (See https://discord.com/channels/731730685944922173/1216976541947531264/1321360311298818048)
					// HintGroup(vec![HintInfo::keys([Key::Shift], "Extend")])
				]);
				hint_data.send_layout(responses);
			}
			SelectToolFsmState::Drawing { .. } | SelectToolFsmState::Dragging { .. } => {}
			SelectToolFsmState::ResizingBounds => {
				let hint_data = HintData(vec![
					HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
					HintGroup(vec![HintInfo::keys([Key::Alt], "From Pivot"), HintInfo::keys([Key::Shift], "Preserve Aspect Ratio")]),
					snapping_hints(),
				]);
				hint_data.send_layout(responses);
			}
			SelectToolFsmState::RotatingBounds => {
				let hint_data = HintData(vec![
					HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
					HintGroup(vec![HintInfo::keys([Key::Shift], "15° Increments")]),
					snapping_hints(),
				]);
				hint_data.send_layout(responses);
			}
			SelectToolFsmState::SkewingBounds { .. } => {
				let hint_data = HintData(vec![
					HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
					HintGroup(vec![HintInfo::keys([Key::Control], "Unlock Slide")]),
					snapping_hints(),
				]);
				hint_data.send_layout(responses);
			}
			SelectToolFsmState::DraggingPivot => {
				let hint_data = HintData(vec![
					HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
					snapping_hints(),
				]);
				hint_data.send_layout(responses);
			}
		}
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

fn not_artboard(document: &DocumentMessageHandler) -> impl Fn(&LayerNodeIdentifier) -> bool + '_ {
	|&layer| layer != LayerNodeIdentifier::ROOT_PARENT && !document.network_interface.is_artboard(&layer.to_node(), &[])
}

fn drag_shallowest_manipulation(responses: &mut VecDeque<Message>, selected: Vec<LayerNodeIdentifier>, tool_data: &mut SelectToolData, document: &DocumentMessageHandler, remove: bool, exists: bool) {
	if selected.is_empty() {
		return;
	}

	let clicked_layer = document.find_deepest(&selected).unwrap_or_else(|| {
		LayerNodeIdentifier::ROOT_PARENT
			.children(document.metadata())
			.next()
			.expect("ROOT_PARENT should have at least one layer when clicking")
	});

	let metadata = document.metadata();

	let selected_layers = document.network_interface.selected_nodes().selected_layers(document.metadata()).collect::<Vec<_>>();
	let final_selection: Option<LayerNodeIdentifier> = (!selected_layers.is_empty() && selected_layers != vec![LayerNodeIdentifier::ROOT_PARENT]).then_some(()).and_then(|_| {
		let mut relevant_layers = document.network_interface.selected_nodes().selected_layers(document.metadata()).collect::<Vec<_>>();
		if !relevant_layers.contains(&clicked_layer) {
			relevant_layers.push(clicked_layer);
		}
		clicked_layer
			.ancestors(metadata)
			.filter(not_artboard(document))
			.find(|&ancestor| relevant_layers.iter().all(|layer| *layer == ancestor || ancestor.is_ancestor_of(metadata, layer)))
			.and_then(|least_common_ancestor| {
				let common_siblings: Vec<_> = least_common_ancestor.children(metadata).collect();
				(clicked_layer == least_common_ancestor)
					.then_some(least_common_ancestor)
					.or_else(|| common_siblings.iter().find(|&&child| clicked_layer == child || child.is_ancestor_of(metadata, &clicked_layer)).copied())
			})
	});

	if final_selection.is_some_and(|layer| selected_layers.iter().any(|selected| layer.is_child_of(metadata, selected))) {
		if exists && remove && selected_layers.len() == 1 {
			responses.add(DocumentMessage::DeselectAllLayers);
			tool_data.layers_dragging.clear();
		}
		return;
	}

	if !exists && !remove {
		responses.add(DocumentMessage::DeselectAllLayers);
		tool_data.layers_dragging.clear();
	}

	let new_selected = final_selection.unwrap_or_else(|| clicked_layer.ancestors(document.metadata()).filter(not_artboard(document)).last().unwrap_or(clicked_layer));
	// Duplicates cause `SelectedNodesSet` to carry the layer twice, breaking the Data panel's single-selection check in `node_to_inspect`
	if !tool_data.layers_dragging.contains(&new_selected) {
		tool_data.layers_dragging.push(new_selected);
	}
	tool_data.layers_dragging.retain(|&selected_layer| !selected_layer.is_child_of(metadata, &new_selected));
	if remove {
		tool_data.layers_dragging.retain(|&selected_layer| clicked_layer != selected_layer);
		if selected_layers.contains(&new_selected) {
			tool_data.layers_dragging.retain(|&selected_layer| new_selected != selected_layer);
		}
	}

	responses.add(NodeGraphMessage::SelectedNodesSet {
		nodes: tool_data
			.layers_dragging
			.iter()
			.filter_map(|layer| {
				if *layer != LayerNodeIdentifier::ROOT_PARENT {
					Some(layer.to_node())
				} else {
					log::error!("ROOT_PARENT cannot be part of tool_data.layers_dragging");
					None
				}
			})
			.collect(),
	});
}

fn layer_selected_shallowest(clicked_layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> Option<LayerNodeIdentifier> {
	let metadata = document.metadata();
	let selected_layers = document.network_interface.selected_nodes().selected_layers(document.metadata()).collect::<Vec<_>>();
	let final_selection: Option<LayerNodeIdentifier> = (!selected_layers.is_empty() && selected_layers != vec![LayerNodeIdentifier::ROOT_PARENT]).then_some(()).and_then(|_| {
		let mut relevant_layers = document.network_interface.selected_nodes().selected_layers(document.metadata()).collect::<Vec<_>>();
		if !relevant_layers.contains(&clicked_layer) {
			relevant_layers.push(clicked_layer);
		}
		clicked_layer
			.ancestors(metadata)
			.filter(not_artboard(document))
			.find(|&ancestor| relevant_layers.iter().all(|layer| *layer == ancestor || ancestor.is_ancestor_of(metadata, layer)))
			.and_then(|least_common_ancestor| {
				let common_siblings: Vec<_> = least_common_ancestor.children(metadata).collect();
				(clicked_layer == least_common_ancestor)
					.then_some(least_common_ancestor)
					.or_else(|| common_siblings.iter().find(|&&child| clicked_layer == child || child.is_ancestor_of(metadata, &clicked_layer)).copied())
			})
	});

	if final_selection.is_some_and(|layer| selected_layers.iter().any(|selected| layer.is_child_of(metadata, selected))) {
		return None;
	}

	let new_selected = final_selection.unwrap_or_else(|| clicked_layer.ancestors(document.metadata()).filter(not_artboard(document)).last().unwrap_or(clicked_layer));
	Some(new_selected)
}

fn drag_deepest_manipulation(responses: &mut VecDeque<Message>, selected: Vec<LayerNodeIdentifier>, tool_data: &mut SelectToolData, document: &DocumentMessageHandler, remove: bool) {
	let layer = document.find_deepest(&selected).unwrap_or(
		LayerNodeIdentifier::ROOT_PARENT
			.children(document.metadata())
			.next()
			.expect("ROOT_PARENT should have a layer child when clicking"),
	);

	if !remove {
		// Duplicates cause `SelectedNodesSet` to carry the layer twice, breaking the Data panel's single-selection check in `node_to_inspect`
		if !tool_data.layers_dragging.contains(&layer) {
			tool_data.layers_dragging.push(layer);
		}
	} else {
		tool_data.layers_dragging.retain(|&selected_layer| layer != selected_layer);
	}
	responses.add(NodeGraphMessage::SelectedNodesSet {
		nodes: tool_data
			.layers_dragging
			.iter()
			.filter_map(|layer| {
				if *layer != LayerNodeIdentifier::ROOT_PARENT {
					Some(layer.to_node())
				} else {
					log::error!("ROOT_PARENT cannot be part of tool_data.layers_dragging");
					None
				}
			})
			.collect(),
	});
}

/// Called when you double click on the layer of the shallowest layer.
/// If possible, the direct sibling of an old selected layer is the new selected layer.
/// Otherwise, the first non-parent ancestor is selected.
fn edit_layer_shallowest_manipulation(document: &DocumentMessageHandler, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
	let Some(new_selected) = layer.ancestors(document.metadata()).filter(not_artboard(document)).find(|ancestor| {
		ancestor
			.parent(document.metadata())
			.is_some_and(|parent| document.network_interface.selected_nodes().selected_layers_contains(parent, document.metadata()))
	}) else {
		edit_layer_deepest_manipulation(layer, &document.network_interface, responses);
		return;
	};

	if new_selected == LayerNodeIdentifier::ROOT_PARENT {
		log::error!("new_selected cannot be ROOT_PARENT");
		return;
	}

	responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![new_selected.to_node()] });
}

/// Called when a double click on a layer in deep select mode.
/// If the layer is text, the text tool is selected.
fn edit_layer_deepest_manipulation(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface, responses: &mut VecDeque<Message>) {
	if network_interface.document_metadata().is_text_layer(layer) {
		responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Text });
		responses.add(TextToolMessage::EditSelected);
	}
}

pub fn extend_lasso(lasso_polygon: &mut Vec<DVec2>, point: DVec2) {
	if lasso_polygon.len() < 2 {
		lasso_polygon.push(point);
	} else {
		let last_points = lasso_polygon.last_chunk::<2>().unwrap();
		let distance = last_points[0].distance_squared(last_points[1]);

		if distance < SELECTION_TOLERANCE.powi(2) {
			lasso_polygon.pop();
		}
		lasso_polygon.push(point);
	}
}

pub fn filter_nested_selection(metadata: &DocumentMetadata, new_selected: &HashSet<LayerNodeIdentifier>) -> HashSet<LayerNodeIdentifier> {
	// First collect childless layers
	let mut filtered_selection: HashSet<_> = new_selected.iter().copied().filter(|layer| !layer.has_children(metadata)).collect();

	// Then process parents with all children selected
	for &layer in new_selected {
		// Skip if the layer is not a parent
		if !layer.has_children(metadata) {
			continue;
		}

		// If any ancestor is already present in the filtered selection, don't include its child
		if layer.ancestors(metadata).any(|ancestor| filtered_selection.contains(&ancestor)) {
			continue;
		}

		// Skip if any of the children are not selected
		if !layer.descendants(metadata).all(|descendant| new_selected.contains(&descendant)) {
			continue;
		}

		// Remove all descendants of the parent
		for child in layer.descendants(metadata) {
			filtered_selection.remove(&child);
		}
		// Add the parent
		filtered_selection.insert(layer);
	}

	filtered_selection
}

#[cfg(test)]
mod test_select_tool {
	use super::SelectOptionsUpdate;
	use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
	use crate::messages::portfolio::document::utility_types::misc::{GridSnapping, GridType};
	use crate::messages::tool::common_functionality::snap_session::{SnapSession, WAKE_DWELL_ON_APPROACH};
	use crate::test_utils::test_prelude::*;
	use snapping::{Awake, Engagement, Highlight, Kind};
	use std::f64::consts::FRAC_PI_2;

	/// Draws a rectangle between two viewport positions, which leaves it selected, and returns its layer.
	async fn draw_rectangle(editor: &mut EditorTestUtils, from: DVec2, to: DVec2) -> LayerNodeIdentifier {
		editor.draw_rect(from.x, from.y, to.x, to.y).await;
		let layer = editor.get_selected_layer().await.expect("the drawn rectangle should be selected");
		assert_bounds(editor, layer, [from, to]);
		layer
	}

	/// The least and greatest corners of a layer's bounding box in the viewport.
	fn bounds(editor: &EditorTestUtils, layer: LayerNodeIdentifier) -> [DVec2; 2] {
		editor.active_document().metadata().bounding_box_viewport(layer).expect("the layer should have a bounding box")
	}

	fn assert_bounds(editor: &EditorTestUtils, layer: LayerNodeIdentifier, expected: [DVec2; 2]) {
		let [min, max] = bounds(editor, layer);
		let [expected_min, expected_max] = expected;
		assert!(
			min.distance(expected_min) < 1e-6 && max.distance(expected_max) < 1e-6,
			"the layer spans {min} to {max} rather than {expected_min} to {expected_max}"
		);
	}

	/// Presses the Select tool at a viewport position, which takes hold of the layer there.
	async fn press_at(editor: &mut EditorTestUtils, position: DVec2) {
		editor.select_tool(ToolType::Select).await;
		editor.move_mouse(position.x, position.y, ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(position.x, position.y, ModifierKeys::empty()).await;
	}

	async fn move_to(editor: &mut EditorTestUtils, position: DVec2, modifier_keys: ModifierKeys) {
		editor.move_mouse(position.x, position.y, modifier_keys, MouseKeys::LEFT).await;
	}

	async fn drag(editor: &mut EditorTestUtils, from: DVec2, to: DVec2, modifier_keys: ModifierKeys) {
		press_at(editor, from).await;
		move_to(editor, to, modifier_keys).await;
		editor.left_mouseup(to.x, to.y, modifier_keys).await;
	}

	/// Takes hold of the transform cage at a viewport position, once drawing the overlays has laid it around the selected layers.
	async fn press_cage_at(editor: &mut EditorTestUtils, position: DVec2) {
		editor.select_tool(ToolType::Select).await;
		editor.handle_message(SelectToolMessage::Overlays { context: OverlayContext::default() }).await;
		editor.move_mouse(position.x, position.y, ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(position.x, position.y, ModifierKeys::empty()).await;
	}

	async fn resize(editor: &mut EditorTestUtils, from: DVec2, to: DVec2, modifier_keys: ModifierKeys) {
		press_cage_at(editor, from).await;
		move_to(editor, to, modifier_keys).await;
		editor.left_mouseup(to.x, to.y, modifier_keys).await;
	}

	#[tokio::test]
	async fn a_rectangle_dragged_near_a_corner_of_another_lands_corner_to_corner() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 180.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 350.)).await;

		// Its top left corner is brought a few pixels from the other's bottom right corner
		let grab = DVec2::new(330., 320.);
		let to_the_corner = DVec2::new(200., 180.) - DVec2::new(300., 300.);
		drag(&mut editor, grab, grab + to_the_corner + DVec2::new(3., -2.), ModifierKeys::empty()).await;

		assert_bounds(&editor, layer, [DVec2::new(200., 180.), DVec2::new(260., 230.)]);
	}

	#[tokio::test]
	async fn an_anchor_away_from_the_corners_and_middles_of_a_dragged_box_lands_on_a_corner_of_another() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 180.)).await;
		editor.draw_polygon(300., 300., 400., 400.).await;
		let polygon = editor.get_selected_layer().await.expect("the drawn polygon should be selected");

		let drawn_anchors = |editor: &EditorTestUtils| {
			let metadata = editor.active_document().metadata();
			let to_viewport = metadata.transform_to_viewport(polygon);
			let anchors = metadata.layer_outline(polygon).flat_map(crate::messages::tool::common_functionality::snap_session::anchor_positions);
			anchors.map(|anchor| to_viewport.transform_point2(anchor)).collect::<Vec<_>>()
		};

		// The pentagon's leftmost corner is partway down the left side of its box
		let leftmost = drawn_anchors(&editor).into_iter().min_by(|a, b| a.x.total_cmp(&b.x)).expect("the polygon should have anchors");
		let corner = DVec2::new(200., 180.);
		let grab = DVec2::new(350., 360.);
		drag(&mut editor, grab, grab + corner - leftmost + DVec2::new(3., -2.), ModifierKeys::empty()).await;

		assert!(drawn_anchors(&editor).iter().any(|anchor| anchor.distance(corner) < 1e-6), "no anchor landed on {corner}");
	}

	#[tokio::test]
	async fn a_rectangle_dragged_nearly_level_with_one_far_beside_it_lines_up_with_it_unasked() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(160., 160.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(400., 300.), DVec2::new(440., 380.)).await;

		// Its top comes four pixels below the other's top, with hundreds of pixels between them
		let grab = DVec2::new(420., 340.);
		drag(&mut editor, grab, grab + DVec2::new(7., -196.), ModifierKeys::empty()).await;

		assert_bounds(&editor, layer, [DVec2::new(407., 100.), DVec2::new(447., 180.)]);
	}

	#[tokio::test]
	async fn a_rectangle_dragged_nearly_halfway_between_two_others_lands_with_the_same_gap_on_both_sides() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(160., 160.)).await;
		draw_rectangle(&mut editor, DVec2::new(300., 100.), DVec2::new(360., 160.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(200., 112.), DVec2::new(240., 148.)).await;

		// Halfway is ten further along, where the gap on either side comes to fifty
		let grab = DVec2::new(220., 130.);
		drag(&mut editor, grab, grab + DVec2::new(7., 0.), ModifierKeys::empty()).await;

		assert_bounds(&editor, layer, [DVec2::new(210., 112.), DVec2::new(250., 148.)]);
	}

	#[tokio::test]
	async fn on_the_grid_the_corner_nearest_where_a_rectangle_was_grabbed_lands_on_a_grid_point() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let (from, to) = (DVec2::new(300., 300.), DVec2::new(357.3, 343.7));
		let layer = draw_rectangle(&mut editor, from, to).await;
		editor.active_document_mut().snapping_state.grid_snapping = true;

		// Neither side length is a multiple of the grid spacing, so only one corner at a time can be on the grid
		let grab = to - DVec2::new(3.7, 6.1);
		drag(&mut editor, grab, grab + DVec2::new(41.3, 27.9), ModifierKeys::empty()).await;

		let document = editor.active_document();
		let state = &document.snapping_state;
		let GridType::Rectangular { spacing } = state.grid.grid_type else {
			panic!("the default grid should be rectangular")
		};
		let spacing = GridSnapping::compute_rectangle_spacing(spacing, &document.document_ptz).expect("the default grid should be usable");

		let bottom_right = document.metadata().document_to_viewport.inverse().transform_point2(bounds(&editor, layer)[1]);
		let steps = (bottom_right - state.grid.origin) / spacing;
		assert!(
			(steps - steps.round()).abs().max_element() < 1e-6,
			"the bottom right corner is at {bottom_right}, which is {steps} grid steps"
		);
	}

	#[tokio::test]
	async fn a_rectangle_dragged_with_shift_keeps_to_its_direction_and_stops_where_its_corner_meets_another_corner() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 300.), DVec2::new(160., 360.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 360.)).await;

		// The pointer strays well below the line, where the top left corner stops three pixels short of the other's top right corner
		let grab = DVec2::new(330., 330.);
		drag(&mut editor, grab, grab + DVec2::new(-137., 20.), ModifierKeys::SHIFT).await;

		assert_bounds(&editor, layer, [DVec2::new(160., 300.), DVec2::new(220., 360.)]);
	}

	#[tokio::test]
	async fn an_alt_dragged_copy_lines_up_with_the_original_and_the_original_ends_where_it_began() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let original = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 360.)).await;

		// The original is moved before Alt is pressed, which puts it back and drags a copy from there instead
		let grab = DVec2::new(330., 330.);
		press_at(&mut editor, grab).await;
		move_to(&mut editor, grab + DVec2::new(50., 0.), ModifierKeys::empty()).await;
		move_to(&mut editor, grab + DVec2::new(50., 0.), ModifierKeys::ALT).await;

		// The copy's top comes four pixels below the original's top
		let pointer = grab + DVec2::new(100., 4.);
		move_to(&mut editor, pointer, ModifierKeys::ALT).await;
		editor.left_mouseup(pointer.x, pointer.y, ModifierKeys::ALT).await;

		let layers: Vec<_> = editor.active_document().metadata().all_layers().collect();
		assert_eq!(layers.len(), 2, "there should be the original and one copy");
		let copy = layers.into_iter().find(|&layer| layer != original).expect("the copy should be a layer of its own");

		assert_bounds(&editor, original, [DVec2::new(300., 300.), DVec2::new(360., 360.)]);
		assert_bounds(&editor, copy, [DVec2::new(400., 300.), DVec2::new(460., 360.)]);
	}

	#[tokio::test]
	async fn a_second_drag_snaps_to_where_the_first_left_a_rectangle() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let first = draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(160., 160.)).await;
		let second = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 360.)).await;

		drag(&mut editor, DVec2::new(330., 330.), DVec2::new(530., 130.), ModifierKeys::empty()).await;
		assert_bounds(&editor, second, [DVec2::new(500., 100.), DVec2::new(560., 160.)]);

		// The first rectangle, taken hold of without being selected, brings its bottom right corner a few pixels from the second's new top left corner
		let grab = DVec2::new(130., 130.);
		drag(&mut editor, grab, grab + DVec2::new(343., -62.), ModifierKeys::empty()).await;
		assert_bounds(&editor, first, [DVec2::new(440., 40.), DVec2::new(500., 100.)]);
	}

	#[tokio::test]
	async fn escape_during_a_drag_puts_the_rectangle_back_and_the_next_drag_still_snaps() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(160., 160.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 360.)).await;

		let (grab, away) = (DVec2::new(330., 330.), DVec2::new(400., 420.));
		press_at(&mut editor, grab).await;
		move_to(&mut editor, away, ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, [DVec2::new(370., 390.), DVec2::new(430., 450.)]);

		editor.press(Key::Escape, ModifierKeys::empty()).await;
		editor.left_mouseup(away.x, away.y, ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, [DVec2::new(300., 300.), DVec2::new(360., 360.)]);

		// Its top left corner is brought a few pixels from the other's bottom right corner
		drag(&mut editor, grab, grab + DVec2::new(-137., -142.), ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, [DVec2::new(160., 160.), DVec2::new(220., 220.)]);
	}

	#[tokio::test]
	async fn on_a_zoomed_and_tilted_canvas_a_rectangle_still_lands_corner_to_corner() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let other = draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 180.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 350.)).await;

		let document_bounds =
			|editor: &EditorTestUtils, layer: LayerNodeIdentifier| editor.active_document().metadata().bounding_box_document(layer).expect("the rectangle should have a bounding box");
		let [top_left, bottom_right] = document_bounds(&editor, layer);
		let corner = document_bounds(&editor, other)[1];
		let grab = editor.active_document().metadata().document_to_viewport.inverse().transform_point2(DVec2::new(330., 320.));

		editor.handle_message(NavigationMessage::CanvasTiltSet { angle_radians: 30_f64.to_radians() }).await;
		editor.handle_message(NavigationMessage::CanvasZoomSet { zoom_factor: 2. }).await;

		// Its top left corner is brought a few pixels on screen from the other's bottom right corner
		let document_to_viewport = editor.active_document().metadata().document_to_viewport;
		let grab = document_to_viewport.transform_point2(grab);
		let pointer = grab + document_to_viewport.transform_vector2(corner - top_left) + DVec2::new(3., -2.);
		drag(&mut editor, grab, pointer, ModifierKeys::empty()).await;

		let [min, max] = document_bounds(&editor, layer);
		let expected = [corner, corner + bottom_right - top_left];
		assert!(
			min.distance(expected[0]) < 1e-6 && max.distance(expected[1]) < 1e-6,
			"the rectangle spans {min} to {max} in the document rather than {expected:?}"
		);
	}

	#[tokio::test]
	async fn a_rectangle_dragged_while_the_canvas_zooms_keeps_the_spot_it_was_grabbed_by_under_the_pointer() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let (from, to) = (DVec2::new(300., 300.), DVec2::new(360., 350.));
		let layer = draw_rectangle(&mut editor, from, to).await;

		let grab = DVec2::new(330., 320.);
		press_at(&mut editor, grab).await;
		move_to(&mut editor, DVec2::new(250., 270.), ModifierKeys::empty()).await;
		editor.handle_message(NavigationMessage::CanvasZoomSet { zoom_factor: 2. }).await;

		// The rectangle now appears twice as large around the spot it was grabbed by
		let pointer = DVec2::new(420., 380.);
		move_to(&mut editor, pointer, ModifierKeys::empty()).await;
		editor.left_mouseup(pointer.x, pointer.y, ModifierKeys::empty()).await;

		assert_bounds(&editor, layer, [pointer + (from - grab) * 2., pointer + (to - grab) * 2.]);
	}

	#[tokio::test]
	async fn a_rectangle_resized_by_its_corner_to_near_a_corner_of_another_meets_it_while_the_opposite_corner_stays() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 180.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 350.)).await;

		// Its top left corner is brought a few pixels from the other's bottom right corner
		resize(&mut editor, DVec2::new(300., 300.), DVec2::new(203., 178.), ModifierKeys::empty()).await;

		assert_bounds(&editor, layer, [DVec2::new(200., 180.), DVec2::new(360., 350.)]);
	}

	#[tokio::test]
	async fn a_pentagon_resized_by_its_corner_catches_a_corner_of_another_on_the_vertex_partway_down_its_side() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 180.)).await;
		editor.draw_polygon(300., 300., 400., 400.).await;
		let polygon = editor.get_selected_layer().await.expect("the drawn polygon should be selected");

		let drawn_anchors = |editor: &EditorTestUtils| {
			let metadata = editor.active_document().metadata();
			let to_viewport = metadata.transform_to_viewport(polygon);
			let anchors = metadata.layer_outline(polygon).flat_map(crate::messages::tool::common_functionality::snap_session::anchor_positions);
			anchors.map(|anchor| to_viewport.transform_point2(anchor)).collect::<Vec<_>>()
		};
		let leftmost = drawn_anchors(&editor).into_iter().min_by(|a, b| a.x.total_cmp(&b.x)).expect("the polygon should have anchors");
		let [top_left, bottom_right] = bounds(&editor, polygon);

		// The top left corner is dragged to where the stretch brings the leftmost vertex a few pixels from the other's bottom right corner
		let corner = DVec2::new(200., 180.);
		let scale = (corner + DVec2::new(3., -2.) - bottom_right) / (leftmost - bottom_right);
		resize(&mut editor, top_left, bottom_right + (top_left - bottom_right) * scale, ModifierKeys::empty()).await;

		assert!(drawn_anchors(&editor).iter().any(|anchor| anchor.distance(corner) < 1e-6), "no anchor landed on {corner}");
		assert!(bounds(&editor, polygon)[1].distance(bottom_right) < 1e-6);
	}

	#[tokio::test]
	async fn on_a_zoomed_and_tilted_canvas_a_rectangle_resized_by_its_corner_meets_a_corner_of_another() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let other = draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 180.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 350.)).await;

		let document_bounds =
			|editor: &EditorTestUtils, layer: LayerNodeIdentifier| editor.active_document().metadata().bounding_box_document(layer).expect("the rectangle should have a bounding box");
		let [top_left, bottom_right] = document_bounds(&editor, layer);
		let corner = document_bounds(&editor, other)[1];

		editor.handle_message(NavigationMessage::CanvasTiltSet { angle_radians: 30_f64.to_radians() }).await;
		editor.handle_message(NavigationMessage::CanvasZoomSet { zoom_factor: 2. }).await;

		// Its top left corner is brought a few pixels on screen from the other's bottom right corner
		let document_to_viewport = editor.active_document().metadata().document_to_viewport;
		let pointer = document_to_viewport.transform_point2(corner) + DVec2::new(3., -2.);
		resize(&mut editor, document_to_viewport.transform_point2(top_left), pointer, ModifierKeys::empty()).await;

		let [min, max] = document_bounds(&editor, layer);
		assert!(
			min.distance(corner) < 1e-6 && max.distance(bottom_right) < 1e-6,
			"the rectangle spans {min} to {max} in the document rather than {corner} to {bottom_right}"
		);
	}

	#[tokio::test]
	async fn a_corner_snapped_while_resizing_holds_on_as_the_pointer_strays_beyond_where_it_would_have_caught() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 180.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 350.)).await;
		let (near, further) = (DVec2::new(203., 178.), DVec2::new(208.4, 171.6));

		// Brought straight to the further place, the top left corner is too far from the other's bottom right corner, and the lines through it, to catch on them
		press_cage_at(&mut editor, DVec2::new(300., 300.)).await;
		move_to(&mut editor, further, ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, [further, DVec2::new(360., 350.)]);
		editor.press(Key::Escape, ModifierKeys::empty()).await;
		editor.left_mouseup(further.x, further.y, ModifierKeys::empty()).await;

		// Caught on the way there, it stays caught
		press_cage_at(&mut editor, DVec2::new(300., 300.)).await;
		move_to(&mut editor, near, ModifierKeys::empty()).await;
		move_to(&mut editor, further, ModifierKeys::empty()).await;
		editor.left_mouseup(further.x, further.y, ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, [DVec2::new(200., 180.), DVec2::new(360., 350.)]);
	}

	#[tokio::test]
	async fn a_rectangle_stretched_by_its_right_edge_to_nearly_the_width_of_one_beside_it_lands_on_that_width() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 200.), DVec2::new(170., 270.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 200.), DVec2::new(350., 250.)).await;

		// Seventy wide, as the other is, puts the right edge at 370, wherever the pointer is up or down
		resize(&mut editor, DVec2::new(350., 225.), DVec2::new(367., 231.), ModifierKeys::empty()).await;

		assert_bounds(&editor, layer, [DVec2::new(300., 200.), DVec2::new(370., 250.)]);
	}

	#[tokio::test]
	async fn a_horizontal_line_lengthened_by_its_end_to_near_the_edge_of_a_rectangle_lands_on_it() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(500., 250.), DVec2::new(560., 400.)).await;
		editor.drag_tool(ToolType::Line, 300., 300., 400., 300., ModifierKeys::empty()).await;
		let line = editor.get_selected_layer().await.expect("the drawn line should be selected");

		// The box around the line has no height, so its end stretches only along it, to a few pixels short of the rectangle's left edge
		resize(&mut editor, DVec2::new(400., 300.), DVec2::new(497., 304.), ModifierKeys::empty()).await;

		assert_bounds(&editor, line, [DVec2::new(300., 300.), DVec2::new(500., 300.)]);
	}

	#[tokio::test]
	async fn a_rectangle_stretched_toward_one_beside_it_lands_where_its_gaps_on_either_side_are_equal() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		// The rectangle to stretch is drawn first, since drawing it beside the others would already even out its gaps
		let layer = draw_rectangle(&mut editor, DVec2::new(200., 110.), DVec2::new(242., 160.)).await;
		draw_rectangle(&mut editor, DVec2::new(80., 100.), DVec2::new(150., 170.)).await;
		draw_rectangle(&mut editor, DVec2::new(300., 100.), DVec2::new(360., 170.)).await;
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] }).await;

		// The gap on the left is fifty, which the gap on the right comes to with the right edge at 250, short of where either width would put it
		resize(&mut editor, DVec2::new(242., 135.), DVec2::new(247., 135.), ModifierKeys::empty()).await;

		assert_bounds(&editor, layer, [DVec2::new(200., 110.), DVec2::new(250., 160.)]);
	}

	#[tokio::test]
	async fn a_rectangle_resized_with_shift_keeps_its_proportions_and_stops_where_its_corner_meets_the_edge_of_another() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(420., 360.), DVec2::new(480., 460.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 345.)).await;

		// Twice the size puts the bottom right corner on the other's left edge, which the pointer stops a few pixels short of
		resize(&mut editor, DVec2::new(360., 345.), DVec2::new(417., 388.), ModifierKeys::SHIFT).await;

		assert_bounds(&editor, layer, [DVec2::new(300., 300.), DVec2::new(420., 390.)]);
	}

	#[tokio::test]
	async fn a_rectangle_resized_with_alt_stretches_about_its_center_and_its_corner_meets_a_corner_of_another() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(380., 380.), DVec2::new(440., 440.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 360.)).await;

		// Its bottom right corner is brought a few pixels from the other's top left corner, and its top left corner goes as far the other way
		resize(&mut editor, DVec2::new(360., 360.), DVec2::new(377., 382.), ModifierKeys::ALT).await;

		assert_bounds(&editor, layer, [DVec2::new(280., 280.), DVec2::new(380., 380.)]);
	}

	#[tokio::test]
	async fn a_second_resize_snaps_to_where_the_first_left_a_rectangle() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(160., 160.)).await;
		let other = draw_rectangle(&mut editor, DVec2::new(420., 420.), DVec2::new(480., 480.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 360.)).await;

		resize(&mut editor, DVec2::new(300., 300.), DVec2::new(163., 158.), ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, [DVec2::new(160., 160.), DVec2::new(360., 360.)]);

		// Once the resize is over, nothing is shown as snapped
		assert!(editor.editor.dispatcher.message_handlers.tool_message_handler.snap_session.shown().is_none());

		// The other rectangle, selected by a click, has its top left corner brought a few pixels from the first one's bottom right corner
		let inside_the_other = DVec2::new(450., 450.);
		drag(&mut editor, inside_the_other, inside_the_other, ModifierKeys::empty()).await;
		resize(&mut editor, DVec2::new(420., 420.), DVec2::new(363., 358.), ModifierKeys::empty()).await;
		assert_bounds(&editor, other, [DVec2::new(360., 360.), DVec2::new(480., 480.)]);
	}

	#[tokio::test]
	async fn escape_during_a_resize_puts_the_rectangle_back_and_the_next_resize_still_snaps() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(160., 160.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 360.)).await;

		let away = DVec2::new(420., 430.);
		press_cage_at(&mut editor, DVec2::new(360., 360.)).await;
		move_to(&mut editor, away, ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, [DVec2::new(300., 300.), away]);

		editor.press(Key::Escape, ModifierKeys::empty()).await;
		editor.left_mouseup(away.x, away.y, ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, [DVec2::new(300., 300.), DVec2::new(360., 360.)]);

		// Its top left corner is brought a few pixels from the other's bottom right corner
		resize(&mut editor, DVec2::new(300., 300.), DVec2::new(163., 158.), ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, [DVec2::new(160., 160.), DVec2::new(360., 360.)]);
	}

	/// A rectangle turned about its middle by a press just beyond its bottom right corner, which is a hundred from the middle.
	const TURNED: [DVec2; 2] = [DVec2::new(290., 270.), DVec2::new(410., 430.)];
	const TURNED_MIDDLE: DVec2 = DVec2::new(350., 350.);
	const TURN_GRIP: DVec2 = DVec2::new(424., 444.);

	/// Where the turn with a cosine of 0.6 and a sine of 0.8 carries the bottom right corner of [`TURNED`].
	const ANCHOR_IN_THE_TURN: DVec2 = DVec2::new(322., 446.);

	/// The turn that carries the bottom right corner of [`TURNED`] onto [`ANCHOR_IN_THE_TURN`], in radians.
	fn onto_the_anchor() -> f64 {
		DVec2::new(0.6, 0.8).to_angle()
	}

	/// Where a turn about a pivot through an angle in radians carries a position.
	fn turned(position: DVec2, pivot: DVec2, angle: f64) -> DVec2 {
		pivot + DVec2::from_angle(angle).rotate(position - pivot)
	}

	/// The least and greatest corners of the box around a rectangle once turned about a pivot through an angle in radians.
	fn turned_bounds([from, to]: [DVec2; 2], pivot: DVec2, angle: f64) -> [DVec2; 2] {
		let corners = [from, DVec2::new(to.x, from.y), to, DVec2::new(from.x, to.y)].map(|corner| turned(corner, pivot, angle));
		[corners.into_iter().fold(DVec2::INFINITY, DVec2::min), corners.into_iter().fold(DVec2::NEG_INFINITY, DVec2::max)]
	}

	/// Draws a small rectangle with its top right corner at [`ANCHOR_IN_THE_TURN`], then the [`TURNED`] rectangle, which is left selected and returned.
	async fn draw_a_rectangle_to_turn_onto_an_anchor(editor: &mut EditorTestUtils) -> LayerNodeIdentifier {
		draw_rectangle(editor, ANCHOR_IN_THE_TURN - DVec2::new(10., 0.), ANCHOR_IN_THE_TURN + DVec2::new(0., 10.)).await;
		draw_rectangle(editor, TURNED[0], TURNED[1]).await
	}

	#[tokio::test]
	async fn a_rectangle_turned_until_its_corner_nears_an_anchor_in_its_path_lands_with_the_corner_on_it() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let layer = draw_a_rectangle_to_turn_onto_an_anchor(&mut editor).await;

		// Two degrees short of the anchor, the bottom right corner is a few pixels from it
		let pointer = turned(TURN_GRIP, TURNED_MIDDLE, onto_the_anchor() - 2_f64.to_radians());
		press_cage_at(&mut editor, TURN_GRIP).await;
		move_to(&mut editor, pointer, ModifierKeys::empty()).await;

		// The grip stays as far from the middle as it began, so the rectangle only turns
		let to_document = editor.active_document().metadata().document_to_viewport.inverse();
		let from_the_middle = |position: DVec2| position.distance(to_document.transform_point2(TURNED_MIDDLE));
		let session = &editor.editor.dispatcher.message_handlers.tool_message_handler.snap_session;
		let grip = session.shown_position().expect("the turn should have tried to snap");
		assert!((from_the_middle(grip) - from_the_middle(to_document.transform_point2(TURN_GRIP))).abs() < 1e-9);

		editor.left_mouseup(pointer.x, pointer.y, ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, turned_bounds(TURNED, TURNED_MIDDLE, onto_the_anchor()));
	}

	#[tokio::test]
	async fn a_rectangle_turned_with_shift_steps_to_a_multiple_of_the_angle_step_and_snaps_to_nothing() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let layer = draw_a_rectangle_to_turn_onto_an_anchor(&mut editor).await;

		// Where the corner would otherwise land on the anchor, the turn of about 51 degrees steps down to 45
		let pointer = turned(TURN_GRIP, TURNED_MIDDLE, onto_the_anchor() - 2_f64.to_radians());
		press_cage_at(&mut editor, TURN_GRIP).await;
		move_to(&mut editor, pointer, ModifierKeys::SHIFT).await;
		assert!(editor.editor.dispatcher.message_handlers.tool_message_handler.snap_session.shown().is_none());
		editor.left_mouseup(pointer.x, pointer.y, ModifierKeys::SHIFT).await;

		assert_bounds(&editor, layer, turned_bounds(TURNED, TURNED_MIDDLE, 45_f64.to_radians()));
	}

	#[tokio::test]
	async fn a_rectangle_turned_with_shift_steps_by_the_angle_step_the_document_sets() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let layer = draw_rectangle(&mut editor, TURNED[0], TURNED[1]).await;
		editor.handle_message(DocumentMessage::SetSnappingAngleStep { degrees: 30. }).await;

		// The turn of about 51 degrees steps up to 60 rather than down to 45
		let pointer = turned(TURN_GRIP, TURNED_MIDDLE, onto_the_anchor() - 2_f64.to_radians());
		press_cage_at(&mut editor, TURN_GRIP).await;
		move_to(&mut editor, pointer, ModifierKeys::SHIFT).await;
		editor.left_mouseup(pointer.x, pointer.y, ModifierKeys::SHIFT).await;

		assert_bounds(&editor, layer, turned_bounds(TURNED, TURNED_MIDDLE, 60_f64.to_radians()));
	}

	#[tokio::test]
	async fn a_rectangle_turned_with_the_grid_on_and_nothing_nearby_turns_as_far_as_the_pointer_asks() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let layer = draw_rectangle(&mut editor, TURNED[0], TURNED[1]).await;
		editor.active_document_mut().snapping_state.grid_snapping = true;

		let angle = 23.7_f64.to_radians();
		let pointer = turned(TURN_GRIP, TURNED_MIDDLE, angle);
		press_cage_at(&mut editor, TURN_GRIP).await;
		move_to(&mut editor, pointer, ModifierKeys::empty()).await;
		editor.left_mouseup(pointer.x, pointer.y, ModifierKeys::empty()).await;

		assert_bounds(&editor, layer, turned_bounds(TURNED, TURNED_MIDDLE, angle));
	}

	#[tokio::test]
	async fn escape_during_a_turn_puts_the_rectangle_back_and_ends_the_snapping_gesture() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let layer = draw_a_rectangle_to_turn_onto_an_anchor(&mut editor).await;
		let shows_an_attempt = |editor: &EditorTestUtils| editor.editor.dispatcher.message_handlers.tool_message_handler.snap_session.shown().is_some();

		let angle = 30_f64.to_radians();
		let pointer = turned(TURN_GRIP, TURNED_MIDDLE, angle);
		press_cage_at(&mut editor, TURN_GRIP).await;
		move_to(&mut editor, pointer, ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, turned_bounds(TURNED, TURNED_MIDDLE, angle));
		assert!(shows_an_attempt(&editor));

		editor.press(Key::Escape, ModifierKeys::empty()).await;
		editor.left_mouseup(pointer.x, pointer.y, ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, TURNED);
		assert!(!shows_an_attempt(&editor));
	}

	/// Where a press takes hold of the skew handle beside the middle of the top edge of a rectangle from (300, 300) to (420, 380).
	const TOP_SKEW_HANDLE: DVec2 = DVec2::new(370.5, 300.);

	#[tokio::test]
	async fn a_rectangle_skewed_by_its_top_edge_until_a_corner_nears_a_corner_of_another_lands_on_it_while_the_bottom_stays() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(480., 260.), DVec2::new(520., 300.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(420., 380.)).await;

		// The top right corner is slid to three pixels short of the other's bottom left corner, with the pointer straying downward
		let pointer = TOP_SKEW_HANDLE + DVec2::new(57., 5.);
		press_cage_at(&mut editor, TOP_SKEW_HANDLE).await;
		move_to(&mut editor, pointer, ModifierKeys::empty()).await;
		editor.left_mouseup(pointer.x, pointer.y, ModifierKeys::empty()).await;

		assert_bounds(&editor, layer, [DVec2::new(300., 300.), DVec2::new(480., 380.)]);
	}

	#[tokio::test]
	async fn a_rectangle_skewed_with_control_held_moves_its_top_edge_across_itself_as_well_as_along_it() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(420., 380.)).await;

		// The top edge goes thirty along and twenty up, while the bottom edge stays
		let pointer = TOP_SKEW_HANDLE + DVec2::new(30., -20.);
		press_cage_at(&mut editor, TOP_SKEW_HANDLE).await;
		move_to(&mut editor, pointer, ModifierKeys::CONTROL).await;
		editor.left_mouseup(pointer.x, pointer.y, ModifierKeys::CONTROL).await;

		assert_bounds(&editor, layer, [DVec2::new(300., 280.), DVec2::new(450., 380.)]);
	}

	/// Turns on the pivot that the Select tool turns and scales about, which starts in the middle of the selection.
	async fn turn_on_the_pivot(editor: &mut EditorTestUtils) {
		editor.select_tool(ToolType::Select).await;
		let options = SelectOptionsUpdate::SetPivotGizmoEnabled(true);
		editor.handle_message(SelectToolMessage::SelectOptions { options }).await;
	}

	/// Drags the pivot, once drawing the overlays has placed it, from one viewport position to another.
	async fn drag_pivot(editor: &mut EditorTestUtils, from: DVec2, to: DVec2) {
		press_cage_at(editor, from).await;
		move_to(editor, to, ModifierKeys::empty()).await;
		editor.left_mouseup(to.x, to.y, ModifierKeys::empty()).await;
	}

	/// Turns the selected rectangle from (300, 300) to (420, 380) a quarter turn about a pivot, held by Shift to a whole number of steps.
	async fn turn_a_quarter_about(editor: &mut EditorTestUtils, pivot: DVec2) {
		let grip = DVec2::new(434., 394.);
		let pointer = turned(grip, pivot, FRAC_PI_2);
		press_cage_at(editor, grip).await;
		move_to(editor, pointer, ModifierKeys::SHIFT).await;
		editor.left_mouseup(pointer.x, pointer.y, ModifierKeys::SHIFT).await;
	}

	#[tokio::test]
	async fn the_pivot_dragged_near_a_corner_of_its_layer_lands_on_it() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let (from, to) = (DVec2::new(300., 300.), DVec2::new(420., 380.));
		let layer = draw_rectangle(&mut editor, from, to).await;
		turn_on_the_pivot(&mut editor).await;

		// From the middle, the pivot is brought a few pixels from the top left corner, which a turn then goes about
		drag_pivot(&mut editor, DVec2::new(360., 340.), from + DVec2::new(3., -2.)).await;
		turn_a_quarter_about(&mut editor, from).await;

		assert_bounds(&editor, layer, turned_bounds([from, to], from, FRAC_PI_2));
	}

	#[tokio::test]
	async fn the_pivot_dragged_near_the_middle_of_its_layer_lands_there() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let (from, to) = (DVec2::new(300., 300.), DVec2::new(420., 380.));
		let layer = draw_rectangle(&mut editor, from, to).await;
		turn_on_the_pivot(&mut editor).await;

		// The pivot is taken out of the way, then brought back to a few pixels from the middle, which a turn then goes about
		let (middle, away) = (DVec2::new(360., 340.), DVec2::new(500., 220.));
		drag_pivot(&mut editor, middle, away).await;
		drag_pivot(&mut editor, away, middle + DVec2::new(2., -3.)).await;
		turn_a_quarter_about(&mut editor, middle).await;

		assert_bounds(&editor, layer, turned_bounds([from, to], middle, FRAC_PI_2));
	}

	#[tokio::test]
	async fn a_typed_turn_after_escaping_a_pivot_drag_and_panning_goes_about_the_restored_pivot() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let (from, to) = (DVec2::new(300., 300.), DVec2::new(420., 380.));
		let layer = draw_rectangle(&mut editor, from, to).await;
		turn_on_the_pivot(&mut editor).await;

		// The pivot is taken out of the way, put back in the middle by Esc, then followed by a pan of the view
		let middle = DVec2::new(360., 340.);
		press_cage_at(&mut editor, middle).await;
		move_to(&mut editor, DVec2::new(500., 220.), ModifierKeys::empty()).await;
		editor.press(Key::Escape, ModifierKeys::empty()).await;
		editor.left_mouseup(500., 220., ModifierKeys::empty()).await;
		let pan = DVec2::new(40., -30.);
		editor.handle_message(NavigationMessage::CanvasPan { delta: pan }).await;
		editor.handle_message(SelectToolMessage::Overlays { context: OverlayContext::default() }).await;

		editor.move_mouse(600., 500., ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.handle_message(TransformLayerMessage::BeginRotate).await;
		for digit in [9, 0] {
			editor.handle_message(TransformLayerMessage::TypeDigit { digit }).await;
		}
		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;

		assert_bounds(&editor, layer, turned_bounds([from + pan, to + pan], middle + pan, FRAC_PI_2));
	}

	fn snap_session(editor: &EditorTestUtils) -> &SnapSession {
		&editor.editor.dispatcher.message_handlers.tool_message_handler.snap_session
	}

	/// What the latest attempt to snap engaged.
	fn engaged(editor: &EditorTestUtils) -> Engagement {
		let engagement = snap_session(editor).shown().and_then(|shown| shown.outcome.engagement());
		engagement.cloned().expect("the latest attempt to snap should have engaged something")
	}

	/// Moves the pointer, with no button down, to a viewport position.
	async fn hover(editor: &mut EditorTestUtils, position: DVec2) {
		editor.move_mouse(position.x, position.y, ModifierKeys::empty(), MouseKeys::empty()).await;
	}

	/// Rests the pointer at a viewport position from a time in milliseconds for as long as waking takes before any press, and returns the anchor this wakes.
	async fn wake_anchor(editor: &mut EditorTestUtils, position: DVec2, time: u64) -> Awake {
		let awake_before = snap_session(editor).awake().to_vec();
		editor.animation_frame(time).await;
		hover(editor, position).await;
		editor.animation_frame(time + WAKE_DWELL_ON_APPROACH).await;

		let latest = snap_session(editor).awake().last().copied();
		let Some(anchor @ Awake::Anchor { .. }) = latest.filter(|latest| !awake_before.contains(latest)) else {
			panic!("resting at {position} should have woken an anchor")
		};
		anchor
	}

	/// Checks that the latest attempt to snap lined the pointer up with a woken anchor, which put it at a viewport position.
	fn assert_aligned_with(editor: &EditorTestUtils, anchor: Awake, expected: DVec2) {
		let Awake::Anchor { layer, index } = anchor else { panic!("{anchor:?} should be an anchor") };
		let session = snap_session(editor);

		let lines_up = |candidate: &snapping::Candidate| candidate.key.kind == Kind::Alignment && candidate.highlight == Some(Highlight::Anchor { layer, index });
		assert!(
			session.shown().is_some_and(|shown| shown.outcome.engaged_candidates().any(lines_up)),
			"the pointer should be lined up with {anchor:?}"
		);

		let document_to_viewport = editor.active_document().metadata().document_to_viewport;
		let landed = session.shown_position().map(|position| document_to_viewport.transform_point2(position));
		assert!(
			landed.is_some_and(|landed| landed.abs_diff_eq(expected, 1e-6)),
			"the pointer landed at {landed:?} rather than {expected}"
		);
	}

	#[tokio::test]
	async fn holding_q_through_a_drag_lets_the_rectangle_follow_the_pointer_until_q_is_let_go() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 180.)).await;
		let layer = draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 350.)).await;
		let on_the_corner = [DVec2::new(200., 180.), DVec2::new(260., 230.)];

		// Its top left corner is brought three pixels from the other's bottom right corner, where it snaps
		let grab = DVec2::new(330., 320.);
		let mut pointer = grab + DVec2::new(-97., -122.);
		press_at(&mut editor, grab).await;
		move_to(&mut editor, pointer, ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, on_the_corner);

		// Putting Q down lets go of the corner at once, and the rectangle follows the pointer exactly while Q is held
		editor.key_down(Key::KeyQ, ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, [DVec2::new(203., 178.), DVec2::new(263., 228.)]);
		pointer += DVec2::X;
		move_to(&mut editor, pointer, ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, [DVec2::new(204., 178.), DVec2::new(264., 228.)]);
		assert!(!snap_session(&editor).caught());

		// Letting Q go snaps it back onto the corner, where it stays as the pointer moves on
		editor.key_up(Key::KeyQ, ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, on_the_corner);
		pointer += DVec2::X;
		move_to(&mut editor, pointer, ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, on_the_corner);
		assert!(snap_session(&editor).caught());

		editor.left_mouseup(pointer.x, pointer.y, ModifierKeys::empty()).await;
		assert_bounds(&editor, layer, on_the_corner);
	}

	#[tokio::test]
	async fn tapping_w_through_a_drag_steps_on_to_another_snap_within_reach_once_per_tap_and_holds_it() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 180.)).await;
		draw_rectangle(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 350.)).await;

		// Three pixels from the other's bottom right corner, the top left corner has that corner and the lines through it within reach
		let grab = DVec2::new(330., 320.);
		let mut pointer = grab + DVec2::new(-97., -122.);
		press_at(&mut editor, grab).await;
		move_to(&mut editor, pointer, ModifierKeys::empty()).await;
		let natural = engaged(&editor);
		assert!(natural.alternatives >= 2 && !natural.pinned, "there should be more than one snap to step through");

		// W steps on as soon as it goes down, and staying down as the pointer moves steps no further
		editor.key_down(Key::KeyW, ModifierKeys::empty()).await;
		let stepped = engaged(&editor);
		assert_ne!(stepped.key, natural.key);
		assert!(stepped.pinned);
		pointer += DVec2::X;
		move_to(&mut editor, pointer, ModifierKeys::empty()).await;
		assert_eq!(engaged(&editor).key, stepped.key);
		editor.key_up(Key::KeyW, ModifierKeys::empty()).await;

		// Another tap steps on again, and what it reaches holds as the pointer moves on
		editor.press(Key::KeyW, ModifierKeys::empty()).await;
		pointer += DVec2::X;
		move_to(&mut editor, pointer, ModifierKeys::empty()).await;
		let stepped_again = engaged(&editor);
		assert_ne!(stepped_again.key, stepped.key);
		assert!(stepped_again.pinned);

		editor.left_mouseup(pointer.x, pointer.y, ModifierKeys::empty()).await;
	}

	#[tokio::test]
	async fn a_corner_rested_on_before_drawing_lines_the_pointer_up_from_afar_until_escape_puts_it_back_to_sleep() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 180.)).await;
		editor.handle_message(DocumentMessage::DeselectAllLayers).await;

		// With the Rectangle tool ready, the pointer drawing left on the corner is taken away, then rests beside the corner until it wakes
		hover(&mut editor, DVec2::new(600., 500.)).await;
		editor.animation_frame(1000).await;
		hover(&mut editor, DVec2::new(202., 182.)).await;
		editor.animation_frame(1000 + WAKE_DWELL_ON_APPROACH - 1).await;
		assert!(snap_session(&editor).awake().is_empty());
		editor.animation_frame(1000 + WAKE_DWELL_ON_APPROACH).await;
		let &[woken @ Awake::Anchor { .. }] = snap_session(&editor).awake() else {
			panic!("only the corner should be awake, not {:?}", snap_session(&editor).awake())
		};

		// Three hundred pixels away, the pointer lines up with it
		let (far_but_level, level) = (DVec2::new(500., 182.), DVec2::new(500., 180.));
		hover(&mut editor, far_but_level).await;
		assert_aligned_with(&editor, woken, level);

		// Drawing another rectangle elsewhere leaves it awake and still lined up with, as the next move shows
		editor.drag_path(&[DVec2::new(400., 300.), DVec2::new(460., 360.)], ModifierKeys::empty()).await;
		let drawn = editor.get_selected_layer().await.expect("the rectangle drawn last should be selected");
		assert_bounds(&editor, drawn, [DVec2::new(400., 300.), DVec2::new(460., 360.)]);
		hover(&mut editor, far_but_level).await;
		assert_eq!(snap_session(&editor).awake(), [woken]);
		assert_aligned_with(&editor, woken, level);

		// Escape with nothing under way puts it back to sleep
		editor.press(Key::Escape, ModifierKeys::empty()).await;
		assert!(snap_session(&editor).awake().is_empty());
		hover(&mut editor, far_but_level + DVec2::Y).await;
		assert!(!snap_session(&editor).caught());
	}

	#[tokio::test]
	async fn with_the_pen_tool_a_corner_rested_on_before_the_first_point_lines_the_pointer_up_from_afar() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 180.)).await;
		editor.handle_message(DocumentMessage::DeselectAllLayers).await;
		editor.select_tool(ToolType::Pen).await;

		hover(&mut editor, DVec2::new(600., 500.)).await;
		editor.animation_frame(1000).await;
		hover(&mut editor, DVec2::new(202., 182.)).await;
		editor.animation_frame(1000 + WAKE_DWELL_ON_APPROACH).await;
		let &[woken @ Awake::Anchor { .. }] = snap_session(&editor).awake() else {
			panic!("only the corner should be awake, not {:?}", snap_session(&editor).awake())
		};

		let (far_but_level, level) = (DVec2::new(500., 182.), DVec2::new(500., 180.));
		hover(&mut editor, far_but_level).await;
		assert_aligned_with(&editor, woken, level);
	}

	#[tokio::test]
	async fn with_the_pen_tool_the_middle_of_an_edge_rested_on_before_the_first_point_lines_the_pointer_up_from_afar() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 180.)).await;
		editor.handle_message(DocumentMessage::DeselectAllLayers).await;
		editor.select_tool(ToolType::Pen).await;

		// Resting beside the middle of the top edge wakes that point, not the edge
		hover(&mut editor, DVec2::new(600., 500.)).await;
		editor.animation_frame(1000).await;
		hover(&mut editor, DVec2::new(152., 102.)).await;
		let resting_on: Vec<Kind> = snap_session(&editor).shown().unwrap().outcome.engaged_candidates().map(|candidate| candidate.key.kind).collect();
		assert_eq!(resting_on, vec![Kind::Midpoint]);
		editor.animation_frame(1000 + WAKE_DWELL_ON_APPROACH).await;
		let &[Awake::Position { position, .. }] = snap_session(&editor).awake() else {
			panic!("only the middle of the edge should be awake, not {:?}", snap_session(&editor).awake())
		};
		let document_to_viewport = editor.active_document().metadata().document_to_viewport;
		assert!(document_to_viewport.transform_point2(position).abs_diff_eq(DVec2::new(150., 100.), 1e-6));

		// Far below it, the pointer lines up with it
		hover(&mut editor, DVec2::new(153., 400.)).await;
		let kinds: Vec<Kind> = snap_session(&editor).shown().unwrap().outcome.engaged_candidates().map(|candidate| candidate.key.kind).collect();
		assert_eq!(kinds, vec![Kind::Alignment]);
		let landed = snap_session(&editor).shown_position().map(|position| document_to_viewport.transform_point2(position));
		assert!(landed.is_some_and(|landed| landed.abs_diff_eq(DVec2::new(150., 400.), 1e-6)), "the pointer landed at {landed:?}");
	}

	#[tokio::test]
	async fn a_sixth_corner_rested_on_puts_the_first_one_woken_back_to_sleep() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_rectangle(&mut editor, DVec2::new(100., 100.), DVec2::new(160., 140.)).await;
		draw_rectangle(&mut editor, DVec2::new(230., 190.), DVec2::new(310., 250.)).await;
		draw_rectangle(&mut editor, DVec2::new(390., 290.), DVec2::new(430., 370.)).await;
		editor.handle_message(DocumentMessage::DeselectAllLayers).await;

		// The top left and bottom right corners of each rectangle, none level with another, are rested on in turn from two pixels outside
		let beside_corners = [(98., 98.), (162., 142.), (228., 188.), (312., 252.), (388., 288.), (432., 372.)].map(DVec2::from);
		let [first_five @ .., sixth] = beside_corners;
		let mut woken = Vec::new();
		for (index, beside) in first_five.into_iter().enumerate() {
			woken.push(wake_anchor(&mut editor, beside, 1000 + index as u64 * 2000).await);
		}

		// Five stay awake, and the first still lines the pointer up from afar
		assert_eq!(snap_session(&editor).awake(), woken);
		hover(&mut editor, DVec2::new(600., 102.)).await;
		assert_aligned_with(&editor, woken[0], DVec2::new(600., 100.));

		// The sixth puts the first back to sleep, while the rest stay awake in the order they woke
		woken.push(wake_anchor(&mut editor, sixth, 11_000).await);
		assert_eq!(snap_session(&editor).awake(), &woken[1..]);
		hover(&mut editor, DVec2::new(600., 102.)).await;
		assert!(!snap_session(&editor).caught());
		hover(&mut editor, DVec2::new(600., 142.)).await;
		assert_aligned_with(&editor, woken[1], DVec2::new(600., 140.));
	}
}
