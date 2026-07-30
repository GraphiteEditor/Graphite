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
use graphene_std::registry::{ConstructionError, EdgeHandle, ErasedNode, NodeIOTypes, RegistryEntry};
use graphene_std::render_node::RenderIntermediate;
use graphene_std::runtime::RuntimeHandle;
use graphene_std::transform::Footprint;
use graphene_std::uuid::NodeId;
use graphene_std::vector::Vector;
use graphene_std::{Artboard, Context, Graphic, ProtoNodeIdentifier, SourceId, concrete, fn_type};
use node_registry_macros::{async_node, convert_node, into_node};
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
		// Context nullification
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => std::sync::Arc<PlatformEditorApi>, Context => graphene_std::ContextModification]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => RenderIntermediate, Context => graphene_std::ContextModification]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => RenderOutput, Context => graphene_std::ContextModification]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => AttributeDyn, Context => graphene_std::ContextModification]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => AttributeValueDyn, Context => graphene_std::ContextModification]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => ListDyn, Context => graphene_std::ContextModification]),
		#[cfg(target_family = "wasm")]
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => CanvasHandle, Context => graphene_std::ContextModification]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => std::sync::Arc<PlatformEditorApi>, Context => graphene_std::ContextModification]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => wgpu_executor::WgpuExecutorHandle, Context => graphene_std::ContextModification]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Option<wgpu_executor::WgpuExecutorHandle>, Context => graphene_std::ContextModification]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => wgpu_executor::WgpuPipelineCache, Context => graphene_std::ContextModification]),
		// ==========
		// MEMO NODES
		// ==========
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => ()]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => bool]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<Artboard>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<Graphic>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<Vector>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<Raster<CPU>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<Color>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Image<Color>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<GradientStops>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<String>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<NodeId>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<f64>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<u8>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<bool>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<DAffine2>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<BlendMode>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<graphene_std::vector::style::GradientType>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<graphene_std::vector::style::GradientSpreadMethod>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => AttributeDyn]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => AttributeValueDyn]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => ListDyn]),
		#[cfg(target_family = "wasm")]
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => CanvasHandle]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => f64]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => f32]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => u32]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => u64]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => DVec2]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => String]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => DAffine2]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Footprint]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => RenderOutput]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => std::sync::Arc<PlatformEditorApi>]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<Raster<GPU>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Option<f64>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Option<Color>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Graphic]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => glam::f32::Vec2]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => glam::f32::Affine2]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::style::Stroke]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::text::Font]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<BrushStroke>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => DocumentNode]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::ContextModification]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::transform::Footprint]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Box<graphene_std::vector::VectorModification>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::blending::BlendMode]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::LuminanceCalculation]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::QRCodeErrorCorrectionLevel]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::extract_xy::XY]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::text_nodes::StringCapitalization]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::RedGreenBlue]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::RedGreenBlueAlpha]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::animation::RealTimeMode]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::NoiseType]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::FractalType]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::CellularDistanceFunction]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::CellularReturnType]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::DomainWarpType]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::RelativeAbsolute]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::SelectiveColorChoice]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::GridType]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::ArcType]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::RowsOrColumns]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::MergeByDistanceAlgorithm]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::ExtrudeJoiningAlgorithm]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::PointSpacingType]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::style::StrokeCap]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::style::StrokeJoin]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::style::StrokeAlign]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::style::PaintOrder]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::style::GradientType]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::transform::ReferencePoint]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::CentroidType]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::BooleanOperation]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::text::TextAlign]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::transform::ScaleType]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::InterpolationDistribution]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => RenderIntermediate]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => wgpu_executor::WgpuExecutorHandle]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Option<wgpu_executor::WgpuExecutorHandle>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => wgpu_executor::WgpuPipelineCache]),
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
pub static NODE_REGISTRY: once_cell::sync::Lazy<HashMap<ProtoNodeIdentifier, Vec<RegistryEntry>>> = once_cell::sync::Lazy::new(|| node_registry());

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

	pub(crate) use async_node;
	pub(crate) use convert_node;
	pub(crate) use into_node;
}
