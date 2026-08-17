use super::tool_prelude::*;
use crate::consts::{COLOR_OVERLAY_BLUE, DRAG_THRESHOLD, HIDE_HANDLE_DISTANCE, LINE_ROTATE_SNAP_ANGLE, MANIPULATOR_GROUP_MARKER_SIZE, SEGMENT_INSERTION_DISTANCE, SEGMENT_OVERLAY_SIZE};
use crate::messages::portfolio::document::overlays::utility_functions::overlay_bezier_handles;
use crate::messages::portfolio::document::overlays::utility_types::{GizmoEmphasis, OverlayContext};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::graph_modification_utils::{
	self, MeshGradientPaint, NodeGraphLayer, get_fill_node_id_with_direct_fill_input, get_mesh_gradient_paint, get_upstream_mesh_gradient_value_node_id,
};
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapData, SnapManager, SnapTypeConfiguration};
use crate::messages::tool::utility_types::ToolRefreshOptions;
use graphene_std::color::SRGBA8;
use graphene_std::raster::color::Color;
use graphene_std::subpath::{BezierHandles, pathseg_points};
use graphene_std::vector::algorithms::util::pathseg_tangent;
use graphene_std::vector::misc::{dvec2_to_point, point_to_dvec2};
use graphene_std::vector::style::{GradientSpace, MeshGradientSurface};
use graphene_std::vector::{GradientInterpolation, HandleId, MeshGradient, SegmentId};
use kurbo::{DEFAULT_ACCURACY, ParamCurve, ParamCurveNearest};

#[derive(Default, ExtractField)]
pub struct MeshGradientTool {
	fsm_state: MeshGradientToolFsmState,
	data: MeshGradientToolData,
	options: MeshGradientOptions,
}

pub struct MeshGradientOptions {
	space: GradientSpace,
	interpolation: GradientInterpolation,
}

impl Default for MeshGradientOptions {
	fn default() -> Self {
		let MeshGradientSurface {
			gradient_space,
			gradient_interpolation,
			..
		} = MeshGradientSurface::default();
		Self {
			space: gradient_space,
			interpolation: gradient_interpolation,
		}
	}
}

#[impl_message(Message, ToolMessage, MeshGradient)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum MeshGradientToolMessage {
	// Standard messages
	Abort,
	Overlays { context: OverlayContext },
	SelectionChanged,

	// Tool-specific messages
	DeleteEdge,
	DoubleClick,
	PointerDown,
	PointerMove { constrain_axis: Key },
	PointerOutsideViewport { constrain_axis: Key },
	PointerUp,
	StartTransactionForColorStop,
	CommitTransactionForColorStop,
	CloseStopColorPicker,
	UpdateStopColor { color: Color },
	UpdateOptions { options: MeshGradientOptionsUpdate },
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Eq, Clone, Debug, Hash, serde::Serialize, serde::Deserialize)]
pub enum MeshGradientOptionsUpdate {
	Space(GradientSpace),
	Interpolation(GradientInterpolation),
}

impl ToolMetadata for MeshGradientTool {
	fn icon_name(&self) -> String {
		"GeneralGradientTool".into()
	}
	fn tooltip_label(&self) -> String {
		"Mesh Gradient Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::MeshGradient
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for MeshGradientTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		match message {
			ToolMessage::MeshGradient(MeshGradientToolMessage::UpdateOptions { options }) => {
				match options {
					MeshGradientOptionsUpdate::Space(space) => self.options.space = space,
					MeshGradientOptionsUpdate::Interpolation(interpolation) => self.options.interpolation = interpolation,
				}

				// Write back only the setting that actually changed, so a layer whose other setting differs keeps it
				apply_mesh_gradient_options(context, responses, |surface| match &options {
					MeshGradientOptionsUpdate::Space(space) => surface.gradient_space = *space,
					MeshGradientOptionsUpdate::Interpolation(interpolation) => surface.gradient_interpolation = *interpolation,
				});
				self.refresh_options(responses);
			}
			ToolMessage::MeshGradient(MeshGradientToolMessage::SelectionChanged) => {
				if let Some(surface) = first_selected_mesh_gradient_surface(context.document) {
					self.options.space = surface.gradient_space;
					self.options.interpolation = surface.gradient_interpolation;
					self.refresh_options(responses);
				}
				self.fsm_state.process_event(message, &mut self.data, context, &self.options, responses, false);
			}
			ToolMessage::MeshGradient(MeshGradientToolMessage::StartTransactionForColorStop) => {
				if self.data.color_picker_transaction_open {
					responses.add(DocumentMessage::EndTransaction);
				}
				responses.add(DocumentMessage::StartTransaction);
				self.data.color_picker_transaction_open = true;
			}
			ToolMessage::MeshGradient(MeshGradientToolMessage::CommitTransactionForColorStop) => {
				if self.data.color_picker_transaction_open {
					responses.add(DocumentMessage::EndTransaction);
					self.data.color_picker_transaction_open = false;
				}
			}
			ToolMessage::MeshGradient(MeshGradientToolMessage::UpdateStopColor { color }) => {
				let Some(selected_mesh) = self.data.selected_mesh.as_mut() else { return };

				if let MeshGradientTarget::Corner { corner_index, .. } = selected_mesh.target
					&& self.data.color_picker_editing_color_stop == Some(corner_index)
					&& selected_mesh.surface.mesh.set_corner_color(corner_index, color).is_some()
				{
					selected_mesh.update_gradient_in_graph(responses);
					responses.add(PropertiesPanelMessage::Refresh);
					responses.add(OverlaysMessage::Draw);
				}
			}
			ToolMessage::MeshGradient(MeshGradientToolMessage::CloseStopColorPicker) => {
				if self.data.color_picker_transaction_open {
					responses.add(DocumentMessage::EndTransaction);
					self.data.color_picker_transaction_open = false;
				}
				self.data.color_picker_editing_color_stop = None;
			}
			_ => {
				self.fsm_state.process_event(message, &mut self.data, context, &self.options, responses, false);

				if let Some(surface) = first_selected_mesh_gradient_surface(context.document) {
					let mut needs_refresh = false;
					if self.options.space != surface.gradient_space {
						self.options.space = surface.gradient_space;
						needs_refresh = true;
					}
					if self.options.interpolation != surface.gradient_interpolation {
						self.options.interpolation = surface.gradient_interpolation;
						needs_refresh = true;
					}
					if needs_refresh {
						self.refresh_options(responses);
					}
				}
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(MeshGradientToolMessageDiscriminant;
			UpdateOptions,
			PointerDown,
			PointerUp,
			PointerMove,
			DoubleClick,
			DeleteEdge,
			Abort,
		)
	}
}

impl LayoutHolder for MeshGradientTool {
	fn layout(&self) -> Layout {
		let space_entries = graph_modification_utils::mesh_gradient_space_sections()
			.into_iter()
			.map(|section| {
				section
					.into_iter()
					.map(|(space, metadata)| {
						MenuListEntry::new(metadata.name)
							.label(metadata.label)
							.tooltip_label(metadata.label)
							.tooltip_description(metadata.description.unwrap_or_default())
							.on_update(move |_| {
								MeshGradientToolMessage::UpdateOptions {
									options: MeshGradientOptionsUpdate::Space(space),
								}
								.into()
							})
					})
					.collect()
			})
			.collect();
		let space = DropdownInput::new(space_entries)
			.selected_index(graph_modification_utils::mesh_gradient_space_index(self.options.space))
			.tooltip_description("The color space the mesh interpolates its corner colors through.")
			.widget_instance();

		let interpolation_entries = MenuListEntry::sections_from_choice_type(|interpolation| {
			MeshGradientToolMessage::UpdateOptions {
				options: MeshGradientOptionsUpdate::Interpolation(interpolation),
			}
			.into()
		});
		let interpolation = DropdownInput::new(interpolation_entries)
			.selected_index(Some(self.options.interpolation as u32))
			.tooltip_description("The path the corners interpolate along, deciding whether the gradient jumps, turns corners, or flows smoothly through them.")
			.widget_instance();

		Layout(vec![LayoutGroup::row(vec![
			TextLabel::new("Space").widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			space,
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextLabel::new("Interpolation").widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			interpolation,
		])])
	}
}

/// The mesh gradient a layer paints.
fn layer_mesh_gradient_paint(document: &DocumentMessageHandler, layer: LayerNodeIdentifier) -> Option<MeshGradientPaint> {
	get_mesh_gradient_paint(layer, &document.network_interface, || document.metadata().nonzero_bounding_box(layer))
}

/// Returns the first mesh gradient painted by the selection, paired with the settings riding alongside it.
fn first_selected_mesh_gradient_surface(document: &DocumentMessageHandler) -> Option<MeshGradientSurface> {
	document
		.network_interface
		.selected_nodes()
		.selected_visible_layers(&document.network_interface)
		.find_map(|layer| layer_mesh_gradient_paint(document, layer).map(|paint| paint.surface))
}

/// Whether the layer's fill already paints a mesh gradient.
fn layer_paints_mesh_gradient(document: &DocumentMessageHandler, layer: LayerNodeIdentifier) -> bool {
	layer_mesh_gradient_paint(document, layer).is_some()
}

/// Rewrites the settings of the first mesh gradient of every selected layer, leaving its geometry and colors alone.
fn apply_mesh_gradient_options(context: &mut ToolActionMessageContext, responses: &mut VecDeque<Message>, update: impl Fn(&mut MeshGradientSurface)) {
	let document = &context.document;
	let selected_layers: Vec<_> = document.network_interface.selected_nodes().selected_visible_layers(&document.network_interface).collect();

	let mut transaction_started = false;
	for layer in selected_layers {
		let Some(source) = resolve_mesh_gradient_source(layer, &document.network_interface) else {
			continue;
		};
		let Some(mut surface) = layer_mesh_gradient_paint(document, layer).map(|paint| paint.surface) else {
			continue;
		};
		update(&mut surface);

		if !transaction_started {
			responses.add(DocumentMessage::StartTransaction);
			transaction_started = true;
		}
		responses.add(match source {
			GradientSource::Direct => GraphOperationMessage::FillMeshGradientSet { layer, mesh_gradient: surface },
			GradientSource::Chain => GraphOperationMessage::MeshGradientSet { layer, mesh_gradient: surface },
		});
	}

	if transaction_started {
		responses.add(DocumentMessage::EndTransaction);
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MeshGradientToolFsmState {
	Ready {
		hovering: MeshGradientHoverTarget,
		selected: MeshGradientSelectedTarget,
	},
	Dragging,
}

impl Default for MeshGradientToolFsmState {
	fn default() -> Self {
		Self::Ready {
			hovering: MeshGradientHoverTarget::None,
			selected: MeshGradientSelectedTarget::None,
		}
	}
}

#[derive(Clone, Debug, PartialEq)]
struct SelectedMeshGradient {
	layer: LayerNodeIdentifier,
	surface: MeshGradientSurface,
	mesh_to_document: DAffine2,
	source: GradientSource,
	target: MeshGradientTarget,
}

impl SelectedMeshGradient {
	pub fn update_gradient_in_graph(&mut self, responses: &mut VecDeque<Message>) {
		let message = match self.source {
			GradientSource::Direct => GraphOperationMessage::FillMeshGradientSet {
				layer: self.layer,
				mesh_gradient: self.surface.clone(),
			},
			GradientSource::Chain => GraphOperationMessage::MeshGradientSet {
				layer: self.layer,
				mesh_gradient: self.surface.clone(),
			},
		};
		responses.add(message);
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GradientSource {
	Direct,
	Chain,
}

fn resolve_mesh_gradient_source(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<GradientSource> {
	if get_fill_node_id_with_direct_fill_input(layer, network_interface).is_some() {
		Some(GradientSource::Direct)
	} else if get_upstream_mesh_gradient_value_node_id(layer, network_interface).is_some() {
		Some(GradientSource::Chain)
	} else {
		None
	}
}

fn approximate_valid_region_bounds(initial_position: DVec2, [min, max]: [DVec2; 2], mut is_valid: impl FnMut(DVec2) -> bool) -> Option<[DVec2; 2]> {
	const SUBDIVISIONS: usize = 12;

	let mut x_samples = (0..=SUBDIVISIONS).map(|index| min.x + (max.x - min.x) * index as f64 / SUBDIVISIONS as f64).collect::<Vec<_>>();
	let mut y_samples = (0..=SUBDIVISIONS).map(|index| min.y + (max.y - min.y) * index as f64 / SUBDIVISIONS as f64).collect::<Vec<_>>();
	x_samples.push(initial_position.x);
	y_samples.push(initial_position.y);
	x_samples.sort_by(f64::total_cmp);
	y_samples.sort_by(f64::total_cmp);
	x_samples.dedup();
	y_samples.dedup();

	let columns = x_samples.len();
	let rows = y_samples.len();
	let seed_column = x_samples.iter().position(|&x| x == initial_position.x)?;
	let seed_row = y_samples.iter().position(|&y| y == initial_position.y)?;
	let seed_index = seed_row * columns + seed_column;

	let valid_samples = y_samples.iter().flat_map(|&y| x_samples.iter().map(move |&x| DVec2::new(x, y))).map(&mut is_valid).collect::<Vec<_>>();

	if !valid_samples[seed_index] {
		return None;
	}

	let mut visited = vec![false; rows * columns];
	let mut queue = VecDeque::from([seed_index]);
	let mut bounds_min = initial_position;
	let mut bounds_max = initial_position;

	while let Some(index) = queue.pop_front() {
		if visited[index] || !valid_samples[index] {
			continue;
		}
		visited[index] = true;

		let row = index / columns;
		let column = index % columns;
		let position = DVec2::new(x_samples[column], y_samples[row]);
		bounds_min = bounds_min.min(position);
		bounds_max = bounds_max.max(position);

		if row > 0 {
			queue.push_back(index - columns);
		}
		if row + 1 < rows {
			queue.push_back(index + columns);
		}
		if column > 0 {
			queue.push_back(index - 1);
		}
		if column + 1 < columns {
			queue.push_back(index + 1);
		}
	}

	Some([bounds_min, bounds_max])
}

/// Walks back from `target` toward the valid region's center for the furthest position that keeps the mesh free of foldovers.
fn constrain_to_valid_region(
	target: DVec2,
	valid_region_center: &mut Option<DVec2>,
	resolve_center: impl FnOnce() -> DVec2,
	candidate: impl Fn(DVec2) -> Option<MeshGradient>,
) -> Option<MeshGradient> {
	if let Some(gradient) = candidate(target) {
		return Some(gradient);
	}

	const BINARY_SEARCH_ITERATIONS: usize = 12;
	let center = *valid_region_center.get_or_insert_with(resolve_center);
	let mut valid_t = 0.;
	let mut invalid_t = 1.;
	let mut valid_gradient = candidate(center)?;

	for _ in 0..BINARY_SEARCH_ITERATIONS {
		let mid_t = (valid_t + invalid_t) / 2.;
		let mid_position = center.lerp(target, mid_t);

		if let Some(gradient) = candidate(mid_position) {
			valid_t = mid_t;
			valid_gradient = gradient;
		} else {
			invalid_t = mid_t;
		}
	}

	Some(valid_gradient)
}

#[derive(Clone, Debug, PartialEq)]
enum MeshGradientTarget {
	Corner {
		corner_index: usize,
		initial_mouse: DVec2,
		initial_corner: DVec2,
		/// Resolved on the first frame the drag leaves the valid region, then reused for the rest of the drag.
		valid_region_center: Option<DVec2>,
	},
	Segment {
		segment_id: SegmentId,
		initial_mouse: DVec2,
		initial_handles: [DVec2; 2],
		/// Resolved on the first frame the drag leaves the valid region, then reused for the rest of the drag.
		valid_region_center: Option<DVec2>,
	},
	Handle {
		handle_id: HandleId,
		initial_mouse: DVec2,
		initial_handle: DVec2,
		/// Resolved on the first frame the drag leaves the valid region, then reused for the rest of the drag.
		valid_region_center: Option<DVec2>,
	},
}

impl ToolTransition for MeshGradientTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(MeshGradientToolMessage::Abort.into()),
			selection_changed: Some(MeshGradientToolMessage::SelectionChanged.into()),
			overlay_provider: Some(|context| MeshGradientToolMessage::Overlays { context }.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Debug, Default)]
struct MeshGradientToolData {
	selected_mesh: Option<SelectedMeshGradient>,
	snap_manager: SnapManager,
	drag_start: DVec2,
	/// The pointer-down position before snapping (document space), used to detect whether the mouse moved between the press and a double-click.
	drag_start_unsnapped: DVec2,
	auto_panning: AutoPanning,
	auto_pan_shift: DVec2,
	color_picker_editing_color_stop: Option<usize>,
	color_picker_transaction_open: bool,
}

impl Fsm for MeshGradientToolFsmState {
	type ToolData = MeshGradientToolData;
	type ToolOptions = MeshGradientOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		tool_action_data: &mut ToolActionMessageContext,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let ToolActionMessageContext { document, input, viewport, .. } = tool_action_data;
		let ToolMessage::MeshGradient(event) = event else { return self };

		match (self, event) {
			(_, MeshGradientToolMessage::Overlays { context: mut overlay_context }) => {
				let metadata = document.metadata();
				let mut hovered_segment: Option<(f64, DVec2, DVec2)> = None;
				let mut hovering_corner = false;

				for layer in document.network_interface.selected_nodes().selected_visible_layers(&document.network_interface) {
					let Some(paint) = layer_mesh_gradient_paint(document, layer) else {
						continue;
					};

					let layer_to_viewport = metadata.transform_to_viewport(layer);

					{
						let mesh = &paint.surface.mesh;

						let mesh_to_viewport = layer_to_viewport * paint.transform;
						let geometry = mesh.geometry();

						// Render the mesh geometry's outline in the same manner as the path tool does
						if overlay_context.visibility_settings.path() {
							overlay_context.outline_vector(geometry, mesh_to_viewport);
						}

						if let Some(selected_segment_id) = tool_data.selected_mesh.as_ref().and_then(|selected_mesh| {
							if selected_mesh.layer != layer {
								return None;
							}
							match selected_mesh.target {
								MeshGradientTarget::Segment { segment_id, .. } => Some(segment_id),
								_ => None,
							}
						}) && let Some(edge) = mesh.edges().find(|edge| edge.segment_id == selected_segment_id)
						{
							overlay_context.outline_select_bezier(edge.segment, mesh_to_viewport);
						}

						if overlay_context.visibility_settings.handles() {
							for (segment_id, bezier, _, _) in geometry.segment_bezier_iter() {
								overlay_bezier_handles(bezier, segment_id, mesh_to_viewport, |_| false, &mut overlay_context);
							}
						}

						if overlay_context.visibility_settings.anchors() {
							for &position in geometry.point_domain.positions() {
								overlay_context.manipulator_anchor(mesh_to_viewport.transform_point2(position), false, None);
							}
						}

						// Then, place the color stop gizmos for all mesh corners
						for corner in mesh.corners() {
							let position = mesh_to_viewport.transform_point2(corner.position);
							let color = SRGBA8::from(corner.color).to_css_hex();
							hovering_corner |= position.distance_squared(input.mouse.position) < (MANIPULATOR_GROUP_MARKER_SIZE * 2.).powi(2);

							let is_selected = tool_data.selected_mesh.as_ref().is_some_and(|selected_mesh| {
								matches!(
									selected_mesh.target,
									MeshGradientTarget::Corner{corner_index, ..}
										if selected_mesh.layer == layer
											&& corner_index == corner.index
								)
							});

							let emphasis = if is_selected { GizmoEmphasis::Active } else { GizmoEmphasis::Regular };

							overlay_context.gradient_color_stop(position, emphasis, &color, false);
						}

						// Display the normal line overray when the mouse is on a edge
						if !hovering_corner {
							let local_mouse = mesh_to_viewport.inverse().transform_point2(input.mouse.position);
							for edge in mesh.edges() {
								let t = edge.segment.nearest(dvec2_to_point(local_mouse), DEFAULT_ACCURACY).t.clamp(0., 1.);
								let closest_local = point_to_dvec2(edge.segment.eval(t));
								let closest_viewport = mesh_to_viewport.transform_point2(closest_local);
								let distance_squared = closest_viewport.distance_squared(input.mouse.position);

								if distance_squared > SEGMENT_INSERTION_DISTANCE.powi(2) {
									continue;
								}

								let tangent_local = pathseg_tangent(edge.segment, t);
								let Some(tangent_viewport) = mesh_to_viewport.transform_vector2(tangent_local).try_normalize() else {
									continue;
								};
								let normal_viewport = tangent_viewport.perp();
								if hovered_segment.as_ref().is_none_or(|(closest_distance, _, _)| distance_squared < *closest_distance) {
									hovered_segment = Some((distance_squared, closest_viewport, normal_viewport));
								}
							}
						}
					}
				}

				if matches!(self, MeshGradientToolFsmState::Ready { .. })
					&& !hovering_corner
					&& let Some((_, point, normal)) = hovered_segment
				{
					overlay_context.line(point - normal * SEGMENT_OVERLAY_SIZE, point + normal * SEGMENT_OVERLAY_SIZE, Some(COLOR_OVERLAY_BLUE), None);
				}

				tool_data.snap_manager.draw_overlays(SnapData::new(document, input, viewport), &mut overlay_context);

				match self {
					MeshGradientToolFsmState::Ready { selected, .. } => MeshGradientToolFsmState::Ready {
						hovering: if hovering_corner {
							MeshGradientHoverTarget::Corner
						} else if hovered_segment.is_some() {
							MeshGradientHoverTarget::Segment
						} else {
							MeshGradientHoverTarget::None
						},
						selected,
					},
					_ => self,
				}
			}
			(state, MeshGradientToolMessage::SelectionChanged) => {
				if matches!(state, MeshGradientToolFsmState::Dragging) {
					responses.add(DocumentMessage::AbortTransaction);
					tool_data.snap_manager.cleanup(responses);
				} else if tool_data.color_picker_transaction_open {
					responses.add(DocumentMessage::EndTransaction);
				}
				tool_data.color_picker_transaction_open = false;
				tool_data.color_picker_editing_color_stop = None;
				tool_data.selected_mesh = None;
				responses.add(OverlaysMessage::Draw);

				MeshGradientToolFsmState::default()
			}

			(_state @ MeshGradientToolFsmState::Ready { .. }, MeshGradientToolMessage::DeleteEdge) => {
				let Some(selected_mesh) = tool_data.selected_mesh.as_mut() else { return self };
				let MeshGradientTarget::Segment { segment_id, .. } = selected_mesh.target else { return self };
				let mut mesh = selected_mesh.surface.mesh.clone();
				if mesh.remove_edge(segment_id).is_none() {
					return self;
				}
				selected_mesh.surface.mesh = mesh;

				responses.add(DocumentMessage::StartTransaction);
				selected_mesh.update_gradient_in_graph(responses);
				responses.add(DocumentMessage::EndTransaction);
				tool_data.selected_mesh = None;
				responses.add(OverlaysMessage::Draw);

				MeshGradientToolFsmState::Ready {
					hovering: MeshGradientHoverTarget::None,
					selected: MeshGradientSelectedTarget::None,
				}
			}

			(_, MeshGradientToolMessage::DoubleClick) => {
				// Ignore when dragging
				let drag_start_viewport = document.metadata().document_to_viewport.transform_point2(tool_data.drag_start_unsnapped);
				if input.mouse.position.distance(drag_start_viewport) > DRAG_THRESHOLD {
					return self;
				}

				let Some(selected_mesh) = tool_data.selected_mesh.as_mut() else { return self };
				let mesh_to_viewport = document.metadata().document_to_viewport * selected_mesh.mesh_to_document;

				match selected_mesh.target {
					// Display color picker when the mesh corner color gizmo is double clicked
					MeshGradientTarget::Corner { corner_index, .. } => {
						let Some(corner) = selected_mesh.surface.mesh.corners().find(|corner| corner.index == corner_index) else {
							return self;
						};

						tool_data.color_picker_editing_color_stop = Some(corner.index);

						let position = mesh_to_viewport.transform_point2(corner.position).into();
						responses.add(FrontendMessage::UpdateGradientStopColorPickerPosition { color: corner.color.into(), position });
					}
					MeshGradientTarget::Segment { segment_id, .. } => {
						let Some(segment) = selected_mesh.surface.mesh.edges().find(|edge| edge.segment_id == segment_id) else {
							return self;
						};
						let local_mouse = mesh_to_viewport.inverse().transform_point2(input.mouse.position);
						let time = segment.segment.nearest(dvec2_to_point(local_mouse), DEFAULT_ACCURACY).t.clamp(0., 1.);
						if selected_mesh
							.surface
							.mesh
							.insert_grid_line(segment.segment_id, selected_mesh.surface.gradient_space, selected_mesh.surface.gradient_interpolation, time)
							.is_none()
						{
							return self;
						}

						responses.add(DocumentMessage::StartTransaction);
						selected_mesh.update_gradient_in_graph(responses);
						responses.add(DocumentMessage::EndTransaction);
						responses.add(OverlaysMessage::Draw);

						// Inserting a grid line removes the selected segment, so discard its now-stale ID and deletion hint.
						tool_data.selected_mesh = None;
						return MeshGradientToolFsmState::default();
					}
					_ => {}
				};

				self
			}

			(MeshGradientToolFsmState::Ready { .. }, MeshGradientToolMessage::PointerDown) => {
				let metadata = document.metadata();
				let document_to_viewport = metadata.document_to_viewport;
				let mouse = input.mouse.position;
				let document_mouse = document_to_viewport.inverse().transform_point2(mouse);
				tool_data.drag_start = document_mouse;
				tool_data.drag_start_unsnapped = document_mouse;
				tool_data.auto_pan_shift = DVec2::ZERO;
				let tolerance_squared = (MANIPULATOR_GROUP_MARKER_SIZE * 2.).powi(2);

				for layer in document.network_interface.selected_nodes().selected_visible_layers(&document.network_interface) {
					let Some(paint) = layer_mesh_gradient_paint(document, layer) else {
						continue;
					};
					let Some(source) = resolve_mesh_gradient_source(layer, &document.network_interface) else {
						continue;
					};

					let layer_to_viewport = metadata.transform_to_viewport(layer);

					{
						let gradient = &paint.surface.mesh;

						let mesh_to_viewport = layer_to_viewport * paint.transform;
						let mesh_to_document = document_to_viewport.inverse() * mesh_to_viewport;
						let local_mouse = mesh_to_viewport.inverse().transform_point2(mouse);

						// Change the corner position. Hit check on corners should have higher priority than the segments.
						for corner in gradient.corners() {
							let corner_in_viewport = mesh_to_viewport.transform_point2(corner.position);
							let distance_squared = corner_in_viewport.distance_squared(mouse);

							if distance_squared < tolerance_squared {
								responses.add(DocumentMessage::StartTransaction);

								tool_data.selected_mesh = Some(SelectedMeshGradient {
									layer,
									surface: paint.surface.clone(),
									mesh_to_document,
									source,
									target: MeshGradientTarget::Corner {
										corner_index: corner.index,
										initial_mouse: local_mouse,
										initial_corner: corner.position,
										valid_region_center: None,
									},
								});

								return MeshGradientToolFsmState::Dragging;
							}
						}

						let mut closest_handle: Option<(HandleId, DVec2, f64)> = None;
						let hidden_distance_squared = HIDE_HANDLE_DISTANCE.powi(2);

						// Change the handle position.
						for (segment_id, bezier, _, _) in gradient.geometry().segment_bezier_iter() {
							let mut consider_handle = |handle_id: HandleId, handle: DVec2, anchor: DVec2, _other_anchor: Option<DVec2>| {
								let handle_viewport = mesh_to_viewport.transform_point2(handle);
								let anchor_viewport = mesh_to_viewport.transform_point2(anchor);

								// Ignore handles that is not displayed in the overlay
								if handle_viewport.distance_squared(anchor_viewport) < hidden_distance_squared {
									return;
								}

								let distance_squared = handle_viewport.distance_squared(mouse);
								if distance_squared < tolerance_squared && closest_handle.as_ref().is_none_or(|(_, _, closest_distance)| distance_squared < *closest_distance) {
									closest_handle = Some((handle_id, handle, distance_squared));
								}
							};

							match bezier.handles {
								BezierHandles::Linear => {}
								BezierHandles::Quadratic { handle } => {
									consider_handle(HandleId::primary(segment_id), handle, bezier.start, Some(bezier.end));
								}
								BezierHandles::Cubic { handle_start, handle_end } => {
									consider_handle(HandleId::primary(segment_id), handle_start, bezier.start, None);
									consider_handle(HandleId::end(segment_id), handle_end, bezier.end, None);
								}
							}
						}

						// Resolved only after every segment has been offered, so the nearest-wins comparison spans the whole mesh
						if let Some((handle_id, initial_handle, _)) = closest_handle {
							responses.add(DocumentMessage::StartTransaction);

							tool_data.selected_mesh = Some(SelectedMeshGradient {
								layer,
								surface: paint.surface.clone(),
								mesh_to_document,
								source,
								target: MeshGradientTarget::Handle {
									handle_id,
									initial_mouse: local_mouse,
									initial_handle,
									valid_region_center: None,
								},
							});

							return MeshGradientToolFsmState::Dragging;
						}

						for edge in gradient.edges() {
							// Mold the mesh edge by dragging the segment directly while keeping the corners fixed.
							let t = edge.segment.nearest(dvec2_to_point(local_mouse), DEFAULT_ACCURACY).t;
							let closest_position_in_viewport = mesh_to_viewport.transform_point2(point_to_dvec2(edge.segment.eval(t)));
							let distance_squared = closest_position_in_viewport.distance_squared(mouse);

							if distance_squared < tolerance_squared {
								let points = pathseg_points(edge.segment);

								let handles = match (points.p1, points.p2) {
									(Some(p1), Some(p2)) => [p1, p2],
									(Some(control), None) | (None, Some(control)) => [points.p0 + (control - points.p0) * 2. / 3., points.p3 + (control - points.p3) * 2. / 3.],
									(None, None) => [points.p0 + (points.p3 - points.p0) / 3., points.p3 + (points.p0 - points.p3) / 3.],
								};

								responses.add(DocumentMessage::StartTransaction);

								tool_data.selected_mesh = Some(SelectedMeshGradient {
									layer,
									surface: paint.surface.clone(),
									mesh_to_document,
									source,
									target: MeshGradientTarget::Segment {
										segment_id: edge.segment_id,
										initial_mouse: local_mouse,
										initial_handles: handles,
										valid_region_center: None,
									},
								});

								return MeshGradientToolFsmState::Dragging;
							}
						}
					}
				}

				// No gizmo was under the cursor, so the click falls through to the layer beneath it
				let Some(layer) = document.click_based_on_position(document_mouse) else { return self };
				if NodeGraphLayer::is_raster_layer(layer, &mut document.network_interface) {
					return self;
				}

				if !document.network_interface.selected_nodes().selected_layers_contains(layer, document.metadata()) {
					responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] });
				}

				// A layer already painted with a mesh gradient is only selected, leaving its mesh as it stands to be edited
				if layer_paints_mesh_gradient(document, layer) {
					responses.add(OverlaysMessage::Draw);
					return self;
				}

				// Otherwise the layer's paint, whatever it was, gives way to a fresh mesh gradient held as the Fill node's value
				responses.add(DocumentMessage::StartTransaction);
				responses.add(GraphOperationMessage::FillMeshGradientSet {
					layer,
					mesh_gradient: MeshGradientSurface {
						mesh: MeshGradient::default(),
						gradient_space: tool_options.space,
						gradient_interpolation: tool_options.interpolation,
					},
				});
				responses.add(DocumentMessage::EndTransaction);
				responses.add(OverlaysMessage::Draw);

				self
			}
			(MeshGradientToolFsmState::Dragging, MeshGradientToolMessage::PointerMove { constrain_axis }) => {
				let MeshGradientToolData {
					selected_mesh,
					snap_manager,
					auto_panning,
					auto_pan_shift,
					..
				} = tool_data;
				let Some(selected_mesh) = selected_mesh.as_mut() else { return self };

				let document_to_viewport = document.metadata().document_to_viewport;
				let mesh_to_document = selected_mesh.mesh_to_document;
				let mut mesh_to_viewport = document_to_viewport * mesh_to_document;
				mesh_to_viewport.translation += *auto_pan_shift;
				*auto_pan_shift = DVec2::ZERO;

				let current_local_mouse = mesh_to_viewport.inverse().transform_point2(input.mouse.position);
				let snap_data = SnapData::new(document, input, viewport);
				let snap_angle = input.keyboard.get(constrain_axis as usize);
				let mut snap_local_point = |origin_local: DVec2, local_point: DVec2| {
					if snap_angle {
						snap_manager.clear_indicator();

						let origin_viewport = mesh_to_viewport.transform_point2(origin_local);
						let local_point_viewport = mesh_to_viewport.transform_point2(local_point);
						let delta = origin_viewport - local_point_viewport;
						let length = delta.length();
						if length <= f64::EPSILON {
							return local_point;
						}

						let snap_resolution = LINE_ROTATE_SNAP_ANGLE.to_radians();
						let angle = (-delta.angle_to(DVec2::X) / snap_resolution).round() * snap_resolution;
						let rotated = DVec2::new(length * angle.cos(), length * angle.sin());
						return mesh_to_viewport.inverse().transform_point2(origin_viewport - rotated);
					}

					let document_point = mesh_to_document.transform_point2(local_point);
					let point = SnapCandidatePoint::gradient_handle(document_point);
					let snapped = snap_manager.free_snap(&snap_data, &point, SnapTypeConfiguration::default());
					let local_point = if snapped.is_snapped() {
						mesh_to_document.inverse().transform_point2(snapped.snapped_point_document)
					} else {
						local_point
					};
					snap_manager.update_indicator(snapped);
					local_point
				};

				match &mut selected_mesh.target {
					MeshGradientTarget::Corner {
						corner_index,
						initial_mouse,
						initial_corner,
						valid_region_center,
					} => {
						let corner_index = *corner_index;
						let initial_mouse = *initial_mouse;
						let initial_corner = *initial_corner;
						let desired_position = initial_corner + current_local_mouse - initial_mouse;
						let snapped_local_mouse = snap_local_point(initial_corner, desired_position);
						let mesh = &selected_mesh.surface.mesh;
						let candidate_gradient = |position| {
							let mut gradient = mesh.clone();
							gradient.set_corner_position(corner_index, position)?;
							let is_valid = gradient.patches().all(|patch| patch.is_some_and(|patch| patch.sampled_no_foldover()));
							is_valid.then_some(gradient)
						};
						let resolve_center = || {
							mesh.geometry()
								.bounding_box()
								.and_then(|bounds| approximate_valid_region_bounds(initial_corner, bounds, |position| candidate_gradient(position).is_some()))
								.map(|[min, max]| min.midpoint(max))
								.unwrap_or(initial_corner)
						};
						let constrained_gradient = constrain_to_valid_region(snapped_local_mouse, valid_region_center, resolve_center, candidate_gradient);

						if let Some(gradient) = constrained_gradient {
							selected_mesh.surface.mesh = gradient;
							selected_mesh.update_gradient_in_graph(responses);
							responses.add(OverlaysMessage::Draw);
						}
					}
					MeshGradientTarget::Segment {
						segment_id,
						initial_mouse: initial_local_mouse,
						initial_handles,
						valid_region_center,
					} => {
						let snapped_local_mouse = snap_local_point(*initial_local_mouse, current_local_mouse);
						let initial_local_mouse = *initial_local_mouse;
						let mesh = &selected_mesh.surface.mesh;
						let candidate_gradient = |mouse_position: DVec2| {
							let delta = mouse_position - initial_local_mouse;
							let mut gradient = mesh.clone();
							gradient.set_edge_handles(
								*segment_id,
								BezierHandles::Cubic {
									handle_start: initial_handles[0] + delta,
									handle_end: initial_handles[1] + delta,
								},
							)?;
							let is_valid = gradient.patches().all(|patch| patch.is_some_and(|patch| patch.sampled_no_foldover()));
							is_valid.then_some(gradient)
						};
						let resolve_center = || {
							mesh.geometry()
								.bounding_box()
								.and_then(|bounds| approximate_valid_region_bounds(initial_local_mouse, bounds, |position| candidate_gradient(position).is_some()))
								.map(|[min, max]| min.midpoint(max))
								.unwrap_or(initial_local_mouse)
						};

						if let Some(gradient) = constrain_to_valid_region(snapped_local_mouse, valid_region_center, resolve_center, candidate_gradient) {
							selected_mesh.surface.mesh = gradient;
							selected_mesh.update_gradient_in_graph(responses);
							responses.add(OverlaysMessage::Draw);
						}
					}
					MeshGradientTarget::Handle {
						handle_id,
						initial_mouse,
						initial_handle,
						valid_region_center,
					} => {
						let delta = current_local_mouse - *initial_mouse;
						let new_handle_position = snap_local_point(*initial_handle, *initial_handle + delta);
						let initial_handle = *initial_handle;
						let mesh = &selected_mesh.surface.mesh;
						let candidate_gradient = |position| {
							let mut gradient = mesh.clone();
							gradient.set_handle_position(*handle_id, position)?;
							let is_valid = gradient.patches().all(|patch| patch.is_some_and(|patch| patch.sampled_no_foldover()));
							is_valid.then_some(gradient)
						};
						let resolve_center = || {
							mesh.geometry()
								.bounding_box()
								.and_then(|bounds| approximate_valid_region_bounds(initial_handle, bounds, |position| candidate_gradient(position).is_some()))
								.map(|[min, max]| min.midpoint(max))
								.unwrap_or(initial_handle)
						};

						if let Some(gradient) = constrain_to_valid_region(new_handle_position, valid_region_center, resolve_center, candidate_gradient) {
							selected_mesh.surface.mesh = gradient;
							selected_mesh.update_gradient_in_graph(responses);
							responses.add(OverlaysMessage::Draw);
						}
					}
				};

				// Auto-panning
				let messages = [
					MeshGradientToolMessage::PointerOutsideViewport { constrain_axis }.into(),
					MeshGradientToolMessage::PointerMove { constrain_axis }.into(),
				];
				auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);

				MeshGradientToolFsmState::Dragging
			}

			(MeshGradientToolFsmState::Dragging, MeshGradientToolMessage::PointerUp) => {
				let Some(selected_mesh) = tool_data.selected_mesh.as_ref() else { return self };
				let selected = match selected_mesh.target {
					MeshGradientTarget::Corner { .. } => MeshGradientSelectedTarget::Corner,
					MeshGradientTarget::Segment { .. } => MeshGradientSelectedTarget::Segment,
					MeshGradientTarget::Handle { .. } => MeshGradientSelectedTarget::Handle,
				};

				responses.add(DocumentMessage::EndTransaction);
				tool_data.snap_manager.cleanup(responses);
				responses.add(OverlaysMessage::Draw);

				MeshGradientToolFsmState::Ready {
					hovering: MeshGradientHoverTarget::None,
					selected,
				}
			}
			(MeshGradientToolFsmState::Dragging, MeshGradientToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.snap_manager.cleanup(responses);
				tool_data.selected_mesh = None;
				responses.add(OverlaysMessage::Draw);

				MeshGradientToolFsmState::default()
			}

			(MeshGradientToolFsmState::Dragging, MeshGradientToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				if let Some(shift) = tool_data.auto_panning.shift_viewport(input, viewport, responses) {
					tool_data.auto_pan_shift += shift;
				}

				MeshGradientToolFsmState::Dragging
			}
			(state, MeshGradientToolMessage::PointerOutsideViewport { constrain_axis }) => {
				let messages = [
					MeshGradientToolMessage::PointerOutsideViewport { constrain_axis }.into(),
					MeshGradientToolMessage::PointerMove { constrain_axis }.into(),
				];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}

			(state @ MeshGradientToolFsmState::Ready { .. }, MeshGradientToolMessage::PointerMove { .. }) => {
				responses.add(OverlaysMessage::Draw);
				state
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			MeshGradientToolFsmState::Ready { hovering, selected } => {
				let mut groups = match hovering {
					MeshGradientHoverTarget::None => vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Paint Layer with Mesh")])],
					MeshGradientHoverTarget::Corner => vec![
						HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Move Corner")]),
						HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDouble, "Edit Color")]),
					],
					MeshGradientHoverTarget::Segment => vec![
						HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Select Segment")]),
						HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Mold Segment")]),
						HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDouble, "Insert Grid Line")]),
					],
				};

				if matches!(selected, MeshGradientSelectedTarget::Segment) {
					groups.push(HintGroup(vec![HintInfo::keys([Key::Backspace], "Delete Grid Line")]));
				}

				HintData(groups)
			}
			MeshGradientToolFsmState::Dragging => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])]),
		};

		hint_data.send_layout(responses);
	}

	fn update_cursor(&self, _responses: &mut VecDeque<Message>) {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum MeshGradientHoverTarget {
	#[default]
	None,
	Corner,
	Segment,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum MeshGradientSelectedTarget {
	#[default]
	None,
	Corner,
	Segment,
	Handle,
}
