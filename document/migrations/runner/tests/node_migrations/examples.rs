//! The RFC's examples, written the way a migration crate would. `#[migration]` generates a unit struct per function, and the
//! registry picks every migration up at startup.

use migration_core::{Downstream, Inputs, MigrationError, NewNode, Node, NodeInput, OutputConnector, Upstream, decl, encode, literal, literal_of, migration, rename};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Frozen mirror of the `TaggedValue` variants these migrations use.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Tagged {
	Number(f64),
	Bool(bool),
	XY(Axis),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
	X,
	Y,
}

rename!("graphene_core::ops::AddNode"@1 => "math_nodes::add"@2);

// Later steps of the lineages below, so ordering has chains and pins to respect
rename!("math_nodes::multiply"@1 => "math_nodes::product"@2);
rename!("vector_nodes::split_vec2"@2 => "vector_nodes::split"@3);

/// Adds the opacity and fill toggles.
#[migration("blending_nodes::opacity"@1 => @2)]
pub fn opacity_toggles(inputs: &mut Inputs) {
	inputs.insert(1, literal(Tagged::Bool(true)));
	inputs.push(literal(Tagged::Bool(false)));
	inputs.push(literal(Tagged::Number(100.)));
}

pub const ROTATION: u32 = 2;

/// Rotation is now stored in degrees.
#[migration("transform_nodes::transform"@1 => @2)]
pub fn transform_degrees(node: &Node, inputs: &mut Inputs, upstream: &mut Upstream) -> Result<(), MigrationError> {
	let rotation = &node.inputs()[ROTATION as usize].input;

	let converted = match rotation {
		NodeInput::Value { exposed, .. } => {
			let Some(Tagged::Number(radians)) = literal_of(rotation) else {
				return Err(MigrationError::invariant("rotation is not a number"));
			};
			NodeInput::Value {
				value: encode(&Tagged::Number(radians.to_degrees())),
				exposed: *exposed,
			}
		}
		// Only known at runtime, so convert upstream
		wired => {
			let multiply = upstream.add_node(NewNode {
				declaration: decl!("math_nodes::multiply"@1),
				inputs: vec![wired.clone(), literal(Tagged::Number(180. / PI))],
			});
			NodeInput::Node { id: multiply, index: 0 }
		}
	};

	inputs.set(ROTATION, converted);
	Ok(())
}

/// The output is now in degrees, so consumers keep receiving radians through a conversion.
#[migration("vector_nodes::angle_between"@1 => @2)]
pub fn angle_between_degrees(node: &Node, downstream: &mut Downstream) {
	let consumers = downstream.consumers(0);
	if consumers.is_empty() {
		return;
	}

	let conversion = downstream.add_node(NewNode {
		declaration: decl!("math_nodes::to_radians"@1),
		inputs: vec![NodeInput::Node { id: node.id(), index: 0 }],
	});
	for consumer in consumers {
		downstream.connect(OutputConnector { node: conversion, output: 0 }, consumer);
	}
}

/// Extract XY becomes Split Vec2 with one output per component. A wired axis selects between them at runtime.
#[migration("vector_nodes::extract_xy"@1 => "vector_nodes::split_vec2"@2)]
pub fn extract_xy_to_split(node: &Node, inputs: &mut Inputs, downstream: &mut Downstream) -> Result<(), MigrationError> {
	let axis = &node.inputs()[1].input;
	let consumers = downstream.consumers(0);

	let component = match literal_of::<Tagged>(axis) {
		Some(Tagged::XY(Axis::X)) => OutputConnector { node: node.id(), output: 0 },
		Some(Tagged::XY(Axis::Y)) => OutputConnector { node: node.id(), output: 1 },
		Some(_) => return Err(MigrationError::invariant("axis is not an XY value")),
		None if matches!(axis, NodeInput::Value { .. }) => return Err(MigrationError::invariant("axis is not an XY value")),
		// Nothing reads the output, so nothing needs a selector
		None if consumers.is_empty() => {
			inputs.truncate(1);
			return Ok(());
		}
		None => {
			let select = downstream.add_node(NewNode {
				declaration: decl!("logic_nodes::select_axis"@1),
				inputs: vec![axis.clone(), NodeInput::Node { id: node.id(), index: 0 }, NodeInput::Node { id: node.id(), index: 1 }],
			});
			OutputConnector { node: select, output: 0 }
		}
	};

	for consumer in consumers {
		downstream.connect(component, consumer);
	}
	inputs.truncate(1);
	Ok(())
}
