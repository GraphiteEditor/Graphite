use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use graphene_std::{NodeInputDecleration, ProtoNodeIdentifier};

/// Describes how a gizmo handle should be positioned relative to the shape.
#[derive(Clone, Debug, PartialEq)]
pub enum PositionHint {
	/// Positioned at the center of the shape's bounding box.
	BoundingBoxCenter,
	/// Positioned at the edge of the shape's bounding box (e.g., midpoint of a side).
	BoundingBoxEdge,
	/// Positioned at a corner of the shape's bounding box.
	BoundingBoxCorner,
	/// Position derived from a specific parameter value (e.g., a radius endpoint).
	/// The `usize` refers to the input index whose value determines the position.
	ParameterDerived(usize),
}

/// The kind of interactive control a gizmo provides.
#[derive(Clone, Debug, PartialEq)]
pub enum GizmoType {
	/// A linear slider controlling an `f64` parameter (e.g., radius, blur amount).
	Slider,
	/// A circular dial controlling a `u32` parameter (e.g., number of sides/points).
	Dial,
	/// A 2D position handle controlling a `DVec2` parameter (e.g., offset, center).
	Position,
	/// A rotary handle controlling an angle in degrees (`f64`).
	Angle,
}

/// Metadata describing a single gizmo-enabled parameter on a node.
#[derive(Clone, Debug, PartialEq)]
pub struct GizmoParameterInfo {
	/// Human-readable name of the parameter (e.g., "Radius", "Sides").
	pub name: &'static str,
	/// The node input index this gizmo controls.
	pub input_index: usize,
	/// What kind of gizmo to display.
	pub gizmo_type: GizmoType,
	/// Where to position the gizmo handle on the canvas.
	pub position_hint: PositionHint,
	/// Optional minimum value constraint.
	pub min: Option<f64>,
	/// Optional maximum value constraint.
	pub max: Option<f64>,
}

/// All gizmo information for a particular node type.
#[derive(Clone, Debug, PartialEq)]
pub struct GizmoInfo {
	/// The node identifier this info applies to.
	pub node_identifier: ProtoNodeIdentifier,
	/// The gizmo-enabled parameters for this node.
	pub parameters: Vec<GizmoParameterInfo>,
}

/// Returns gizmo metadata for a given node type, if it has gizmo-enabled parameters.
///
/// This is the central registry that maps node types to their interactive gizmo descriptions.
/// When a new node should support gizmos, add an entry here.
pub fn get_gizmo_info(identifier: &DefinitionIdentifier) -> Option<GizmoInfo> {
	let DefinitionIdentifier::ProtoNode(proto_id) = identifier else {
		return None;
	};

	use graphene_std::vector::generator_nodes::grid;
	use graphene_std::vector::generator_nodes::spiral;

	let parameters = if *proto_id == graphene_std::vector::generator_nodes::star::IDENTIFIER {
		vec![
			GizmoParameterInfo {
				name: "Sides",
				input_index: 1,
				gizmo_type: GizmoType::Dial,
				position_hint: PositionHint::BoundingBoxCenter,
				min: Some(3.0),
				max: None,
			},
			GizmoParameterInfo {
				name: "Outer Radius",
				input_index: 2,
				gizmo_type: GizmoType::Slider,
				position_hint: PositionHint::ParameterDerived(2),
				min: Some(0.0),
				max: None,
			},
			GizmoParameterInfo {
				name: "Inner Radius",
				input_index: 3,
				gizmo_type: GizmoType::Slider,
				position_hint: PositionHint::ParameterDerived(3),
				min: Some(0.0),
				max: None,
			},
		]
	} else if *proto_id == graphene_std::vector::generator_nodes::regular_polygon::IDENTIFIER {
		vec![
			GizmoParameterInfo {
				name: "Sides",
				input_index: 1,
				gizmo_type: GizmoType::Dial,
				position_hint: PositionHint::BoundingBoxCenter,
				min: Some(3.0),
				max: None,
			},
			GizmoParameterInfo {
				name: "Radius",
				input_index: 2,
				gizmo_type: GizmoType::Slider,
				position_hint: PositionHint::ParameterDerived(2),
				min: Some(0.0),
				max: None,
			},
		]
	} else if *proto_id == graphene_std::vector::generator_nodes::arc::IDENTIFIER {
		vec![
			GizmoParameterInfo {
				name: "Radius",
				input_index: 1,
				gizmo_type: GizmoType::Slider,
				position_hint: PositionHint::ParameterDerived(1),
				min: Some(0.0),
				max: None,
			},
			GizmoParameterInfo {
				name: "Start Angle",
				input_index: 2,
				gizmo_type: GizmoType::Angle,
				position_hint: PositionHint::ParameterDerived(2),
				min: Some(0.0),
				max: Some(360.0),
			},
			GizmoParameterInfo {
				name: "Sweep Angle",
				input_index: 3,
				gizmo_type: GizmoType::Angle,
				position_hint: PositionHint::ParameterDerived(3),
				min: Some(-360.0),
				max: Some(360.0),
			},
		]
	} else if *proto_id == graphene_std::vector::generator_nodes::circle::IDENTIFIER {
		vec![GizmoParameterInfo {
			name: "Radius",
			input_index: 1,
			gizmo_type: GizmoType::Slider,
			position_hint: PositionHint::ParameterDerived(1),
			min: Some(0.0),
			max: None,
		}]
	} else if *proto_id == graphene_std::vector::generator_nodes::grid::IDENTIFIER {
		vec![
			GizmoParameterInfo {
				name: "Columns",
				input_index: grid::ColumnsInput::INDEX,
				gizmo_type: GizmoType::Dial,
				position_hint: PositionHint::BoundingBoxEdge,
				min: Some(1.0),
				max: None,
			},
			GizmoParameterInfo {
				name: "Rows",
				input_index: grid::RowsInput::INDEX,
				gizmo_type: GizmoType::Dial,
				position_hint: PositionHint::BoundingBoxEdge,
				min: Some(1.0),
				max: None,
			},
		]
	} else if *proto_id == graphene_std::vector::generator_nodes::spiral::IDENTIFIER {
		vec![
			GizmoParameterInfo {
				name: "Turns",
				input_index: spiral::TurnsInput::INDEX,
				gizmo_type: GizmoType::Slider,
				position_hint: PositionHint::ParameterDerived(spiral::TurnsInput::INDEX),
				min: Some(0.0),
				max: None,
			},
			GizmoParameterInfo {
				name: "Outer Radius",
				input_index: spiral::OuterRadiusInput::INDEX,
				gizmo_type: GizmoType::Slider,
				position_hint: PositionHint::ParameterDerived(spiral::OuterRadiusInput::INDEX),
				min: Some(0.0),
				max: None,
			},
		]
	} else {
		return None;
	};

	Some(GizmoInfo {
		node_identifier: proto_id.clone(),
		parameters,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	fn def_id(proto: &ProtoNodeIdentifier) -> DefinitionIdentifier {
		DefinitionIdentifier::ProtoNode(proto.clone())
	}

	#[test]
	fn star_registry_entry_exists() {
		let info = get_gizmo_info(&def_id(&graphene_std::vector::generator_nodes::star::IDENTIFIER));
		assert!(info.is_some(), "Star should have a registry entry");
		let info = info.unwrap();
		assert_eq!(info.parameters.len(), 3);
		assert_eq!(info.parameters[0].name, "Sides");
		assert_eq!(info.parameters[0].gizmo_type, GizmoType::Dial);
		assert_eq!(info.parameters[1].name, "Outer Radius");
		assert_eq!(info.parameters[1].gizmo_type, GizmoType::Slider);
		assert_eq!(info.parameters[2].name, "Inner Radius");
		assert_eq!(info.parameters[2].input_index, 3);
	}

	#[test]
	fn polygon_registry_entry_exists() {
		let info = get_gizmo_info(&def_id(&graphene_std::vector::generator_nodes::regular_polygon::IDENTIFIER));
		assert!(info.is_some(), "Polygon should have a registry entry");
		let info = info.unwrap();
		assert_eq!(info.parameters.len(), 2);
		assert_eq!(info.parameters[0].name, "Sides");
		assert_eq!(info.parameters[0].min, Some(3.0));
		assert_eq!(info.parameters[1].name, "Radius");
	}

	#[test]
	fn arc_registry_entry_exists() {
		let info = get_gizmo_info(&def_id(&graphene_std::vector::generator_nodes::arc::IDENTIFIER));
		assert!(info.is_some(), "Arc should have a registry entry");
		let info = info.unwrap();
		assert_eq!(info.parameters.len(), 3);
		assert_eq!(info.parameters[0].name, "Radius");
		assert_eq!(info.parameters[0].gizmo_type, GizmoType::Slider);
		assert_eq!(info.parameters[1].name, "Start Angle");
		assert_eq!(info.parameters[1].gizmo_type, GizmoType::Angle);
		assert_eq!(info.parameters[2].name, "Sweep Angle");
		assert_eq!(info.parameters[2].gizmo_type, GizmoType::Angle);
		assert_eq!(info.parameters[2].min, Some(-360.0));
		assert_eq!(info.parameters[2].max, Some(360.0));
	}

	#[test]
	fn circle_registry_entry_exists() {
		let info = get_gizmo_info(&def_id(&graphene_std::vector::generator_nodes::circle::IDENTIFIER));
		assert!(info.is_some(), "Circle should have a registry entry");
		let info = info.unwrap();
		assert_eq!(info.parameters.len(), 1);
		assert_eq!(info.parameters[0].name, "Radius");
		assert_eq!(info.parameters[0].min, Some(0.0));
	}

	#[test]
	fn grid_registry_entry_exists() {
		let info = get_gizmo_info(&def_id(&graphene_std::vector::generator_nodes::grid::IDENTIFIER));
		assert!(info.is_some(), "Grid should have a registry entry");
		let info = info.unwrap();
		assert_eq!(info.parameters.len(), 2);
		assert_eq!(info.parameters[0].name, "Columns");
		assert_eq!(info.parameters[0].gizmo_type, GizmoType::Dial);
		assert_eq!(info.parameters[1].name, "Rows");
	}

	#[test]
	fn spiral_registry_entry_exists() {
		let info = get_gizmo_info(&def_id(&graphene_std::vector::generator_nodes::spiral::IDENTIFIER));
		assert!(info.is_some(), "Spiral should have a registry entry");
		let info = info.unwrap();
		assert_eq!(info.parameters.len(), 2);
		assert_eq!(info.parameters[0].name, "Turns");
		assert_eq!(info.parameters[1].name, "Outer Radius");
	}

	#[test]
	fn unknown_node_returns_none() {
		let fake_id = ProtoNodeIdentifier::new("fake::nonexistent::node");
		let info = get_gizmo_info(&def_id(&fake_id));
		assert!(info.is_none(), "Unknown node should return None");
	}

	#[test]
	fn all_six_shapes_registered() {
		let identifiers = [
			graphene_std::vector::generator_nodes::star::IDENTIFIER,
			graphene_std::vector::generator_nodes::regular_polygon::IDENTIFIER,
			graphene_std::vector::generator_nodes::arc::IDENTIFIER,
			graphene_std::vector::generator_nodes::circle::IDENTIFIER,
			graphene_std::vector::generator_nodes::grid::IDENTIFIER,
			graphene_std::vector::generator_nodes::spiral::IDENTIFIER,
		];
		for id in &identifiers {
			assert!(get_gizmo_info(&def_id(&*id)).is_some(), "Shape with identifier {:?} should be registered", id);
		}
	}

	#[test]
	fn parameter_indices_are_valid() {
		// Verify no parameter has input_index 0 (which is typically the primary input, not a parameter)
		let identifiers = [
			graphene_std::vector::generator_nodes::star::IDENTIFIER,
			graphene_std::vector::generator_nodes::regular_polygon::IDENTIFIER,
			graphene_std::vector::generator_nodes::arc::IDENTIFIER,
			graphene_std::vector::generator_nodes::circle::IDENTIFIER,
			graphene_std::vector::generator_nodes::grid::IDENTIFIER,
			graphene_std::vector::generator_nodes::spiral::IDENTIFIER,
		];
		for id in &identifiers {
			let info = get_gizmo_info(&def_id(&*id)).unwrap();
			for param in &info.parameters {
				assert!(param.input_index >= 1, "Parameter '{}' has input_index 0, which is typically the primary input", param.name);
			}
		}
	}

	#[test]
	fn slider_gizmos_have_non_negative_min() {
		let identifiers = [
			graphene_std::vector::generator_nodes::star::IDENTIFIER,
			graphene_std::vector::generator_nodes::regular_polygon::IDENTIFIER,
			graphene_std::vector::generator_nodes::circle::IDENTIFIER,
		];
		for id in &identifiers {
			let info = get_gizmo_info(&def_id(&*id)).unwrap();
			for param in &info.parameters {
				if param.gizmo_type == GizmoType::Slider {
					assert!(param.min.unwrap_or(0.0) >= 0.0, "Slider '{}' should have non-negative min", param.name);
				}
			}
		}
	}
}
