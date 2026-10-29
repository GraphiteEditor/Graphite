//! The RFC's examples, each written the way an author would, followed by what `#[migration]` would expand it to.

use migration_core::{
	AnchorContext, Declaration, Downstream, Edits, Inputs, MigrationError, NewNode, Node, NodeInput, NodeMigration, OutputConnector, Region, Rename, Upstream, View, decl, encode, literal, literal_of,
	migration,
};
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

// ======
// Rename
// ======

pub fn add_rename() -> Rename {
	migration!(rename "graphene_core::ops::AddNode"@1 => "math_nodes::add"@2)
}

// ===============
// Opacity toggles
// ===============

/// Adds the opacity and fill toggles.
fn opacity_toggles(inputs: &mut Inputs) {
	inputs.insert(1, literal(Tagged::Bool(true)));
	inputs.push(literal(Tagged::Bool(false)));
	inputs.push(literal(Tagged::Number(100.)));
}

pub struct OpacityToggles;

impl NodeMigration for OpacityToggles {
	fn source(&self) -> Declaration {
		decl!("blending_nodes::opacity"@1)
	}
	fn target(&self) -> Declaration {
		decl!("blending_nodes::opacity"@2)
	}
	fn footprint(&self) -> Region {
		Region::ANCHOR.union(<Inputs as View>::REGION)
	}

	fn run(&self, context: &AnchorContext) -> Result<Edits, MigrationError> {
		let mut inputs = Inputs::extract(context);
		opacity_toggles(&mut inputs);
		inputs.into_edits()
	}
}

// =================
// Transform degrees
// =================

pub const ROTATION: u32 = 2;

/// Rotation is now stored in degrees.
fn transform_degrees(node: &Node, inputs: &mut Inputs, upstream: &mut Upstream) -> Result<(), MigrationError> {
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

pub struct TransformDegrees;

impl NodeMigration for TransformDegrees {
	fn source(&self) -> Declaration {
		decl!("transform_nodes::transform"@1)
	}
	fn target(&self) -> Declaration {
		decl!("transform_nodes::transform"@2)
	}
	fn pins(&self) -> &'static [Declaration] {
		const { &[decl!("math_nodes::multiply"@1)] }
	}
	fn footprint(&self) -> Region {
		Region::ANCHOR.union(<Inputs as View>::REGION).union(<Upstream as View>::REGION)
	}

	fn run(&self, context: &AnchorContext) -> Result<Edits, MigrationError> {
		let mut inputs = Inputs::extract(context);
		let mut upstream = Upstream::extract(context);
		transform_degrees(context.source_node(), &mut inputs, &mut upstream)?;

		let mut edits = inputs.into_edits()?;
		edits.merge(upstream.into_edits()?);
		Ok(edits)
	}
}

const _: () = assert!(Region::disjoint(&[<Inputs as View<'static>>::REGION, <Upstream<'static> as View<'static>>::REGION]));

// =====================
// Angle between degrees
// =====================

/// The output is now in degrees, so consumers keep receiving radians through a conversion.
fn angle_between_degrees(node: &Node, downstream: &mut Downstream) {
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

pub struct AngleBetweenDegrees;

impl NodeMigration for AngleBetweenDegrees {
	fn source(&self) -> Declaration {
		decl!("vector_nodes::angle_between"@1)
	}
	fn target(&self) -> Declaration {
		decl!("vector_nodes::angle_between"@2)
	}
	fn pins(&self) -> &'static [Declaration] {
		const { &[decl!("math_nodes::to_radians"@1)] }
	}
	fn footprint(&self) -> Region {
		Region::ANCHOR.union(<Downstream as View>::REGION)
	}

	fn run(&self, context: &AnchorContext) -> Result<Edits, MigrationError> {
		let mut downstream = Downstream::extract(context);
		angle_between_degrees(context.source_node(), &mut downstream);
		downstream.into_edits()
	}
}

// =========================
// Extract XY to Split Vec2
// =========================

/// Extract XY becomes Split Vec2 with one output per component. A wired axis selects between them at runtime.
fn extract_xy_to_split(node: &Node, inputs: &mut Inputs, downstream: &mut Downstream) -> Result<(), MigrationError> {
	let axis = &node.inputs()[1].input;

	let component = match literal_of::<Tagged>(axis) {
		Some(Tagged::XY(Axis::X)) => OutputConnector { node: node.id(), output: 0 },
		Some(Tagged::XY(Axis::Y)) => OutputConnector { node: node.id(), output: 1 },
		Some(_) => return Err(MigrationError::invariant("axis is not an XY value")),
		None if matches!(axis, NodeInput::Value { .. }) => return Err(MigrationError::invariant("axis is not an XY value")),
		None => {
			let select = downstream.add_node(NewNode {
				declaration: decl!("logic_nodes::select_axis"@1),
				inputs: vec![axis.clone(), NodeInput::Node { id: node.id(), index: 0 }, NodeInput::Node { id: node.id(), index: 1 }],
			});
			OutputConnector { node: select, output: 0 }
		}
	};

	// Source state only, so the select node added above isn't among these
	for consumer in downstream.consumers(0) {
		downstream.connect(component, consumer);
	}
	inputs.truncate(1);
	Ok(())
}

pub struct ExtractXyToSplit;

impl NodeMigration for ExtractXyToSplit {
	fn source(&self) -> Declaration {
		decl!("vector_nodes::extract_xy"@1)
	}
	fn target(&self) -> Declaration {
		decl!("vector_nodes::split_vec2"@2)
	}
	fn pins(&self) -> &'static [Declaration] {
		const { &[decl!("logic_nodes::select_axis"@1)] }
	}
	fn footprint(&self) -> Region {
		Region::ANCHOR.union(<Inputs as View>::REGION).union(<Downstream as View>::REGION)
	}

	fn run(&self, context: &AnchorContext) -> Result<Edits, MigrationError> {
		let mut inputs = Inputs::extract(context);
		let mut downstream = Downstream::extract(context);
		extract_xy_to_split(context.source_node(), &mut inputs, &mut downstream)?;

		let mut edits = inputs.into_edits()?;
		edits.merge(downstream.into_edits()?);
		Ok(edits)
	}
}

const _: () = assert!(Region::disjoint(&[<Inputs as View<'static>>::REGION, <Downstream<'static> as View<'static>>::REGION]));
