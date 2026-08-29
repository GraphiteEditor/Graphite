//! Which node parameters get a canvas gizmo, as a table. A node declares its gizmo-enabled inputs as a
//! `const` slice of [`GizmoInfo`] and registers itself in [`registered_gizmo_nodes`]; the
//! [generic gizmos](super::generic_gizmos) build the handles from that.
//!
//! `README.md`, next to this file, is the guide to adding one.

use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::DocumentMessageHandler;
use crate::messages::tool::common_functionality::gizmos::gizmo_behaviors;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use glam::{DAffine2, DVec2};
use graph_craft::ProtoNodeIdentifier;
use graph_craft::document::value::TaggedValue;
use graphene_std::vector::generator_nodes;
use graphene_std::vector::generator_nodes::{arc, circle, grid, heart, regular_polygon, spiral, star};
use graphene_std::{NodeParameter, ParameterRef};

/// The kind of control a gizmo presents, which also fixes the [`TaggedValue`] type of the parameter it edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GizmoType {
	/// A handle dragged along a ray, editing an `f64` length.
	Slider,
	/// A dial stepped by horizontal drag, editing a `u32` count.
	Dial,
	/// A draggable point editing a `DVec2`. Not implemented yet.
	Position,
	/// An angle in `f64` degrees. Runs on the [`Slider`](Self::Slider) machinery, but an angle is never a
	/// distance along a ray, so a declaration using it is expected to carry its own [`GizmoBehavior::drag`].
	Angle,
}

/// Where a gizmo's handle is anchored relative to its layer. The registry declares the intent and the
/// generic gizmos do the math.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionHint {
	/// Anchor at the center of the layer's bounding box.
	BoundingBoxCenter,
	/// Anchor on the right/middle edge of the layer's bounding box.
	BoundingBoxEdge,
	/// Anchor at the top-right corner of the layer's bounding box.
	BoundingBoxCorner,
	/// Derive the anchor from the parameter's own value (e.g. a radius handle sits at distance
	/// `value` from the layer origin). The most precise option for length-like parameters.
	ParameterDerived,
}

/// How the user is currently engaging a gizmo. Hooks receive it so a shape can draw one thing at rest and
/// another mid-drag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GizmoState {
	/// Not hovered: the gizmo is idle, but a shape may still want a subtle hint on screen.
	Inactive,
	/// The cursor is on the handle.
	Hover,
	/// A drag is in progress.
	Dragging,
}

/// What a shape-specific hook is given to work with.
pub struct GizmoContext<'a> {
	pub layer: LayerNodeIdentifier,
	pub document: &'a DocumentMessageHandler,
	pub parameter: ParameterRef,
	pub state: GizmoState,
	/// Where the cursor is, for hints that track the pointer rather than the handle.
	pub mouse_position: DVec2,
	/// The path editor's state, available while overlays are being drawn. A shape uses it to stand down
	/// when the cursor is over an editable segment, so a resting hint never competes with path editing.
	pub shape_editor: Option<&'a ShapeState>,
	/// Which of the parameter's grab points is in play, indexing the list from `handle_positions`.
	pub handle_index: usize,
}

/// What a drag has done so far, handed to a shape's own [`GizmoBehavior::drag`].
pub struct DragInput {
	/// Where the drag began, in viewport space.
	pub drag_start: DVec2,
	/// Where the cursor is now, in viewport space.
	pub mouse_position: DVec2,
	/// The dragged parameter's value when the drag began.
	pub initial_value: f64,
	/// Every input of the node as it stood when the drag began, indexed the way its parameter symbols are. A
	/// drag that rewrites several parameters needs this: by the second frame the live values are its own.
	pub initial_parameters: Vec<Option<TaggedValue>>,
	/// Angle swept around the layer's origin since the drag began, in degrees. Accumulated frame by frame so
	/// it keeps counting past a full turn.
	pub total_angle: f64,
	/// This frame's rotation about the layer's origin, in degrees.
	pub angle_delta: f64,
	/// Which grab point the drag is running from.
	pub handle_index: usize,
}

/// What a drag wants done, once it has worked out what the cursor meant.
#[derive(Default)]
pub struct DragWrites {
	/// Node inputs to set.
	pub inputs: Vec<(ParameterRef, TaggedValue)>,
	/// A transform to apply to the layer, for a control that moves the shape as it resizes it. A grid grown
	/// from its top edge has to move up as it gains a row, or the edge slides out from under the cursor.
	pub transform: Option<DAffine2>,
}

impl DragWrites {
	/// The common case: a drag that only writes node inputs.
	pub fn inputs(inputs: Vec<(ParameterRef, TaggedValue)>) -> Self {
		Self { inputs, transform: None }
	}

	pub fn is_empty(&self) -> bool {
		self.inputs.is_empty() && self.transform.is_none()
	}
}

/// Values a drag should settle onto, computed from the layer's other parameters.
pub type SnapTargetsFn = fn(&GizmoContext) -> Vec<f64>;
/// Extra overlay a shape draws around its gizmo.
pub type OverlayFn = fn(&GizmoContext, &mut OverlayContext);
/// Inputs to write alongside the dragged one.
pub type CoupledWritesFn = fn(&GizmoContext, f64) -> Vec<(ParameterRef, TaggedValue)>;
/// Where a parameter can be grabbed, in the layer's local space.
pub type HandlePositionsFn = fn(&GizmoContext, f64) -> Vec<DVec2>;
/// How far the cursor is from each grab point.
pub type HoverDistancesFn = fn(&GizmoContext) -> Vec<Option<f64>>;
/// How cursor motion becomes node inputs.
pub type DragFn = fn(&GizmoContext, &mut DragInput) -> DragWrites;

/// The escape hatch for a node whose gizmo needs more than the generic mechanics.
///
/// The generic layer always owns hit-testing, the hover/drag state machine, the handle overlay and the
/// input write. What is left depends on the node's geometry and cannot be expressed as data: a star's snap
/// radii fall out of its side count and its other radius. Those arrive here as functions from
/// [`gizmo_behaviors`], so the registry stays a table. Every field is optional.
#[derive(Clone, Copy, Debug)]
pub struct GizmoBehavior {
	/// Values the drag should snap to, recomputed from the layer's current parameters at drag start.
	pub snap_targets: Option<SnapTargetsFn>,
	/// Extra overlay for a node that shows more than the bare handle, such as the star's outline and spokes.
	/// Called in every state, so it draws the resting affordance too.
	pub overlay: Option<OverlayFn>,
	/// Inputs to write alongside the dragged one, given the value the drag produced.
	pub coupled_writes: Option<CoupledWritesFn>,
	/// Where this parameter can be grabbed, in the layer's local space, given its current value. The default
	/// is a single handle `value` out along the local +X axis.
	///
	/// A shape may offer the same parameter in several places at once: a star's outer radius can be taken hold
	/// of at any of its outer points. The drag then runs along the ray through the one the user grabbed.
	pub handle_positions: Option<HandlePositionsFn>,
	/// How far the cursor is from each grab point, when they are not points. The default measures to the
	/// handle positions above; a grid's rows are grabbed anywhere along an edge, so it measures to a line
	/// instead. `None` in a slot means that grab point is unavailable right now.
	pub hover_distances: Option<HoverDistancesFn>,
	/// How cursor motion becomes node inputs. The default reads the cursor's distance along the ray through
	/// the grabbed handle, which is what a length wants.
	///
	/// A control that winds or sweeps rather than extends supplies its own, and returns every input the
	/// motion implies: a spiral's turns cannot change without its outer radius following, or it tightens as
	/// it grows. Supplying one also bypasses clamping and snapping, since only a drag that writes several
	/// parameters knows how they constrain each other.
	///
	/// [`DragInput`] is mutable so a gesture can re-anchor: an arc dragged past a full sweep hands over to
	/// its other endpoint and rewrites the baseline the rest of the drag is measured against.
	pub drag: Option<DragFn>,
	/// Per-frame rotation, in degrees, below which swept angle counts as cursor noise. Near the layer's
	/// origin the angle between successive positions is mostly noise, and feeding it in makes the value
	/// jitter while the cursor is still. Zero accumulates everything.
	pub angle_deadzone: f64,
	/// Set when `overlay` already draws whatever the user grabs, suppressing the generic handle dot and its
	/// line from the origin. A grid marks its edge with a dashed line and a circle draws a band around its
	/// circumference; neither has a handle sitting at a point.
	pub draws_own_handle: bool,
	/// Set when this gizmo is grabbed anywhere along a region rather than at a point: a circle's whole
	/// circumference, a grid's whole edge. Decides priority against an overlapping point handle, which wins
	/// outright. See `rank_candidates` in [`generic_gizmos`](super::generic_gizmos).
	pub extended_target: bool,
}

impl GizmoBehavior {
	/// No hooks at all. A declaration sets the few fields it needs and takes the rest from here.
	pub const NONE: Self = Self {
		snap_targets: None,
		overlay: None,
		coupled_writes: None,
		handle_positions: None,
		hover_distances: None,
		drag: None,
		angle_deadzone: 0.,
		draws_own_handle: false,
		extended_target: false,
	};
}

/// One gizmo-enabled parameter of a node: which input it edits, how it is presented, and the bounds that
/// apply. `min` and `max` are inclusive.
#[derive(Clone, Copy, Debug)]
pub struct GizmoInfo {
	pub parameter_index: usize,
	pub gizmo_type: GizmoType,
	/// Shown in overlays and tooltips.
	pub name: &'static str,
	pub min: Option<f64>,
	pub max: Option<f64>,
	pub position_hint: PositionHint,
	pub behavior: GizmoBehavior,
}

const CIRCLE_GIZMOS: &[GizmoInfo] = &[GizmoInfo {
	parameter_index: circle::RadiusInput::INDEX,
	gizmo_type: GizmoType::Slider,
	name: "Radius",
	min: Some(0.),
	max: None,
	behavior: gizmo_behaviors::CIRCULAR_RADIUS,
	position_hint: PositionHint::ParameterDerived,
}];

// The radius is grabbable at every corner rather than at a single `(radius, 0)` handle, which would land
// off the polygon's geometry. See `polygon_radius_handles`.
const POLYGON_GIZMOS: &[GizmoInfo] = &[
	GizmoInfo {
		parameter_index: regular_polygon::SidesInput::INDEX,
		gizmo_type: GizmoType::Dial,
		name: "Sides",
		min: Some(3.),
		max: None,
		behavior: gizmo_behaviors::POLYGON_SIDES,
		position_hint: PositionHint::BoundingBoxCenter,
	},
	GizmoInfo {
		parameter_index: regular_polygon::RadiusInput::INDEX,
		gizmo_type: GizmoType::Slider,
		name: "Radius",
		min: Some(0.),
		max: None,
		behavior: gizmo_behaviors::POLYGON_RADIUS,
		position_hint: PositionHint::ParameterDerived,
	},
];

const STAR_GIZMOS: &[GizmoInfo] = &[
	GizmoInfo {
		parameter_index: star::SidesInput::INDEX,
		gizmo_type: GizmoType::Dial,
		name: "Points",
		min: Some(3.),
		max: None,
		behavior: gizmo_behaviors::STAR_SIDES,
		position_hint: PositionHint::BoundingBoxCenter,
	},
	GizmoInfo {
		parameter_index: star::Radius1Input::INDEX,
		gizmo_type: GizmoType::Slider,
		name: "Outer Radius",
		min: Some(0.),
		max: None,
		behavior: gizmo_behaviors::STAR_RADIUS,
		position_hint: PositionHint::ParameterDerived,
	},
	GizmoInfo {
		parameter_index: star::Radius2Input::INDEX,
		gizmo_type: GizmoType::Slider,
		name: "Inner Radius",
		min: Some(0.),
		max: None,
		behavior: gizmo_behaviors::STAR_RADIUS,
		position_hint: PositionHint::ParameterDerived,
	},
];

const ARC_GIZMOS: &[GizmoInfo] = &[
	GizmoInfo {
		parameter_index: arc::RadiusInput::INDEX,
		gizmo_type: GizmoType::Slider,
		name: "Radius",
		min: Some(0.),
		max: None,
		behavior: gizmo_behaviors::CIRCULAR_RADIUS,
		position_hint: PositionHint::ParameterDerived,
	},
	// One entry, not two: either endpoint can move the start angle and the sweep together, so a separate
	// start-angle gizmo would be a second control over the same gesture.
	GizmoInfo {
		parameter_index: arc::SweepAngleInput::INDEX,
		gizmo_type: GizmoType::Angle,
		name: "Sweep",
		min: Some(0.),
		max: Some(360.),
		behavior: gizmo_behaviors::ARC_SWEEP,
		position_hint: PositionHint::ParameterDerived,
	},
];

// Only the turns control. A handle for either radius would sit at an arbitrary point on a curve that is
// nowhere near circular, whereas winding the spiral from its own endpoints reads immediately. Both radii
// stay in the Properties panel.
const SPIRAL_GIZMOS: &[GizmoInfo] = &[GizmoInfo {
	parameter_index: spiral::TurnsInput::INDEX,
	gizmo_type: GizmoType::Slider,
	name: "Turns",
	min: Some(0.),
	max: None,
	behavior: gizmo_behaviors::SPIRAL_TURNS,
	position_hint: PositionHint::ParameterDerived,
}];

// Three of the heart's eleven parameters: the ones with an obvious place to grab on the shape. The rest
// (curvature, tilt, sharpness) are shaping controls better set by number than by eye.
const HEART_GIZMOS: &[GizmoInfo] = &[
	GizmoInfo {
		parameter_index: heart::RadiusInput::INDEX,
		gizmo_type: GizmoType::Slider,
		name: "Radius",
		min: Some(0.),
		max: None,
		behavior: GizmoBehavior::NONE,
		position_hint: PositionHint::ParameterDerived,
	},
	GizmoInfo {
		parameter_index: heart::CleavageDepthInput::INDEX,
		gizmo_type: GizmoType::Slider,
		name: "Cleavage",
		min: Some(0.),
		max: Some(0.6),
		behavior: gizmo_behaviors::HEART_CLEAVAGE,
		position_hint: PositionHint::ParameterDerived,
	},
	GizmoInfo {
		parameter_index: heart::ShoulderWidthInput::INDEX,
		gizmo_type: GizmoType::Slider,
		name: "Shoulder Width",
		min: Some(0.),
		max: Some(1.4),
		behavior: gizmo_behaviors::HEART_SHOULDER,
		position_hint: PositionHint::ParameterDerived,
	},
];

// Rows and columns only. A grid's spacing is a two-axis value with no obvious handle on the shape, so it
// stays in the Properties panel.
const GRID_GIZMOS: &[GizmoInfo] = &[
	GizmoInfo {
		parameter_index: grid::ColumnsInput::INDEX,
		gizmo_type: GizmoType::Dial,
		name: "Columns",
		min: Some(1.),
		max: None,
		behavior: gizmo_behaviors::GRID_COLUMNS,
		position_hint: PositionHint::BoundingBoxCorner,
	},
	GizmoInfo {
		parameter_index: grid::RowsInput::INDEX,
		gizmo_type: GizmoType::Dial,
		name: "Rows",
		min: Some(1.),
		max: None,
		behavior: gizmo_behaviors::GRID_ROWS,
		position_hint: PositionHint::BoundingBoxCorner,
	},
];

/// Every node type with registered gizmos, paired with its declarations.
///
/// This is a function rather than a `const` because a [`ProtoNodeIdentifier`] is not usable as a `'static`
/// reference in one. Building the array is cheap: the identifiers are backed by `&'static str`.
pub fn registered_gizmo_nodes() -> [(ProtoNodeIdentifier, &'static [GizmoInfo]); 7] {
	[
		(generator_nodes::circle::IDENTIFIER, CIRCLE_GIZMOS),
		(generator_nodes::regular_polygon::IDENTIFIER, POLYGON_GIZMOS),
		(generator_nodes::star::IDENTIFIER, STAR_GIZMOS),
		(generator_nodes::arc::IDENTIFIER, ARC_GIZMOS),
		(generator_nodes::spiral::IDENTIFIER, SPIRAL_GIZMOS),
		(generator_nodes::grid::IDENTIFIER, GRID_GIZMOS),
		(generator_nodes::heart::IDENTIFIER, HEART_GIZMOS),
	]
}

/// The gizmo declarations for a node type, or an empty slice when it has none.
pub fn get_gizmo_info(identifier: &ProtoNodeIdentifier) -> &'static [GizmoInfo] {
	registered_gizmo_nodes()
		.into_iter()
		.find(|(registered, _)| registered.as_str() == identifier.as_str())
		.map(|(_, infos)| infos)
		.unwrap_or(&[])
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn circle_exposes_a_radius_slider() {
		let infos = get_gizmo_info(&generator_nodes::circle::IDENTIFIER);
		assert_eq!(infos.len(), 1);
		assert_eq!(infos[0].parameter_index, 1);
		assert_eq!(infos[0].gizmo_type, GizmoType::Slider);
		assert_eq!(infos[0].min, Some(0.));
		assert_eq!(infos[0].position_hint, PositionHint::ParameterDerived);
	}

	#[test]
	fn polygon_exposes_a_sides_dial_and_a_radius() {
		let infos = get_gizmo_info(&generator_nodes::regular_polygon::IDENTIFIER);
		assert_eq!(infos.len(), 2);

		let sides = &infos[0];
		assert_eq!(sides.gizmo_type, GizmoType::Dial);
		assert_eq!(sides.parameter_index, 1);
		assert_eq!(sides.min, Some(3.));

		// The radius is grabbed at the polygon's corners, so it has to place its own handles.
		let radius = &infos[1];
		assert_eq!(radius.gizmo_type, GizmoType::Slider);
		assert_eq!(radius.min, Some(0.));
		assert!(radius.behavior.handle_positions.is_some());
	}

	#[test]
	fn star_exposes_a_points_dial_and_two_radius_sliders() {
		let infos = get_gizmo_info(&generator_nodes::star::IDENTIFIER);
		assert_eq!(infos.iter().filter(|info| info.gizmo_type == GizmoType::Dial).count(), 1);
		assert_eq!(infos.iter().filter(|info| info.gizmo_type == GizmoType::Slider).count(), 2);
	}

	#[test]
	fn heart_exposes_radius_cleavage_and_shoulder() {
		let infos = get_gizmo_info(&generator_nodes::heart::IDENTIFIER);
		assert_eq!(infos.len(), 3);

		// A plain distance, so it needs no behavior of its own.
		let radius = &infos[0];
		assert_eq!(radius.gizmo_type, GizmoType::Slider);
		assert_eq!(radius.min, Some(0.));
		assert!(radius.behavior.handle_positions.is_none());

		// The other two are fractions of the radius, so each places its own handle and converts back on drag.
		for proportion in &infos[1..] {
			assert!(proportion.behavior.handle_positions.is_some());
			assert!(proportion.behavior.drag.is_some());
			assert!(proportion.max.is_some());
		}
	}

	#[test]
	fn all_existing_shapes_are_registered() {
		assert_eq!(registered_gizmo_nodes().len(), 7);
		for (_, infos) in registered_gizmo_nodes() {
			assert!(!infos.is_empty(), "every registered node must declare at least one gizmo");
		}
	}

	#[test]
	fn unregistered_node_returns_no_gizmos() {
		// The Fill node is not a generator, so it has no gizmos.
		assert!(get_gizmo_info(&graphene_std::vector_nodes::fill::IDENTIFIER).is_empty());
	}

	#[test]
	fn arc_sweep_outranks_the_radius_band_it_sits_on() {
		let infos = get_gizmo_info(&generator_nodes::arc::IDENTIFIER);
		let radius = infos.iter().find(|info| info.name == "Radius").expect("arc has a radius gizmo");
		let sweep = infos.iter().find(|info| info.name == "Sweep").expect("arc has a sweep gizmo");

		// The sweep endpoints lie on the circumference the radius is grabbed along, so the two overlap
		// everywhere the sweep is reachable. Only the point/region distinction separates them.
		assert!(radius.behavior.extended_target, "the radius is grabbed along the whole circumference");
		assert!(!sweep.behavior.extended_target, "the sweep is grabbed at its endpoints");
	}
}
