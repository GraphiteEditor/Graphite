//! # Gizmo Registry
//!
//! A data-driven lookup that maps node types to the parameters that should be exposed as
//! interactive canvas gizmos. This is the foundation of the *generic* gizmo system: instead of
//! writing a bespoke handler for every shape (see the `shape_gizmos` module for the legacy,
//! hand-written handlers), a node simply declares which of its inputs are gizmo-enabled here and
//! the generic gizmo manager builds the appropriate interactive handles automatically.
//!
//! To add gizmos to a new node:
//! 1. Add a `const` slice of [`GizmoInfo`] describing its gizmo-enabled parameters.
//! 2. Register the node's [`ProtoNodeIdentifier`] in [`registered_gizmo_nodes`].
//!
//! See `GENERIC_GIZMOS.md` (next to this file) for a full walkthrough.

use graph_craft::ProtoNodeIdentifier;
use graphene_std::NodeInputDecleration;
use graphene_std::vector::generator_nodes;
use graphene_std::vector::generator_nodes::{grid, spiral};

/// The kind of interactive control a gizmo presents, which also determines the underlying
/// [`TaggedValue`](graph_craft::document::value::TaggedValue) type of the parameter it edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GizmoType {
	/// A draggable handle that edits a continuous `f64` parameter (e.g. a radius or length).
	Slider,
	/// A rotary dial that edits a discrete `u32` parameter (e.g. a number of sides).
	Dial,
	/// A draggable point that edits a `DVec2` parameter (e.g. a position or 2D spacing).
	Position,
	/// A draggable handle constrained to a circle that edits an angle, stored as `f64` degrees.
	Angle,
}

/// A hint describing where a gizmo's handle should be anchored relative to its layer. Handle
/// positioning varies per node type, so this lets the registry declare intent while leaving the
/// concrete math to the generic gizmo implementations.
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

/// Describes a single gizmo-enabled parameter of a node: which input it edits, how it should be
/// presented, and the constraints/positioning that apply.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GizmoInfo {
	/// The index of the node input this gizmo edits.
	pub parameter_index: usize,
	/// The control type to instantiate for this parameter.
	pub gizmo_type: GizmoType,
	/// A human-readable name, shown in overlays/tooltips.
	pub name: &'static str,
	/// Inclusive lower bound for the value, if any.
	pub min: Option<f64>,
	/// Inclusive upper bound for the value, if any.
	pub max: Option<f64>,
	/// Where the gizmo's handle should be anchored.
	pub position_hint: PositionHint,
}

// --- Per-node gizmo declarations ------------------------------------------------------------

const CIRCLE_GIZMOS: &[GizmoInfo] = &[GizmoInfo {
	parameter_index: 1,
	gizmo_type: GizmoType::Slider,
	name: "Radius",
	min: Some(0.),
	max: None,
	position_hint: PositionHint::ParameterDerived,
}];

// Only the sides dial: a polygon's radius is already adjustable via the transform cage, and a
// `(radius, 0)` slider handle lands off the polygon's geometry, so it adds confusion without value.
const POLYGON_GIZMOS: &[GizmoInfo] = &[GizmoInfo {
	parameter_index: 1,
	gizmo_type: GizmoType::Dial,
	name: "Sides",
	min: Some(3.),
	max: None,
	position_hint: PositionHint::BoundingBoxCenter,
}];

const STAR_GIZMOS: &[GizmoInfo] = &[
	GizmoInfo {
		parameter_index: 1,
		gizmo_type: GizmoType::Dial,
		name: "Points",
		min: Some(3.),
		max: None,
		position_hint: PositionHint::BoundingBoxCenter,
	},
	GizmoInfo {
		parameter_index: 2,
		gizmo_type: GizmoType::Slider,
		name: "Outer Radius",
		min: Some(0.),
		max: None,
		position_hint: PositionHint::ParameterDerived,
	},
	GizmoInfo {
		parameter_index: 3,
		gizmo_type: GizmoType::Slider,
		name: "Inner Radius",
		min: Some(0.),
		max: None,
		position_hint: PositionHint::ParameterDerived,
	},
];

const ARC_GIZMOS: &[GizmoInfo] = &[
	GizmoInfo {
		parameter_index: 1,
		gizmo_type: GizmoType::Slider,
		name: "Radius",
		min: Some(0.),
		max: None,
		position_hint: PositionHint::ParameterDerived,
	},
	GizmoInfo {
		parameter_index: 2,
		gizmo_type: GizmoType::Angle,
		name: "Start Angle",
		min: None,
		max: None,
		position_hint: PositionHint::ParameterDerived,
	},
	GizmoInfo {
		parameter_index: 3,
		gizmo_type: GizmoType::Angle,
		name: "Sweep Angle",
		min: None,
		max: None,
		position_hint: PositionHint::ParameterDerived,
	},
];

const SPIRAL_GIZMOS: &[GizmoInfo] = &[
	GizmoInfo {
		parameter_index: spiral::InnerRadiusInput::INDEX,
		gizmo_type: GizmoType::Slider,
		name: "Inner Radius",
		min: Some(0.),
		max: None,
		position_hint: PositionHint::ParameterDerived,
	},
	GizmoInfo {
		parameter_index: spiral::OuterRadiusInput::INDEX,
		gizmo_type: GizmoType::Slider,
		name: "Outer Radius",
		min: Some(0.),
		max: None,
		position_hint: PositionHint::ParameterDerived,
	},
	GizmoInfo {
		parameter_index: spiral::TurnsInput::INDEX,
		gizmo_type: GizmoType::Slider,
		name: "Turns",
		min: Some(0.),
		max: None,
		position_hint: PositionHint::BoundingBoxEdge,
	},
];

// Only the radius slider: the heart's many shaping parameters (cleavage, lobes, shoulders, point)
// are fine-tuned via the Properties panel, while the overall size reads naturally as a canvas handle.
const HEART_GIZMOS: &[GizmoInfo] = &[GizmoInfo {
	parameter_index: 1,
	gizmo_type: GizmoType::Slider,
	name: "Radius",
	min: Some(0.),
	max: None,
	position_hint: PositionHint::ParameterDerived,
}];

const GRID_GIZMOS: &[GizmoInfo] = &[
	GizmoInfo {
		parameter_index: grid::ColumnsInput::INDEX,
		gizmo_type: GizmoType::Dial,
		name: "Columns",
		min: Some(1.),
		max: None,
		position_hint: PositionHint::BoundingBoxCorner,
	},
	GizmoInfo {
		parameter_index: grid::RowsInput::INDEX,
		gizmo_type: GizmoType::Dial,
		name: "Rows",
		min: Some(1.),
		max: None,
		position_hint: PositionHint::BoundingBoxCorner,
	},
	GizmoInfo {
		parameter_index: grid::SpacingInput::<f64>::INDEX,
		gizmo_type: GizmoType::Position,
		name: "Spacing",
		min: Some(0.),
		max: None,
		position_hint: PositionHint::BoundingBoxCorner,
	},
];

/// Returns every node type that has registered gizmos, paired with its gizmo declarations.
///
/// The identifier is cloned at call time because [`ProtoNodeIdentifier`]s are not trivially
/// usable as `'static` references in a `const`. This is cheap (the identifiers are backed by
/// `&'static str`) and only runs when a selection changes.
pub fn registered_gizmo_nodes() -> Vec<(ProtoNodeIdentifier, &'static [GizmoInfo])> {
	vec![
		(generator_nodes::circle::IDENTIFIER, CIRCLE_GIZMOS),
		(generator_nodes::regular_polygon::IDENTIFIER, POLYGON_GIZMOS),
		(generator_nodes::star::IDENTIFIER, STAR_GIZMOS),
		(generator_nodes::arc::IDENTIFIER, ARC_GIZMOS),
		(generator_nodes::spiral::IDENTIFIER, SPIRAL_GIZMOS),
		(generator_nodes::grid::IDENTIFIER, GRID_GIZMOS),
		(generator_nodes::heart::IDENTIFIER, HEART_GIZMOS),
	]
}

/// Looks up the gizmo declarations for a given node type. Returns an empty slice when the node
/// has no registered gizmos.
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
	fn polygon_exposes_only_a_sides_dial() {
		let infos = get_gizmo_info(&generator_nodes::regular_polygon::IDENTIFIER);
		assert_eq!(infos.len(), 1);

		let sides = &infos[0];
		assert_eq!(sides.gizmo_type, GizmoType::Dial);
		assert_eq!(sides.parameter_index, 1);
		assert_eq!(sides.min, Some(3.));

		// The radius is intentionally not exposed as a gizmo (handled by the transform cage instead).
		assert!(infos.iter().all(|info| info.gizmo_type != GizmoType::Slider));
	}

	#[test]
	fn star_exposes_a_points_dial_and_two_radius_sliders() {
		let infos = get_gizmo_info(&generator_nodes::star::IDENTIFIER);
		assert_eq!(infos.iter().filter(|info| info.gizmo_type == GizmoType::Dial).count(), 1);
		assert_eq!(infos.iter().filter(|info| info.gizmo_type == GizmoType::Slider).count(), 2);
	}

	#[test]
	fn heart_exposes_only_a_radius_slider() {
		let infos = get_gizmo_info(&generator_nodes::heart::IDENTIFIER);
		assert_eq!(infos.len(), 1);
		assert_eq!(infos[0].parameter_index, 1);
		assert_eq!(infos[0].gizmo_type, GizmoType::Slider);
		assert_eq!(infos[0].min, Some(0.));
		assert_eq!(infos[0].position_hint, PositionHint::ParameterDerived);
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
		// The Fill node is not a generator with gizmos, so it must return an empty slice.
		assert!(get_gizmo_info(&graphene_std::vector_nodes::fill::IDENTIFIER).is_empty());
	}
}
