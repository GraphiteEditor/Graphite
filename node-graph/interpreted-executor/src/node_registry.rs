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
use node_registry_macros::{convert_node, into_node, record_extract_node, record_lift_node};
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
		// ============
		// REF ADAPTERS
		// ============
		#[cfg(target_family = "wasm")]
		#[cfg(target_family = "wasm")]
		#[cfg(target_family = "wasm")]
		record_lift_node!(f64),
		record_extract_node!(f64),
		record_lift_node!(()),
		record_extract_node!(()),
		record_lift_node!(bool),
		record_extract_node!(bool),
		record_lift_node!(u32),
		record_extract_node!(u32),
		record_lift_node!(u64),
		record_extract_node!(u64),
		record_lift_node!(f32),
		record_extract_node!(f32),
		record_lift_node!(DVec2),
		record_extract_node!(DVec2),
		record_lift_node!(IVec2),
		record_extract_node!(IVec2),
		record_lift_node!(DAffine2),
		record_extract_node!(DAffine2),
		record_lift_node!(Option<DAffine2>),
		record_extract_node!(Option<DAffine2>),
		record_lift_node!(Footprint),
		record_extract_node!(Footprint),
		record_lift_node!(SourceId),
		record_extract_node!(SourceId),
		record_lift_node!(BlendMode),
		record_extract_node!(BlendMode),
		record_lift_node!(graphene_std::vector::style::GradientType),
		record_extract_node!(graphene_std::vector::style::GradientType),
		record_lift_node!(graphene_std::vector::style::GradientSpreadMethod),
		record_extract_node!(graphene_std::vector::style::GradientSpreadMethod),
		record_lift_node!(String),
		record_extract_node!(String),
		record_lift_node!(List<String>),
		record_extract_node!(List<String>),
		record_lift_node!(List<NodeId>),
		record_extract_node!(List<NodeId>),
		record_lift_node!(List<f64>),
		record_extract_node!(List<f64>),
		record_lift_node!(List<u8>),
		record_extract_node!(List<u8>),
		record_lift_node!(List<Vector>),
		record_extract_node!(List<Vector>),
		record_lift_node!(List<Graphic>),
		record_extract_node!(List<Graphic>),
		record_lift_node!(List<Raster<CPU>>),
		record_extract_node!(List<Raster<CPU>>),
		#[cfg(feature = "gpu")]
		record_lift_node!(List<Raster<GPU>>),
		#[cfg(feature = "gpu")]
		record_extract_node!(List<Raster<GPU>>),
		record_lift_node!(List<Color>),
		record_extract_node!(List<Color>),
		record_lift_node!(List<Artboard>),
		record_extract_node!(List<Artboard>),
		record_lift_node!(List<GradientStops>),
		record_extract_node!(List<GradientStops>),
		record_lift_node!(AttributeDyn),
		record_extract_node!(AttributeDyn),
		record_lift_node!(AttributeValueDyn),
		record_extract_node!(AttributeValueDyn),
		record_lift_node!(ListDyn),
		record_extract_node!(ListDyn),
		record_lift_node!(std::sync::Arc<PlatformEditorApi>),
		record_extract_node!(std::sync::Arc<PlatformEditorApi>),
		record_lift_node!(RuntimeHandle),
		record_extract_node!(RuntimeHandle),
		record_lift_node!(RenderIntermediate),
		record_extract_node!(RenderIntermediate),
		record_lift_node!(RenderOutput),
		record_extract_node!(RenderOutput),
		#[cfg(target_family = "wasm")]
		record_lift_node!(CanvasHandle),
		#[cfg(target_family = "wasm")]
		record_extract_node!(CanvasHandle),
		#[cfg(feature = "gpu")]
		record_lift_node!(WgpuExecutorHandle),
		#[cfg(feature = "gpu")]
		record_extract_node!(WgpuExecutorHandle),
		#[cfg(feature = "gpu")]
		record_lift_node!(Option<WgpuExecutorHandle>),
		#[cfg(feature = "gpu")]
		record_extract_node!(Option<WgpuExecutorHandle>),
		#[cfg(feature = "gpu")]
		record_lift_node!(wgpu_executor::WgpuPipelineCache),
		#[cfg(feature = "gpu")]
		record_extract_node!(wgpu_executor::WgpuPipelineCache),
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

	node_types.extend(graph_craft::document::value::TaggedValue::record_bridge_entries());
	node_types.extend(core_types::registry::record_bridge_rows::<graphene_std::application_io::resource::Resource>());

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
					io: NodeIOTypes::new(concrete!(Context), concrete!($to), vec![fn_type!(Context, $from)]),
					constructor: |inputs| {
						if inputs.len() != 1 {
							return Err(ConstructionError::Arity { expected: 1, got: inputs.len() });
						}
						let mut inputs = inputs.into_iter();
						let node = graphene_std::ops::IntoNode::<$to, _>::new(inputs.next().unwrap().downcast::<$from>()?);
						Ok(EdgeHandle::new(std::sync::Arc::new(node) as std::sync::Arc<ErasedNode<$to>>))
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
						concrete!($to),
						vec![fn_type!(Context, $from), fn_type!(Context, $convert), fn_type!(Context, RuntimeHandle), fn_type!(Context, SourceId)],
					),
					constructor: |inputs| {
						if inputs.len() != 4 {
							return Err(ConstructionError::Arity { expected: 4, got: inputs.len() });
						}
						let mut inputs = inputs.into_iter();
						let node = graphene_std::ops::ConvertAsyncNode::<$to, _, _, _, _>::new(
							inputs.next().unwrap().downcast::<$from>()?,
							inputs.next().unwrap().downcast::<$convert>()?,
							inputs.next().unwrap().downcast::<RuntimeHandle>()?,
							inputs.next().unwrap().downcast::<SourceId>()?,
						);
						Ok(EdgeHandle::new(std::sync::Arc::new(node) as std::sync::Arc<ErasedNode<$to>>))
					},
				},
			)
		};
		(from: $from:ty, to: $to:ty, converter: $convert:ty) => {
			(
				ProtoNodeIdentifier::new(concat!["graphene_core::ops::ConvertNode<", stringify!($to), ">"]),
				RegistryEntry {
					io: NodeIOTypes::new(concrete!(Context), concrete!($to), vec![fn_type!(Context, $from), fn_type!(Context, $convert)]),
					constructor: |inputs| {
						if inputs.len() != 2 {
							return Err(ConstructionError::Arity { expected: 2, got: inputs.len() });
						}
						let mut inputs = inputs.into_iter();
						let node = graphene_std::ops::ConvertNode::<$to, _, _>::new(inputs.next().unwrap().downcast::<$from>()?, inputs.next().unwrap().downcast::<$convert>()?);
						Ok(EdgeHandle::new(std::sync::Arc::new(node) as std::sync::Arc<ErasedNode<$to>>))
					},
				},
			)
		};
	}

	macro_rules! record_lift_node {
		($type:ty) => {
			(
				ProtoNodeIdentifier::new("core_types::record::RecordLiftNode"),
				RegistryEntry {
					io: NodeIOTypes::new(concrete!(Context), core_types::registry::record_type::<$type>(), vec![fn_type!(Context, $type)]),
					constructor: |inputs| {
						if inputs.len() != 1 {
							return Err(ConstructionError::Arity { expected: 1, got: inputs.len() });
						}
						let mut inputs = inputs.into_iter();
						let node = core_types::record::RecordLift::<$type, _>::new(inputs.next().unwrap().downcast::<$type>()?);
						Ok(EdgeHandle::new_record::<$type>(std::sync::Arc::new(node) as std::sync::Arc<core_types::registry::ErasedRecordNode>))
					},
				},
			)
		};
	}

	macro_rules! record_extract_node {
		($type:ty) => {
			(
				ProtoNodeIdentifier::new("core_types::record::RecordExtractNode"),
				RegistryEntry {
					io: NodeIOTypes::new(concrete!(Context), concrete!($type), vec![core_types::registry::record_edge_type::<$type>()]),
					constructor: |inputs| {
						if inputs.len() != 1 {
							return Err(ConstructionError::Arity { expected: 1, got: inputs.len() });
						}
						let mut inputs = inputs.into_iter();
						let edge = inputs.next().unwrap();
						let layout = edge.layout().ok_or(ConstructionError::MissingLayout)?.clone();
						let node = core_types::record::RecordExtract::<$type, _>::new(edge.downcast_record::<$type>()?, &layout);
						Ok(EdgeHandle::new(std::sync::Arc::new(node) as std::sync::Arc<ErasedNode<$type>>))
					},
				},
			)
		};
	}

	pub(crate) use convert_node;
	pub(crate) use into_node;
	pub(crate) use record_extract_node;
	pub(crate) use record_lift_node;
}
