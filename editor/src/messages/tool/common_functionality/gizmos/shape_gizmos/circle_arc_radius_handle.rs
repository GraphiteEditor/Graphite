use crate::consts::GIZMO_HIDE_THRESHOLD;
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::{DocumentMessageHandler, NodeGraphMessage};
use crate::messages::prelude::{FrontendMessage, Responses};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, get_arc_id, get_stroke_width};
use crate::messages::tool::common_functionality::shapes::shape_utility::{extract_arc_parameters, extract_circle_radius, extract_polygon_parameters, extract_star_parameters};
use crate::messages::tool::common_functionality::snap_session::{Kind, SnapSession, SnapView, ValueCandidate};
use glam::{DAffine2, DVec2};
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::ParameterRef;
use graphene_std::core_types::FallibleVec2Operations;
use std::collections::VecDeque;
use std::f64::consts::FRAC_PI_2;

/// How many of the round shapes nearest the dragged one offer their radius to snap to.
const MAX_EQUAL_RADII: usize = 32;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum RadiusHandleState {
	#[default]
	Inactive,
	Hover,
	Dragging,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RadiusHandle {
	pub layer: Option<LayerNodeIdentifier>,
	initial_radius: f64,
	/// The radius the pointer asks for, before any snap.
	free_radius: f64,
	/// The radii of the other round shapes, gathered once a drag begins.
	equal_radii: Option<Vec<ValueCandidate>>,
	handle_state: RadiusHandleState,
	angle: f64,
	previous_mouse_position: DVec2,
}

impl RadiusHandle {
	pub fn cleanup(&mut self) {
		self.handle_state = RadiusHandleState::Inactive;
		self.layer = None;
		self.equal_radii = None;
	}

	pub fn hovered(&self) -> bool {
		self.handle_state == RadiusHandleState::Hover
	}

	pub fn is_dragging(&self) -> bool {
		self.handle_state == RadiusHandleState::Dragging
	}

	pub fn update_state(&mut self, state: RadiusHandleState) {
		self.handle_state = state;
	}

	pub fn check_if_inside_dash_lines(angle: f64, mouse_position: DVec2, viewport: DAffine2, radius: f64, document: &DocumentMessageHandler, layer: LayerNodeIdentifier) -> bool {
		let center = viewport.transform_point2(DVec2::ZERO);
		if let Some(stroke_width) = get_stroke_width(layer, &document.network_interface) {
			let circle_point = calculate_circle_point_position(angle, radius.abs());
			let Some(direction) = circle_point.try_normalize() else { return false };
			let mouse_distance = mouse_position.distance(center);

			let spacing = Self::calculate_extra_spacing(viewport, radius, center, stroke_width, 15.);

			let inner_point = viewport.transform_point2(circle_point - direction * spacing).distance(center);
			let outer_point = viewport.transform_point2(circle_point + direction * spacing).distance(center);

			mouse_distance >= inner_point && mouse_distance <= outer_point
		} else {
			let point_position = viewport.transform_point2(calculate_circle_point_position(angle, radius.abs()));
			mouse_position.distance(center) <= point_position.distance(center)
		}
	}

	fn calculate_extra_spacing(viewport: DAffine2, radius: f64, viewport_center: DVec2, stroke_width: f64, threshold: f64) -> f64 {
		let start_point = viewport.transform_point2(calculate_circle_point_position(0., radius)).distance(viewport_center);
		let end_point = viewport.transform_point2(calculate_circle_point_position(FRAC_PI_2, radius)).distance(viewport_center);
		let min_radius = start_point.min(end_point);
		let extra_spacing = if min_radius < threshold { 10. * (min_radius / threshold) } else { 10. };

		stroke_width + extra_spacing
	}

	pub fn handle_actions(&mut self, layer: LayerNodeIdentifier, document: &DocumentMessageHandler, mouse_position: DVec2, responses: &mut VecDeque<Message>) {
		match &self.handle_state {
			RadiusHandleState::Inactive => {
				let Some(radius) = extract_circle_radius(layer, document).or(extract_arc_parameters(Some(layer), document).map(|(r, _, _, _)| r)) else {
					return;
				};
				let viewport = document.metadata().transform_to_viewport(layer);
				let vector = viewport.inverse().transform_point2(mouse_position);
				let Some(angle) = vector.try_angle_to(DVec2::X) else { return };
				let point_position = viewport.transform_point2(calculate_circle_point_position(angle, radius.abs()));
				let center = viewport.transform_point2(DVec2::ZERO);

				if point_position.distance(center) < GIZMO_HIDE_THRESHOLD {
					return;
				}

				if Self::check_if_inside_dash_lines(angle, mouse_position, viewport, radius.abs(), document, layer) {
					self.layer = Some(layer);
					self.initial_radius = radius;
					self.free_radius = radius;
					self.previous_mouse_position = mouse_position;
					self.angle = angle;

					self.update_state(RadiusHandleState::Hover);

					responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::EWResize });
				}
			}
			RadiusHandleState::Dragging | RadiusHandleState::Hover => {}
		}
	}

	pub fn overlays(&self, document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
		match &self.handle_state {
			RadiusHandleState::Inactive => {}
			RadiusHandleState::Dragging | RadiusHandleState::Hover => {
				let Some(layer) = self.layer else { return };
				let Some(radius) = extract_circle_radius(layer, document).or(extract_arc_parameters(Some(layer), document).map(|(r, _, _, _)| r)) else {
					return;
				};
				let viewport = document.metadata().transform_to_viewport(layer);
				let center = viewport.transform_point2(DVec2::ZERO);

				let x_point = viewport.transform_point2(calculate_circle_point_position(0., radius));
				let y_point = viewport.transform_point2(calculate_circle_point_position(FRAC_PI_2, radius));

				let direction_x = viewport.transform_vector2(DVec2::X);
				let direction_y = viewport.transform_vector2(-DVec2::Y);

				if let Some(stroke_width) = get_stroke_width(layer, &document.network_interface) {
					let spacing = Self::calculate_extra_spacing(viewport, radius, center, stroke_width, 15.);
					let smaller_radius_x = (x_point - direction_x * spacing).distance(center);
					let smaller_radius_y = (y_point - direction_y * spacing).distance(center);

					let larger_radius_x = (x_point + direction_x * spacing).distance(center);
					let larger_radius_y = (y_point + direction_y * spacing).distance(center);

					overlay_context.dashed_ellipse(center, smaller_radius_x, smaller_radius_y, None, None, None, None, None, None, Some(4.), Some(4.), Some(0.5));
					overlay_context.dashed_ellipse(center, larger_radius_x, larger_radius_y, None, None, None, None, None, None, Some(4.), Some(4.), Some(0.5));

					return;
				}

				let radius_x = x_point.distance(center);
				let radius_y = y_point.distance(center);
				overlay_context.dashed_ellipse(center, radius_x, radius_y, None, None, None, None, None, None, Some(4.), Some(4.), Some(0.5));
			}
		}
	}

	pub fn update_inner_radius(&mut self, view: SnapView, snap_session: &mut SnapSession, responses: &mut VecDeque<Message>, drag_start: DVec2) {
		let SnapView { document, input, .. } = view;
		let Some(layer) = self.layer else { return };

		// This gizmo serves both Circle and Arc layers, so resolve which node is present to know which radius parameter to write
		let (node_id, radius_parameter) = if let Some(node_id) = graph_modification_utils::get_circle_id(layer, &document.network_interface) {
			(node_id, ParameterRef::from(graphene_std::vector::generator_nodes::circle::RadiusInput))
		} else if let Some(node_id) = get_arc_id(layer, &document.network_interface) {
			(node_id, ParameterRef::from(graphene_std::vector::generator_nodes::arc::RadiusInput))
		} else {
			return;
		};

		let viewport_transform = document.network_interface.document_metadata().transform_to_viewport(layer);
		let center = viewport_transform.transform_point2(DVec2::ZERO);

		let delta_vector = viewport_transform.inverse().transform_point2(input.mouse.position) - viewport_transform.inverse().transform_point2(self.previous_mouse_position);
		let radius = drag_start - center;
		let sign = radius.dot(delta_vector).signum();

		let net_delta = delta_vector.length() * sign * self.initial_radius.signum();
		self.previous_mouse_position = input.mouse.position;

		// Kept apart from the radius written, which a snap may have moved
		self.free_radius += net_delta;
		let equal_radii = self.equal_radii.get_or_insert_with(|| equal_radius_candidates(document, layer));

		// The handle is where the pointer took hold of the rim, which moves out along that angle as the radius grows
		let spoke = calculate_circle_point_position(self.angle, 1.);
		let on_screen_per_unit = viewport_transform.transform_vector2(spoke).length();
		let to_document = document.metadata().document_to_viewport.inverse() * viewport_transform;
		let position_of = |radius: f64| to_document.transform_point2(spoke * radius);

		// The size is snapped, which keeps whichever sign the radius has come to
		let snapped_radius = snap_session.snap_value(view, position_of, self.free_radius.abs(), on_screen_per_unit, equal_radii) * self.free_radius.signum();

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, radius_parameter),
			input: NodeInput::value(TaggedValue::Number(snapped_radius), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}
}

/// The sizes of the radii of the visible circles, arcs, polygons, and stars nearest a dragged one, in its units.
/// O(n) in the document's layers.
fn equal_radius_candidates(document: &DocumentMessageHandler, dragged: LayerNodeIdentifier) -> Vec<ValueCandidate> {
	let metadata = document.metadata();

	// A radius is compared in document space, as the layer's transform scales it
	let scale = |layer: LayerNodeIdentifier| metadata.transform_to_document(layer).matrix2.determinant().abs().sqrt();
	let dragged_scale = scale(dragged);
	if !dragged_scale.is_normal() {
		return Vec::new();
	}
	let dragged_center = metadata.transform_to_document(dragged).transform_point2(DVec2::ZERO);

	// A star offers its outer radius
	let radius_of = |layer: LayerNodeIdentifier| {
		extract_circle_radius(layer, document)
			.or_else(|| extract_arc_parameters(Some(layer), document).map(|(radius, ..)| radius))
			.or_else(|| extract_polygon_parameters(Some(layer), document).map(|(_, radius)| radius))
			.or_else(|| extract_star_parameters(Some(layer), document).map(|(_, radius1, radius2)| radius1.abs().max(radius2.abs())))
	};

	let mut nearest: Vec<(f64, ValueCandidate)> = metadata
		.all_layers()
		.filter(|&layer| layer != dragged && document.network_interface.is_layer_visible(layer))
		.filter_map(|layer| {
			let value = radius_of(layer)?.abs() * scale(layer) / dragged_scale;
			let distance = metadata.transform_to_document(layer).transform_point2(DVec2::ZERO).distance(dragged_center);
			let candidate = ValueCandidate {
				value,
				kind: Kind::EqualRadius,
				identity: layer.to_node().0,
			};
			(value.is_finite() && value != 0.).then_some((distance, candidate))
		})
		.collect();

	if nearest.len() > MAX_EQUAL_RADII {
		nearest.select_nth_unstable_by(MAX_EQUAL_RADII, |a, b| a.0.total_cmp(&b.0));
		nearest.truncate(MAX_EQUAL_RADII);
	}
	nearest.into_iter().map(|(_, candidate)| candidate).collect()
}

fn calculate_circle_point_position(theta: f64, radius: f64) -> DVec2 {
	DVec2::new(radius * theta.cos(), -radius * theta.sin())
}
