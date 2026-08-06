use glam::{DAffine2, DVec2, IVec2};
use graph_craft::application_io::PlatformEditorApi;
use graph_craft::document::DocumentNode;
use graph_craft::document::value::RenderOutput;
use graphene_std::application_io::Texture;
use graphene_std::brush::brush_stroke::BrushStroke;
use graphene_std::gradient::GradientStops;
use graphene_std::list::{AttributeDyn, AttributeValueDyn, List, ListDyn};
#[cfg(target_family = "wasm")]
use graphene_std::platform_application_io::canvas_utils::CanvasHandle;
#[cfg(feature = "gpu")]
use graphene_std::raster::GPU;
use graphene_std::raster::color::Color;
use graphene_std::raster::*;
use graphene_std::raster::{CPU, Raster};
use graphene_std::registry::{ConstructionError, EdgeHandle, ErasedLendNode, ErasedNode, NodeIOTypes, RegistryEntry, lend_edge_type, ref_type};
use graphene_std::render_node::RenderIntermediate;
use graphene_std::runtime::RuntimeHandle;
use graphene_std::transform::Footprint;
use graphene_std::uuid::NodeId;
use graphene_std::vector::Vector;
use graphene_std::{Artboard, Context, Graphic, ProtoNodeIdentifier, SourceId, concrete, fn_type};
use node_registry_macros::{async_node, clone_node, convert_node, frame_memo_node, into_node, lend_node, record_extract_node, record_lift_node};
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
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => ()]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => List<Artboard>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => List<Graphic>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => List<Vector>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => List<Raster<CPU>>]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => List<Raster<GPU>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => List<Color>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => List<GradientStops>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => Image<Color>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => String]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => IVec2]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => DVec2]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => DAffine2]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => Option<DAffine2>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => bool]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => f64]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => u32]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => u64]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => BlendMode]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => Texture]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::application_io::resource::Resource]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::transform::ReferencePoint]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::BooleanOperation]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::style::StrokeCap]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::style::StrokeJoin]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::style::PaintOrder]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::style::StrokeAlign]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::style::Stroke]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => Box<graphene_std::vector::VectorModification>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::CentroidType]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::PointSpacingType]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => Option<f64>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => List<String>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => List<NodeId>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => List<f64>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => List<u8>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => List<bool>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => List<DAffine2>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => List<BlendMode>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => List<graphene_std::vector::style::GradientType>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => List<graphene_std::vector::style::GradientSpreadMethod>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => AttributeDyn]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => AttributeValueDyn]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => ListDyn]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => Graphic]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::text::Font]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => List<BrushStroke>]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => DocumentNode]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::transform::Footprint]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::blending::BlendMode]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::LuminanceCalculation]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::extract_xy::XY]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::text_nodes::StringCapitalization]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::RedGreenBlue]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::RedGreenBlueAlpha]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::animation::RealTimeMode]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::NoiseType]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::FractalType]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::CellularDistanceFunction]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::CellularReturnType]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::DomainWarpType]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::RelativeAbsolute]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::SelectiveColorChoice]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::GridType]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::ArcType]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::RowsOrColumns]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::MergeByDistanceAlgorithm]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::ExtrudeJoiningAlgorithm]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::PointSpacingType]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::style::GradientType]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::style::GradientSpreadMethod]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::transform::ReferencePoint]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::CentroidType]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::text::TextAlign]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::transform::ScaleType]),
		async_node!(graphene_core::memo::MonitorNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::InterpolationDistribution]),
		// ==========
		// MEMO NODES
		// ==========
		// ============
		// REF ADAPTERS
		// ============
		lend_node!(()),
		clone_node!(()),
		frame_memo_node!(()),
		lend_node!(RuntimeHandle),
		clone_node!(RuntimeHandle),
		frame_memo_node!(RuntimeHandle),
		lend_node!(SourceId),
		clone_node!(SourceId),
		frame_memo_node!(SourceId),
		lend_node!(bool),
		clone_node!(bool),
		frame_memo_node!(bool),
		lend_node!(List<Artboard>),
		clone_node!(List<Artboard>),
		frame_memo_node!(List<Artboard>),
		lend_node!(List<Graphic>),
		clone_node!(List<Graphic>),
		frame_memo_node!(List<Graphic>),
		lend_node!(List<Vector>),
		clone_node!(List<Vector>),
		frame_memo_node!(List<Vector>),
		lend_node!(List<Raster<CPU>>),
		clone_node!(List<Raster<CPU>>),
		frame_memo_node!(List<Raster<CPU>>),
		lend_node!(List<Color>),
		clone_node!(List<Color>),
		frame_memo_node!(List<Color>),
		lend_node!(Image<Color>),
		clone_node!(Image<Color>),
		frame_memo_node!(Image<Color>),
		lend_node!(List<GradientStops>),
		clone_node!(List<GradientStops>),
		frame_memo_node!(List<GradientStops>),
		lend_node!(List<String>),
		clone_node!(List<String>),
		frame_memo_node!(List<String>),
		lend_node!(List<NodeId>),
		clone_node!(List<NodeId>),
		frame_memo_node!(List<NodeId>),
		lend_node!(List<f64>),
		clone_node!(List<f64>),
		frame_memo_node!(List<f64>),
		lend_node!(List<u8>),
		clone_node!(List<u8>),
		frame_memo_node!(List<u8>),
		lend_node!(List<bool>),
		clone_node!(List<bool>),
		frame_memo_node!(List<bool>),
		lend_node!(List<DAffine2>),
		clone_node!(List<DAffine2>),
		frame_memo_node!(List<DAffine2>),
		lend_node!(List<BlendMode>),
		clone_node!(List<BlendMode>),
		frame_memo_node!(List<BlendMode>),
		lend_node!(List<graphene_std::vector::style::GradientType>),
		clone_node!(List<graphene_std::vector::style::GradientType>),
		frame_memo_node!(List<graphene_std::vector::style::GradientType>),
		lend_node!(List<graphene_std::vector::style::GradientSpreadMethod>),
		clone_node!(List<graphene_std::vector::style::GradientSpreadMethod>),
		frame_memo_node!(List<graphene_std::vector::style::GradientSpreadMethod>),
		lend_node!(AttributeDyn),
		clone_node!(AttributeDyn),
		frame_memo_node!(AttributeDyn),
		lend_node!(AttributeValueDyn),
		clone_node!(AttributeValueDyn),
		frame_memo_node!(AttributeValueDyn),
		lend_node!(ListDyn),
		clone_node!(ListDyn),
		frame_memo_node!(ListDyn),
		#[cfg(target_family = "wasm")]
		lend_node!(CanvasHandle),
		#[cfg(target_family = "wasm")]
		clone_node!(CanvasHandle),
		#[cfg(target_family = "wasm")]
		frame_memo_node!(CanvasHandle),
		lend_node!(f64),
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
		(
			ProtoNodeIdentifier::new("graphene_core::memo::MonitorNode"),
			RegistryEntry {
				io: NodeIOTypes::new(
					concrete!(Context),
					core_types::Type::Record(Box::new(core_types::Type::Generic(std::borrow::Cow::Borrowed("T")))),
					vec![core_types::registry::generic_record_edge_type("T")],
				),
				constructor: |inputs| {
					if inputs.len() != 1 {
						return Err(ConstructionError::Arity { expected: 1, got: inputs.len() });
					}
					let mut inputs = inputs.into_iter();
					let handle = inputs.next().unwrap();
					let ty = handle.ty().clone();
					let Some(layout) = handle.layout().cloned() else {
						return Err(ConstructionError::MissingLayout);
					};
					let edge = handle.downcast_erased::<core_types::registry::ErasedRecordNode>(ty.clone())?;
					let node = core_types::record::RecordMonitor::new(edge, &layout);
					Ok(EdgeHandle::new_erased(std::sync::Arc::new(node) as std::sync::Arc<core_types::registry::ErasedRecordNode>, ty))
				},
			},
		),
		clone_node!(f64),
		frame_memo_node!(f64),
		lend_node!(f32),
		clone_node!(f32),
		frame_memo_node!(f32),
		lend_node!(u32),
		clone_node!(u32),
		frame_memo_node!(u32),
		lend_node!(u64),
		clone_node!(u64),
		frame_memo_node!(u64),
		lend_node!(DVec2),
		clone_node!(DVec2),
		frame_memo_node!(DVec2),
		lend_node!(String),
		clone_node!(String),
		frame_memo_node!(String),
		lend_node!(DAffine2),
		clone_node!(DAffine2),
		frame_memo_node!(DAffine2),
		lend_node!(Footprint),
		clone_node!(Footprint),
		frame_memo_node!(Footprint),
		lend_node!(RenderOutput),
		clone_node!(RenderOutput),
		frame_memo_node!(RenderOutput),
		lend_node!(std::sync::Arc<PlatformEditorApi>),
		clone_node!(std::sync::Arc<PlatformEditorApi>),
		frame_memo_node!(std::sync::Arc<PlatformEditorApi>),
		#[cfg(feature = "gpu")]
		lend_node!(List<Raster<GPU>>),
		#[cfg(feature = "gpu")]
		clone_node!(List<Raster<GPU>>),
		#[cfg(feature = "gpu")]
		frame_memo_node!(List<Raster<GPU>>),
		lend_node!(Option<f64>),
		clone_node!(Option<f64>),
		frame_memo_node!(Option<f64>),
		lend_node!(Option<Color>),
		clone_node!(Option<Color>),
		frame_memo_node!(Option<Color>),
		lend_node!(Graphic),
		clone_node!(Graphic),
		frame_memo_node!(Graphic),
		lend_node!(glam::f32::Vec2),
		clone_node!(glam::f32::Vec2),
		frame_memo_node!(glam::f32::Vec2),
		lend_node!(glam::f32::Affine2),
		clone_node!(glam::f32::Affine2),
		frame_memo_node!(glam::f32::Affine2),
		lend_node!(graphene_std::vector::style::Stroke),
		clone_node!(graphene_std::vector::style::Stroke),
		frame_memo_node!(graphene_std::vector::style::Stroke),
		lend_node!(graphene_std::text::Font),
		clone_node!(graphene_std::text::Font),
		frame_memo_node!(graphene_std::text::Font),
		lend_node!(List<BrushStroke>),
		clone_node!(List<BrushStroke>),
		frame_memo_node!(List<BrushStroke>),
		lend_node!(DocumentNode),
		clone_node!(DocumentNode),
		frame_memo_node!(DocumentNode),
		lend_node!(graphene_std::ContextModification),
		clone_node!(graphene_std::ContextModification),
		frame_memo_node!(graphene_std::ContextModification),
		lend_node!(graphene_std::transform::Footprint),
		clone_node!(graphene_std::transform::Footprint),
		frame_memo_node!(graphene_std::transform::Footprint),
		lend_node!(Box<graphene_std::vector::VectorModification>),
		clone_node!(Box<graphene_std::vector::VectorModification>),
		frame_memo_node!(Box<graphene_std::vector::VectorModification>),
		lend_node!(graphene_std::blending::BlendMode),
		clone_node!(graphene_std::blending::BlendMode),
		frame_memo_node!(graphene_std::blending::BlendMode),
		lend_node!(graphene_std::raster::LuminanceCalculation),
		clone_node!(graphene_std::raster::LuminanceCalculation),
		frame_memo_node!(graphene_std::raster::LuminanceCalculation),
		lend_node!(graphene_std::vector::QRCodeErrorCorrectionLevel),
		clone_node!(graphene_std::vector::QRCodeErrorCorrectionLevel),
		frame_memo_node!(graphene_std::vector::QRCodeErrorCorrectionLevel),
		lend_node!(graphene_std::extract_xy::XY),
		clone_node!(graphene_std::extract_xy::XY),
		frame_memo_node!(graphene_std::extract_xy::XY),
		lend_node!(graphene_std::text_nodes::StringCapitalization),
		clone_node!(graphene_std::text_nodes::StringCapitalization),
		frame_memo_node!(graphene_std::text_nodes::StringCapitalization),
		lend_node!(graphene_std::raster::RedGreenBlue),
		clone_node!(graphene_std::raster::RedGreenBlue),
		frame_memo_node!(graphene_std::raster::RedGreenBlue),
		lend_node!(graphene_std::raster::RedGreenBlueAlpha),
		clone_node!(graphene_std::raster::RedGreenBlueAlpha),
		frame_memo_node!(graphene_std::raster::RedGreenBlueAlpha),
		lend_node!(graphene_std::animation::RealTimeMode),
		clone_node!(graphene_std::animation::RealTimeMode),
		frame_memo_node!(graphene_std::animation::RealTimeMode),
		lend_node!(graphene_std::raster::NoiseType),
		clone_node!(graphene_std::raster::NoiseType),
		frame_memo_node!(graphene_std::raster::NoiseType),
		lend_node!(graphene_std::raster::FractalType),
		clone_node!(graphene_std::raster::FractalType),
		frame_memo_node!(graphene_std::raster::FractalType),
		lend_node!(graphene_std::raster::CellularDistanceFunction),
		clone_node!(graphene_std::raster::CellularDistanceFunction),
		frame_memo_node!(graphene_std::raster::CellularDistanceFunction),
		lend_node!(graphene_std::raster::CellularReturnType),
		clone_node!(graphene_std::raster::CellularReturnType),
		frame_memo_node!(graphene_std::raster::CellularReturnType),
		lend_node!(graphene_std::raster::DomainWarpType),
		clone_node!(graphene_std::raster::DomainWarpType),
		frame_memo_node!(graphene_std::raster::DomainWarpType),
		lend_node!(graphene_std::raster::RelativeAbsolute),
		clone_node!(graphene_std::raster::RelativeAbsolute),
		frame_memo_node!(graphene_std::raster::RelativeAbsolute),
		lend_node!(graphene_std::raster::SelectiveColorChoice),
		clone_node!(graphene_std::raster::SelectiveColorChoice),
		frame_memo_node!(graphene_std::raster::SelectiveColorChoice),
		lend_node!(graphene_std::vector::misc::GridType),
		clone_node!(graphene_std::vector::misc::GridType),
		frame_memo_node!(graphene_std::vector::misc::GridType),
		lend_node!(graphene_std::vector::misc::ArcType),
		clone_node!(graphene_std::vector::misc::ArcType),
		frame_memo_node!(graphene_std::vector::misc::ArcType),
		lend_node!(graphene_std::vector::misc::RowsOrColumns),
		clone_node!(graphene_std::vector::misc::RowsOrColumns),
		frame_memo_node!(graphene_std::vector::misc::RowsOrColumns),
		lend_node!(graphene_std::vector::misc::MergeByDistanceAlgorithm),
		clone_node!(graphene_std::vector::misc::MergeByDistanceAlgorithm),
		frame_memo_node!(graphene_std::vector::misc::MergeByDistanceAlgorithm),
		lend_node!(graphene_std::vector::misc::ExtrudeJoiningAlgorithm),
		clone_node!(graphene_std::vector::misc::ExtrudeJoiningAlgorithm),
		frame_memo_node!(graphene_std::vector::misc::ExtrudeJoiningAlgorithm),
		lend_node!(graphene_std::vector::misc::PointSpacingType),
		clone_node!(graphene_std::vector::misc::PointSpacingType),
		frame_memo_node!(graphene_std::vector::misc::PointSpacingType),
		lend_node!(graphene_std::vector::style::StrokeCap),
		clone_node!(graphene_std::vector::style::StrokeCap),
		frame_memo_node!(graphene_std::vector::style::StrokeCap),
		lend_node!(graphene_std::vector::style::StrokeJoin),
		clone_node!(graphene_std::vector::style::StrokeJoin),
		frame_memo_node!(graphene_std::vector::style::StrokeJoin),
		lend_node!(graphene_std::vector::style::StrokeAlign),
		clone_node!(graphene_std::vector::style::StrokeAlign),
		frame_memo_node!(graphene_std::vector::style::StrokeAlign),
		lend_node!(graphene_std::vector::style::PaintOrder),
		clone_node!(graphene_std::vector::style::PaintOrder),
		frame_memo_node!(graphene_std::vector::style::PaintOrder),
		lend_node!(graphene_std::vector::style::GradientType),
		clone_node!(graphene_std::vector::style::GradientType),
		frame_memo_node!(graphene_std::vector::style::GradientType),
		lend_node!(graphene_std::vector::style::GradientSpreadMethod),
		clone_node!(graphene_std::vector::style::GradientSpreadMethod),
		frame_memo_node!(graphene_std::vector::style::GradientSpreadMethod),
		lend_node!(Option<DAffine2>),
		clone_node!(Option<DAffine2>),
		frame_memo_node!(Option<DAffine2>),
		lend_node!(graphene_std::transform::ReferencePoint),
		clone_node!(graphene_std::transform::ReferencePoint),
		frame_memo_node!(graphene_std::transform::ReferencePoint),
		lend_node!(graphene_std::vector::misc::CentroidType),
		clone_node!(graphene_std::vector::misc::CentroidType),
		frame_memo_node!(graphene_std::vector::misc::CentroidType),
		lend_node!(graphene_std::vector::misc::BooleanOperation),
		clone_node!(graphene_std::vector::misc::BooleanOperation),
		frame_memo_node!(graphene_std::vector::misc::BooleanOperation),
		lend_node!(graphene_std::text::TextAlign),
		clone_node!(graphene_std::text::TextAlign),
		frame_memo_node!(graphene_std::text::TextAlign),
		lend_node!(graphene_std::transform::ScaleType),
		clone_node!(graphene_std::transform::ScaleType),
		frame_memo_node!(graphene_std::transform::ScaleType),
		lend_node!(graphene_std::vector::misc::InterpolationDistribution),
		clone_node!(graphene_std::vector::misc::InterpolationDistribution),
		frame_memo_node!(graphene_std::vector::misc::InterpolationDistribution),
		lend_node!(RenderIntermediate),
		clone_node!(RenderIntermediate),
		frame_memo_node!(RenderIntermediate),
		lend_node!(wgpu_executor::WgpuExecutorHandle),
		clone_node!(wgpu_executor::WgpuExecutorHandle),
		frame_memo_node!(wgpu_executor::WgpuExecutorHandle),
		lend_node!(Option<wgpu_executor::WgpuExecutorHandle>),
		clone_node!(Option<wgpu_executor::WgpuExecutorHandle>),
		frame_memo_node!(Option<wgpu_executor::WgpuExecutorHandle>),
		lend_node!(wgpu_executor::WgpuPipelineCache),
		clone_node!(wgpu_executor::WgpuPipelineCache),
		frame_memo_node!(wgpu_executor::WgpuPipelineCache),
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
	macro_rules! async_node {
		// This `params` variant of the macro wraps the normal `fn_params` variant and is used as a shorthand for writing `T` instead of `() => T`
		($path:ty, input: $input:ty, params: [$($type:ty),*]) => {
			async_node!($path, input: $input, fn_params: [ $(() => $type),*])
		};
		($path:ty, input: $input:ty, fn_params: [$first_arg:ty => $first:ty $(, $arg:ty => $type:ty)*]) => {
			(
				ProtoNodeIdentifier::new(stringify!($path)),
				RegistryEntry {
					io: NodeIOTypes::new(concrete!($input), concrete!($first), vec![fn_type!($first_arg, $first) $(, fn_type!($arg, $type))*]),
					constructor: |inputs| {
						let expected = [stringify!($first) $(, stringify!($type))*].len();
						if inputs.len() != expected {
							return Err(ConstructionError::Arity { expected, got: inputs.len() });
						}
						let mut inputs = inputs.into_iter();
						let node = <$path>::new(inputs.next().unwrap().downcast::<$first>()? $(, inputs.next().unwrap().downcast::<$type>()?)*);
						Ok(EdgeHandle::new(std::sync::Arc::new(node) as std::sync::Arc<ErasedNode<$first>>))
					},
				},
			)
		};
	}

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

	macro_rules! lend_node {
		($type:ty) => {
			(
				ProtoNodeIdentifier::new("graphene_core::memo::LendNode"),
				RegistryEntry {
					io: NodeIOTypes::new(concrete!(Context), ref_type::<$type>(), vec![fn_type!(Context, $type)]),
					constructor: |inputs| {
						if inputs.len() != 1 {
							return Err(ConstructionError::Arity { expected: 1, got: inputs.len() });
						}
						let mut inputs = inputs.into_iter();
						let node = graphene_core::memo::LendNode::new(inputs.next().unwrap().downcast::<$type>()?);
						Ok(EdgeHandle::new_ref(std::sync::Arc::new(node) as std::sync::Arc<ErasedLendNode<$type>>))
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

	macro_rules! clone_node {
		($type:ty) => {
			(
				ProtoNodeIdentifier::new("graphene_core::debug::CloneNode"),
				RegistryEntry {
					io: NodeIOTypes::new(concrete!(Context), concrete!($type), vec![lend_edge_type::<$type>()]),
					constructor: |inputs| {
						if inputs.len() != 1 {
							return Err(ConstructionError::Arity { expected: 1, got: inputs.len() });
						}
						let mut inputs = inputs.into_iter();
						let node = graphene_core::debug::CloneNode::new(inputs.next().unwrap().downcast_lend::<$type>()?);
						Ok(EdgeHandle::new(std::sync::Arc::new(node) as std::sync::Arc<ErasedNode<$type>>))
					},
				},
			)
		};
	}

	macro_rules! frame_memo_node {
		($type:ty) => {
			(
				ProtoNodeIdentifier::new("graphene_core::memo::FrameMemoNode"),
				RegistryEntry {
					io: NodeIOTypes::new(concrete!(Context), ref_type::<$type>(), vec![fn_type!(Context, $type)]),
					constructor: |inputs| {
						if inputs.len() != 1 {
							return Err(ConstructionError::Arity { expected: 1, got: inputs.len() });
						}
						let mut inputs = inputs.into_iter();
						let node = graphene_core::memo::FrameMemoNode::new(inputs.next().unwrap().downcast::<$type>()?);
						Ok(EdgeHandle::new_ref(std::sync::Arc::new(node) as std::sync::Arc<ErasedLendNode<$type>>))
					},
				},
			)
		};
	}

	pub(crate) use async_node;
	pub(crate) use clone_node;
	pub(crate) use convert_node;
	pub(crate) use frame_memo_node;
	pub(crate) use into_node;
	pub(crate) use lend_node;
	pub(crate) use record_extract_node;
	pub(crate) use record_lift_node;
}
