use glam::{DAffine2, DVec2, IVec2};
use graph_craft::application_io::PlatformEditorApi;
use graph_craft::document::value::RenderOutput;
use graphene_std::gradient::GradientStops;
use graphene_std::list::{AttributeDyn, AttributeValueDyn, List, ListDyn};
#[cfg(target_family = "wasm")]
use graphene_std::platform_application_io::canvas_utils::CanvasHandle;
#[cfg(feature = "gpu")]
use graphene_std::raster::GPU;
use graphene_std::raster::color::Color;
use graphene_std::raster::*;
use graphene_std::raster::{CPU, Raster};
use graphene_std::registry::{ConstructionError, EdgeHandle, ErasedNode, NodeIOTypes, RegistryEntry};
use graphene_std::render_node::RenderIntermediate;
use graphene_std::runtime::RuntimeHandle;
use graphene_std::transform::Footprint;
use graphene_std::uuid::NodeId;
use graphene_std::vector::Vector;
use graphene_std::{Artboard, Context, Graphic, ProtoNodeIdentifier, SourceId, concrete, fn_type};
use node_registry_macros::{convert_node, into_node};
use std::collections::HashMap;
#[cfg(feature = "gpu")]
use wgpu_executor::WgpuExecutorHandle;

fn node_registry() -> HashMap<ProtoNodeIdentifier, Vec<RegistryEntry>> {
	let mut node_types: Vec<(ProtoNodeIdentifier, RegistryEntry)> = vec![
		// ==========
		// INTO NODES
		// ==========
		into_node!(from: List<Graphic>, to: List<Graphic>),
		into_node!(from: List<Raster<CPU>>, to: List<Raster<CPU>>),
		#[cfg(feature = "gpu")]
		into_node!(from: List<Raster<GPU>>, to: List<Raster<GPU>>),
		convert_node!(from: List<Vector>, to: List<Graphic>),
		convert_node!(from: List<Raster<CPU>>, to: List<Graphic>),
		#[cfg(feature = "gpu")]
		convert_node!(from: List<Raster<GPU>>, to: List<Graphic>),
		// Type-erased attribute conversions for the `Attach Attribute` node, so it monomorphizes only over the destination `List` type.
		convert_node!(from: List<Artboard>, to: AttributeDyn),
		convert_node!(from: List<Graphic>, to: AttributeDyn),
		convert_node!(from: List<Vector>, to: AttributeDyn),
		convert_node!(from: List<Raster<CPU>>, to: AttributeDyn),
		convert_node!(from: List<Color>, to: AttributeDyn),
		convert_node!(from: List<GradientStops>, to: AttributeDyn),
		convert_node!(from: List<f64>, to: AttributeDyn),
		convert_node!(from: List<bool>, to: AttributeDyn),
		convert_node!(from: List<String>, to: AttributeDyn),
		convert_node!(from: List<DAffine2>, to: AttributeDyn),
		convert_node!(from: List<BlendMode>, to: AttributeDyn),
		convert_node!(from: List<graphene_std::vector::style::GradientType>, to: AttributeDyn),
		convert_node!(from: List<graphene_std::vector::style::GradientSpreadMethod>, to: AttributeDyn),
		convert_node!(from: List<Artboard>, to: ListDyn),
		convert_node!(from: List<Graphic>, to: ListDyn),
		convert_node!(from: List<Vector>, to: ListDyn),
		convert_node!(from: List<Raster<CPU>>, to: ListDyn),
		#[cfg(feature = "gpu")]
		convert_node!(from: List<Raster<GPU>>, to: ListDyn),
		convert_node!(from: List<Color>, to: ListDyn),
		convert_node!(from: List<GradientStops>, to: ListDyn),
		convert_node!(from: List<f64>, to: ListDyn),
		convert_node!(from: List<bool>, to: ListDyn),
		convert_node!(from: List<String>, to: ListDyn),
		convert_node!(from: List<u8>, to: ListDyn),
		convert_node!(from: List<NodeId>, to: ListDyn),
		convert_node!(from: List<DAffine2>, to: ListDyn),
		convert_node!(from: List<BlendMode>, to: ListDyn),
		convert_node!(from: List<graphene_std::vector::style::GradientType>, to: ListDyn),
		convert_node!(from: List<graphene_std::vector::style::GradientSpreadMethod>, to: ListDyn),
		// Type-erased attribute value conversions for the `Write Attribute` node, so it monomorphizes only over the destination `List` type.
		convert_node!(from: f64, to: AttributeValueDyn),
		convert_node!(from: u32, to: AttributeValueDyn),
		convert_node!(from: u64, to: AttributeValueDyn),
		convert_node!(from: bool, to: AttributeValueDyn),
		convert_node!(from: String, to: AttributeValueDyn),
		convert_node!(from: DVec2, to: AttributeValueDyn),
		convert_node!(from: DAffine2, to: AttributeValueDyn),
		convert_node!(from: Color, to: AttributeValueDyn),
		convert_node!(from: BlendMode, to: AttributeValueDyn),
		convert_node!(from: graphene_std::vector::style::GradientType, to: AttributeValueDyn),
		convert_node!(from: graphene_std::vector::style::GradientSpreadMethod, to: AttributeValueDyn),
		convert_node!(from: List<String>, to: AttributeValueDyn),
		convert_node!(from: List<NodeId>, to: AttributeValueDyn),
		convert_node!(from: List<Color>, to: AttributeValueDyn),
		convert_node!(from: List<GradientStops>, to: AttributeValueDyn),
		convert_node!(from: List<Vector>, to: AttributeValueDyn),
		convert_node!(from: List<Raster<CPU>>, to: AttributeValueDyn),
		convert_node!(from: List<Raster<GPU>>, to: AttributeValueDyn),
		convert_node!(from: List<Graphic>, to: AttributeValueDyn),
		// into_node!(from: List<Raster<CPU>>, to: List<Raster<SRGBA8>>),
		convert_node!(from: DVec2, to: DVec2),
		convert_node!(from: List<Vector>, to: List<Vector>),
		convert_node!(from: DVec2, to: List<Vector>),
		convert_node!(from: String, to: String),
		convert_node!(from: bool, to: String),
		convert_node!(from: DVec2, to: String),
		convert_node!(from: IVec2, to: String),
		convert_node!(from: DAffine2, to: String),
		#[cfg(feature = "gpu")]
		convert_node!(from: List<Raster<CPU>>, to: List<Raster<CPU>>, converter: WgpuExecutorHandle),
		#[cfg(feature = "gpu")]
		convert_node!(from: List<Raster<CPU>>, to: List<Raster<GPU>>, converter: WgpuExecutorHandle),
		#[cfg(feature = "gpu")]
		convert_node!(from: List<Raster<GPU>>, to: List<Raster<GPU>>, converter: WgpuExecutorHandle),
		#[cfg(feature = "gpu")]
		convert_node!(from: List<Raster<GPU>>, to: List<Raster<CPU>>, converter: WgpuExecutorHandle, async),
		// =============
		// MONITOR NODES
		// =============
		// ==========
		// MEMO NODES
		// ==========
	];
	// =============
	// CONVERT NODES
	// =============
	node_types.extend(
		[
			convert_node!(from: f32, to: numbers),
			convert_node!(from: f64, to: numbers),
			convert_node!(from: i8, to: numbers),
			convert_node!(from: u8, to: numbers),
			convert_node!(from: u16, to: numbers),
			convert_node!(from: i16, to: numbers),
			convert_node!(from: i32, to: numbers),
			convert_node!(from: u32, to: numbers),
			convert_node!(from: i64, to: numbers),
			convert_node!(from: u64, to: numbers),
			convert_node!(from: i128, to: numbers),
			convert_node!(from: u128, to: numbers),
			convert_node!(from: isize, to: numbers),
			convert_node!(from: usize, to: numbers),
			convert_node!(from: numbers, to: DVec2),
			convert_node!(from: numbers, to: String),
		]
		.into_iter()
		.flatten(),
	);


	let mut map: HashMap<ProtoNodeIdentifier, Vec<RegistryEntry>> = HashMap::new();
	let insert = |map: &mut HashMap<ProtoNodeIdentifier, Vec<RegistryEntry>>, id: ProtoNodeIdentifier, entry: RegistryEntry| {
		let rows = map.entry(id).or_default();
		if !rows.iter().any(|row| row.io == entry.io) {
			rows.push(entry);
		}
	};

	for (id, entries) in graphene_std::registry::NODE_REGISTRY.lock().unwrap().iter() {
		for entry in entries {
			insert(&mut map, id.clone(), entry.clone());
		}
	}

	for (id, entry) in node_types.into_iter() {
		// TODO: this is a hack to remove the newline from the node new_name
		// This occurs for the ChannelMixerNode presumably because of the long name.
		// This might be caused by the stringify! macro
		let mut new_name = id.as_str().replace('\n', " ");

		// Remove struct generics for all nodes except for the IntoNode and ConvertNode
		if !(new_name.contains("IntoNode") || new_name.contains("ConvertNode"))
			&& let Some((path, _generics)) = new_name.split_once("<")
		{
			new_name = path.to_string();
		}

		insert(&mut map, ProtoNodeIdentifier::with_owned_string(new_name), entry);
	}

	map
}

// TODO: Replace with `core::cell::LazyCell` (<https://doc.rust-lang.org/core/cell/struct.LazyCell.html>) or similar
pub static NODE_REGISTRY: once_cell::sync::Lazy<HashMap<ProtoNodeIdentifier, Vec<RegistryEntry>>> = once_cell::sync::Lazy::new(node_registry);

mod node_registry_macros {
	macro_rules! into_node {
		(from: $from:ty, to: $to:ty) => {
			(
				ProtoNodeIdentifier::new(concat!["graphene_core::ops::IntoNode<", stringify!($to), ">"]),
				RegistryEntry {
					io: NodeIOTypes::new(
						concrete!(Context),
						core_types::registry::record_type::<$to>(),
						vec![core_types::registry::record_edge_type::<$from>()],
					),
					constructor: |inputs| {
						if inputs.len() != 1 {
							return Err(ConstructionError::Arity { expected: 1, got: inputs.len() });
						}
						let mut inputs = inputs.into_iter();
						let handle = inputs.next().unwrap();
						let Some(layout) = handle.layout().cloned() else {
							return Err(ConstructionError::MissingLayout);
						};
						let node = graphene_std::ops::IntoNode::<$to, _, $from>::new(handle.downcast_record::<$from>()?, &layout);
						Ok(EdgeHandle::new_record::<$to>(std::sync::Arc::new(node) as std::sync::Arc<core_types::registry::ErasedRecordNode>))
					},
				},
			)
		};
	}
	macro_rules! convert_node {
		(from: $from:ty, to: numbers) => {{
			let x: Vec<(ProtoNodeIdentifier, RegistryEntry)> = vec![
				convert_node!(from: $from, to: f32),
				convert_node!(from: $from, to: f64),
				convert_node!(from: $from, to: i8),
				convert_node!(from: $from, to: u8),
				convert_node!(from: $from, to: u16),
				convert_node!(from: $from, to: i16),
				convert_node!(from: $from, to: i32),
				convert_node!(from: $from, to: u32),
				convert_node!(from: $from, to: i64),
				convert_node!(from: $from, to: u64),
				convert_node!(from: $from, to: i128),
				convert_node!(from: $from, to: u128),
				convert_node!(from: $from, to: isize),
				convert_node!(from: $from, to: usize),
			];
			x
		}};
		(from: numbers, to: $to:ty) => {{
			let x: Vec<(ProtoNodeIdentifier, RegistryEntry)> = vec![
				convert_node!(from: f32, to: $to),
				convert_node!(from: f64, to: $to),
				convert_node!(from: i8, to: $to),
				convert_node!(from: u8, to: $to),
				convert_node!(from: u16, to: $to),
				convert_node!(from: i16, to: $to),
				convert_node!(from: i32, to: $to),
				convert_node!(from: u32, to: $to),
				convert_node!(from: i64, to: $to),
				convert_node!(from: u64, to: $to),
				convert_node!(from: i128, to: $to),
				convert_node!(from: u128, to: $to),
				convert_node!(from: isize, to: $to),
				convert_node!(from: usize, to: $to),
			];
			x
		}};
		(from: $from:ty, to: $to:ty) => {
			convert_node!(from: $from, to: $to, converter: ())
		};
		(from: $from:ty, to: $to:ty, converter: $convert:ty, async) => {
			(
				ProtoNodeIdentifier::new(concat!["graphene_core::ops::ConvertNode<", stringify!($to), ">"]),
				RegistryEntry {
					io: NodeIOTypes::new(
						concrete!(Context),
						core_types::registry::record_type::<$to>(),
						vec![
							core_types::registry::record_edge_type::<$from>(),
							core_types::registry::record_edge_type::<$convert>(),
							core_types::registry::record_edge_type::<RuntimeHandle>(),
							core_types::registry::record_edge_type::<SourceId>(),
						],
					),
					constructor: |inputs| {
						if inputs.len() != 4 {
							return Err(ConstructionError::Arity { expected: 4, got: inputs.len() });
						}
						let mut inputs = inputs.into_iter();
						let mut claim = || {
							let handle = inputs.next().unwrap();
							let Some(layout) = handle.layout().cloned() else {
								return Err(ConstructionError::MissingLayout);
							};
							Ok((handle, layout))
						};
						let (value, value_layout) = claim()?;
						let (converter, converter_layout) = claim()?;
						let (runtime, runtime_layout) = claim()?;
						let (source, source_layout) = claim()?;
						let node = graphene_std::ops::ConvertAsyncNode::<$to, _, _, _, _, $from, $convert>::new(
							value.downcast_record::<$from>()?,
							converter.downcast_record::<$convert>()?,
							runtime.downcast_record::<RuntimeHandle>()?,
							source.downcast_record::<SourceId>()?,
							&value_layout,
							&converter_layout,
							&runtime_layout,
							&source_layout,
						);
						Ok(EdgeHandle::new_record::<$to>(std::sync::Arc::new(node) as std::sync::Arc<core_types::registry::ErasedRecordNode>))
					},
				},
			)
		};
		(from: $from:ty, to: $to:ty, converter: $convert:ty) => {
			(
				ProtoNodeIdentifier::new(concat!["graphene_core::ops::ConvertNode<", stringify!($to), ">"]),
				RegistryEntry {
					io: NodeIOTypes::new(
						concrete!(Context),
						core_types::registry::record_type::<$to>(),
						vec![core_types::registry::record_edge_type::<$from>(), core_types::registry::record_edge_type::<$convert>()],
					),
					constructor: |inputs| {
						if inputs.len() != 2 {
							return Err(ConstructionError::Arity { expected: 2, got: inputs.len() });
						}
						let mut inputs = inputs.into_iter();
						let value = inputs.next().unwrap();
						let Some(value_layout) = value.layout().cloned() else {
							return Err(ConstructionError::MissingLayout);
						};
						let converter = inputs.next().unwrap();
						let Some(converter_layout) = converter.layout().cloned() else {
							return Err(ConstructionError::MissingLayout);
						};
						let node = graphene_std::ops::ConvertNode::<$to, _, _, $from, $convert>::new(
							value.downcast_record::<$from>()?,
							converter.downcast_record::<$convert>()?,
							&value_layout,
							&converter_layout,
						);
						Ok(EdgeHandle::new_record::<$to>(std::sync::Arc::new(node) as std::sync::Arc<core_types::registry::ErasedRecordNode>))
					},
				},
			)
		};
	}


	pub(crate) use convert_node;
	pub(crate) use into_node;
}

#[cfg(test)]
mod tests {
	use super::*;
	use graphene_std::Type;

	fn is_record_edge(ty: &Type) -> bool {
		match ty {
			Type::Fn(_, output) => matches!(&**output, Type::Record(_)),
			_ => false,
		}
	}

	/// One wire kind: every row consumes and produces record wires. A plain
	/// io type here would need a bridge adapter, and those are gone.
	#[test]
	fn every_registry_row_is_record_typed() {
		let mut plain: Vec<String> = Vec::new();
		for (id, entries) in NODE_REGISTRY.iter() {
			for entry in entries {
				let plain_output = !matches!(entry.io.return_value, Type::Record(_));
				let plain_inputs: Vec<usize> = entry.io.inputs.iter().enumerate().filter(|(_, ty)| !is_record_edge(ty)).map(|(index, _)| index).collect();
				if plain_output || !plain_inputs.is_empty() {
					plain.push(format!("{} plain_output={plain_output} plain_inputs={plain_inputs:?}", id.as_str()));
				}
			}
		}
		plain.sort();
		assert!(plain.is_empty(), "plain io remains in the registry:\n{}", plain.join("\n"));
	}
}
