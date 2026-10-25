use crate::consts::{GIZMO_HIDE_THRESHOLD, HEART_ANGLE_HANDLE_DISTANCE, HEART_HANDLE_HOVER_RADIUS, HEART_SLIDER_DRAG_DISTANCE};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::{DocumentMessageHandler, NodeGraphMessage, Responses};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shapes::shape_utility::{extract_heart_parameters, heart_outline};
use crate::messages::tool::common_functionality::snap_session::SnapView;
use glam::{DAffine2, DVec2};
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::ParameterRef;
use graphene_std::vector::algorithms::shapes::{HEART_SIZE, HeartProportions, heart_bezpath};
use graphene_std::vector::generator_nodes::heart;
use graphene_std::vector::misc::point_to_dvec2;
use kurbo::{CubicBez, ParamCurve, ParamCurveExtrema, PathSeg};
use std::collections::VecDeque;

/// A parameter of the Heart node, which handles on the canvas change.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum HeartParameter {
	#[default]
	CleftDepth,
	CleftSharpness,
	LobeLean,
	WidestPoint,
	SideFullness,
	TipAngle,
}

impl HeartParameter {
	const ALL: [Self; 6] = [Self::CleftDepth, Self::CleftSharpness, Self::LobeLean, Self::WidestPoint, Self::SideFullness, Self::TipAngle];

	fn input(self) -> ParameterRef {
		match self {
			Self::CleftDepth => heart::CleftDepthInput.into(),
			Self::CleftSharpness => heart::CleftSharpnessInput.into(),
			Self::LobeLean => heart::LobeLeanInput.into(),
			Self::WidestPoint => heart::WidestPointInput.into(),
			Self::SideFullness => heart::SideFullnessInput.into(),
			Self::TipAngle => heart::TipAngleInput.into(),
		}
	}

	/// The node input's value for a proportion, which is a percentage, or degrees for the tip angle.
	fn input_value(self, proportion: f64) -> f64 {
		if self == Self::TipAngle { proportion.to_degrees() } else { proportion * 100. }
	}

	fn proportion_from_input_value(self, value: f64) -> f64 {
		if self == Self::TipAngle { value.to_radians() } else { value / 100. }
	}

	/// The range of the proportion, from the hard bounds the node clamps its input to.
	fn domain(self) -> Option<(f64, f64)> {
		let input = self.input();
		let metadata = graphene_std::registry::NODE_METADATA.lock().ok()?;
		let field = metadata.get(&input.node_identifier)?.fields.get(input.input_index)?;

		Some((self.proportion_from_input_value(field.number_hard_min?), self.proportion_from_input_value(field.number_hard_max?)))
	}

	fn with_proportion(self, proportions: HeartProportions, value: f64) -> HeartProportions {
		let mut proportions = proportions;
		match self {
			Self::CleftDepth => proportions.cleft_depth = value,
			Self::CleftSharpness => proportions.cleft_sharpness = value,
			Self::LobeLean => proportions.lobe_lean = value,
			Self::WidestPoint => proportions.widest_point = value,
			Self::SideFullness => proportions.side_fullness = value,
			Self::TipAngle => proportions.tip_angle = value,
		}
		proportions
	}

	fn cursor(self) -> MouseCursorIcon {
		match self {
			Self::CleftDepth | Self::WidestPoint => MouseCursorIcon::NSResize,
			Self::LobeLean => MouseCursorIcon::EWResize,
			Self::CleftSharpness | Self::SideFullness | Self::TipAngle => MouseCursorIcon::Default,
		}
	}
}

/// The right half of a heart in its layer's space: the lobe from the cleft to the widest point, and the side from there to the tip.
struct RightHalf {
	lobe: CubicBez,
	side: CubicBez,
}

impl RightHalf {
	fn new(proportions: &HeartProportions) -> Option<Self> {
		let bezpath = heart_bezpath(proportions);
		let mut segments = bezpath.segments();
		let (Some(PathSeg::Cubic(lobe)), Some(PathSeg::Cubic(side))) = (segments.next(), segments.next()) else {
			return None;
		};

		Some(Self { lobe, side })
	}

	/// The point of the lobe at the top of the heart.
	fn lobe_top(&self) -> DVec2 {
		let candidates = self.lobe.extrema().into_iter().map(|t| self.lobe.eval(t)).chain([self.lobe.p0, self.lobe.p3]);
		point_to_dvec2(candidates.min_by(|a, b| a.y.total_cmp(&b.y)).unwrap_or(self.lobe.p0))
	}

	/// The angle of the cleft's tangent above the horizontal, heading out toward the widest point.
	fn cleft_elevation(&self) -> f64 {
		let tangent = self.lobe.p1 - self.lobe.p0;
		(-tangent.y).atan2(tangent.x)
	}
}

/// One handle, on the right half or mirrored onto the left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct HeartHandle {
	parameter: HeartParameter,
	mirrored: bool,
}

impl HeartHandle {
	/// Every handle: the cleft's depth on the middle, and a pair mirrored across it for each other parameter.
	fn all() -> impl Iterator<Item = Self> {
		HeartParameter::ALL.into_iter().flat_map(|parameter| {
			let sides: &[bool] = if parameter == HeartParameter::CleftDepth { &[false] } else { &[false, true] };
			sides.iter().map(move |&mirrored| Self { parameter, mirrored })
		})
	}

	/// Flips a point between the right half and this handle's half, since mirroring undoes itself.
	fn onto_side(self, point: DVec2) -> DVec2 {
		if self.mirrored { DVec2::new(-point.x, point.y) } else { point }
	}

	/// Where the handle sits in the viewport, if its tangent has a direction there.
	fn position(self, half: &RightHalf, to_viewport: DAffine2) -> Option<DVec2> {
		let in_viewport = |point: kurbo::Point| to_viewport.transform_point2(self.onto_side(point_to_dvec2(point)));

		// The handles for angles sit a set distance out along their tangents
		let along_tangent = |anchor: kurbo::Point, toward: kurbo::Point| {
			let (anchor, toward) = (in_viewport(anchor), in_viewport(toward));
			Some(anchor + (toward - anchor).try_normalize()? * HEART_ANGLE_HANDLE_DISTANCE)
		};

		match self.parameter {
			HeartParameter::CleftDepth => Some(in_viewport(half.lobe.p0)),
			HeartParameter::CleftSharpness => along_tangent(half.lobe.p0, half.lobe.p1),
			HeartParameter::LobeLean => Some(to_viewport.transform_point2(self.onto_side(half.lobe_top()))),
			HeartParameter::WidestPoint => Some(in_viewport(half.lobe.p3)),
			HeartParameter::SideFullness => Some(in_viewport(half.side.eval(0.5))),
			HeartParameter::TipAngle => along_tangent(half.side.p3, half.side.p2),
		}
	}
}

/// Whether a heart is drawn too small in the viewport for its handles to stay apart, which hides them.
fn too_small_on_screen(to_viewport: DAffine2) -> bool {
	// With eleven handles, a heart needs more room than the threshold the other shapes' radii are held to
	[DVec2::X, DVec2::Y]
		.into_iter()
		.any(|axis| to_viewport.transform_vector2(axis * HEART_SIZE).length() < 3. * GIZMO_HIDE_THRESHOLD)
}

/// Finds where an increasing function reaches a target within [`low`, `high`] by bisection, settling on the nearer end if it lies outside.
fn solve_increasing(increasing: impl Fn(f64) -> f64, target: f64, low: f64, high: f64) -> f64 {
	let (mut low, mut high) = (low, high);

	// O(30) evaluations, narrowing any of the heart's ranges far finer than a drag can show
	for _ in 0..30 {
		let middle = (low + high) / 2.;
		if increasing(middle) < target {
			low = middle;
		} else {
			high = middle;
		}
	}

	(low + high) / 2.
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum HeartHandlesState {
	#[default]
	Inactive,
	Hover,
	Dragging,
}

/// The handles that shape a heart, one or a mirrored pair for each parameter of its node.
#[derive(Clone, Debug, Default)]
pub struct HeartHandles {
	pub layer: Option<LayerNodeIdentifier>,
	handle: HeartHandle,
	initial_proportions: Option<HeartProportions>,
	handle_state: HeartHandlesState,
}

impl HeartHandles {
	pub fn cleanup(&mut self) {
		self.handle_state = HeartHandlesState::Inactive;
		self.initial_proportions = None;
		self.layer = None;
	}

	pub fn update_state(&mut self, state: HeartHandlesState) {
		self.handle_state = state;
	}

	pub fn hovered(&self) -> bool {
		self.handle_state == HeartHandlesState::Hover
	}

	pub fn is_dragging(&self) -> bool {
		self.handle_state == HeartHandlesState::Dragging
	}

	pub fn cursor(&self) -> MouseCursorIcon {
		self.handle.parameter.cursor()
	}

	pub fn handle_actions(&mut self, layer: LayerNodeIdentifier, mouse_position: DVec2, document: &DocumentMessageHandler) {
		if self.handle_state != HeartHandlesState::Inactive {
			return;
		}

		let Some(proportions) = extract_heart_parameters(layer, document) else { return };
		let Some(half) = RightHalf::new(&proportions) else { return };
		let to_viewport = document.metadata().transform_to_viewport(layer);
		if too_small_on_screen(to_viewport) {
			return;
		}

		let nearest = HeartHandle::all()
			.filter_map(|handle| Some((handle, handle.position(&half, to_viewport)?.distance(mouse_position))))
			.filter(|&(_, distance)| distance < HEART_HANDLE_HOVER_RADIUS)
			.min_by(|a, b| a.1.total_cmp(&b.1));

		if let Some((handle, _)) = nearest {
			self.layer = Some(layer);
			self.handle = handle;
			self.initial_proportions = Some(proportions);
			self.update_state(HeartHandlesState::Hover);
		}
	}

	pub fn overlays(&self, document: &DocumentMessageHandler, layer: Option<LayerNodeIdentifier>, overlay_context: &mut OverlayContext) {
		let Some(layer) = layer.or(self.layer) else { return };
		let Some(proportions) = extract_heart_parameters(layer, document) else { return };
		let Some(half) = RightHalf::new(&proportions) else { return };
		let to_viewport = document.metadata().transform_to_viewport(layer);

		match self.handle_state {
			HeartHandlesState::Inactive => {
				if too_small_on_screen(to_viewport) {
					return;
				}

				for position in HeartHandle::all().filter_map(|handle| handle.position(&half, to_viewport)) {
					overlay_context.manipulator_handle(position, false, None);
				}
			}
			HeartHandlesState::Hover | HeartHandlesState::Dragging => {
				heart_outline(layer, document, overlay_context);
				self.guide_overlays(&proportions, &half, to_viewport, overlay_context);

				if let Some(position) = self.handle.position(&half, to_viewport) {
					overlay_context.manipulator_handle(position, true, None);
				}
			}
		}
	}

	/// Draws what the hovered or dragged handle is measured against.
	fn guide_overlays(&self, proportions: &HeartProportions, half: &RightHalf, to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
		let in_viewport = |point: DVec2| to_viewport.transform_point2(self.handle.onto_side(point));
		let dashed_line = |overlay_context: &mut OverlayContext, start: DVec2, end: DVec2| {
			overlay_context.dashed_line(in_viewport(start), in_viewport(end), None, None, Some(4.), Some(4.), Some(0.5));
		};

		let half_size = HEART_SIZE / 2.;
		let widest_y = half_size - HEART_SIZE * proportions.widest_point;

		match self.handle.parameter {
			// From the lobes' tops at 0% to the widest point's height at 100%
			HeartParameter::CleftDepth => dashed_line(overlay_context, DVec2::new(0., -half_size), DVec2::new(0., widest_y)),
			// Along the top of the lobe, which leans in or out
			HeartParameter::LobeLean => dashed_line(overlay_context, DVec2::new(0., -half_size), DVec2::new(half_size, -half_size)),
			// Across the heart at its widest
			HeartParameter::WidestPoint => dashed_line(overlay_context, DVec2::new(-half_size, widest_y), DVec2::new(half_size, widest_y)),
			// The straight side the fullness bulges out from
			HeartParameter::SideFullness => dashed_line(overlay_context, point_to_dvec2(half.side.p0), point_to_dvec2(half.side.p3)),
			// The tangent out of the cleft
			HeartParameter::CleftSharpness => {
				let cleft = in_viewport(point_to_dvec2(half.lobe.p0));
				if let Some(handle) = self.handle.position(half, to_viewport) {
					overlay_context.line(cleft, handle, None, None);
				}
			}
			// Both tangents out of the tip, which open to its angle
			HeartParameter::TipAngle => {
				let tip = in_viewport(point_to_dvec2(half.side.p3));
				for mirrored in [false, true] {
					if let Some(handle) = (HeartHandle { mirrored, ..self.handle }).position(half, to_viewport) {
						overlay_context.line(tip, handle, None, None);
					}
				}
			}
		}
	}

	pub fn update(&mut self, view: SnapView, drag_start: DVec2, responses: &mut VecDeque<Message>) {
		let SnapView { document, input, .. } = view;
		let (Some(layer), Some(initial)) = (self.layer, self.initial_proportions) else { return };
		let Some(node_id) = graph_modification_utils::get_heart_id(layer, &document.network_interface) else {
			return;
		};
		let Some(initial_half) = RightHalf::new(&initial) else { return };
		let parameter = self.handle.parameter;
		let Some((min, max)) = parameter.domain() else {
			log::error!("The Heart node's {parameter:?} parameter should have hard bounds");
			return;
		};

		let to_viewport = document.metadata().transform_to_viewport(layer);
		if to_viewport.matrix2.determinant() == 0. {
			return;
		}

		// The pointer and where the drag began, in the right half's layer space whichever half the handle is on
		let pointer = input.mouse.position;
		let in_right_half = |viewport_point: DVec2| self.handle.onto_side(to_viewport.inverse().transform_point2(viewport_point));
		let (pointer_in_layer, start_in_layer) = (in_right_half(pointer), in_right_half(drag_start));
		let dragged_in_layer = pointer_in_layer - start_in_layer;

		// How far the pointer has gone in viewport pixels, along a direction in the right half's layer space
		let dragged_along = |direction: DVec2| (pointer - drag_start).dot(to_viewport.transform_vector2(self.handle.onto_side(direction)).normalize_or_zero());

		let half_size = HEART_SIZE / 2.;
		let value = match parameter {
			// The cleft follows the pointer up and down, between the lobes' tops at 0% and the widest point's height at 100%
			HeartParameter::CleftDepth => {
				let widest_y = half_size - HEART_SIZE * initial.widest_point;
				(initial_half.lobe.p0.y + dragged_in_layer.y + half_size) / (widest_y + half_size)
			}
			// The widest point follows the pointer up and down
			HeartParameter::WidestPoint => initial.widest_point - dragged_in_layer.y / HEART_SIZE,
			// The tip's tangents turn with the pointer around the tip
			HeartParameter::TipAngle => {
				let tip = point_to_dvec2(initial_half.side.p3);
				let half_angle = |point: DVec2| (point - tip).x.atan2(-(point - tip).y);
				initial.tip_angle + 2. * (half_angle(pointer_in_layer) - half_angle(start_in_layer))
			}
			// The cleft's tangent turns with the pointer around the cleft, which takes finding the sharpness that points it there
			HeartParameter::CleftSharpness => {
				let cleft = point_to_dvec2(initial_half.lobe.p0);
				let elevation = |point: DVec2| (-(point - cleft).y).atan2((point - cleft).x);
				let target = initial_half.cleft_elevation() + elevation(pointer_in_layer) - elevation(start_in_layer);

				let elevation_with = |sharpness: f64| RightHalf::new(&parameter.with_proportion(initial, sharpness)).map_or(f64::NAN, |half| half.cleft_elevation());
				solve_increasing(elevation_with, target, min, max)
			}
			// Dragging outward leans each lobe toward its side, at a set rate since no point on the lobe moves in step with its shape
			HeartParameter::LobeLean => initial.lobe_lean + dragged_along(DVec2::X) / HEART_SLIDER_DRAG_DISTANCE * (max - min),
			// Dragging out from the straight side fills it out, at a set rate since the side moves too little to follow the pointer
			HeartParameter::SideFullness => {
				let chord = initial_half.side.p3 - initial_half.side.p0;
				initial.side_fullness + dragged_along(DVec2::new(chord.y, -chord.x)) / HEART_SLIDER_DRAG_DISTANCE * (max - min)
			}
		};
		if !value.is_finite() {
			return;
		}

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, parameter.input()),
			input: NodeInput::value(TaggedValue::Number(parameter.input_value(value.clamp(min, max))), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}
}
