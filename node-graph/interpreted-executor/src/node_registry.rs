use dyn_any::StaticType;
use glam::{DAffine2, DVec2, IVec2};
use graph_craft::application_io::PlatformEditorApi;
use graph_craft::application_io::resource::Resource;
use graph_craft::document::DocumentNode;
use graph_craft::document::value::RenderOutput;
use graph_craft::proto::{NodeConstructor, TypeErasedBox};
use graphene_std::any::DynAnyNode;
use graphene_std::application_io::Texture;
use graphene_std::brush::brush_stroke::BrushStroke;
use graphene_std::extract_xy::XY;
use graphene_std::gradient::Gradient;
use graphene_std::list::{AttributeValueDyn, Item, List, ListDyn, NodeIdPath};
#[cfg(target_family = "wasm")]
use graphene_std::platform_application_io::canvas_utils::CanvasHandle;
#[cfg(feature = "gpu")]
use graphene_std::raster::GPU;
use graphene_std::raster::color::Color;
use graphene_std::raster::*;
use graphene_std::raster::{CPU, Raster};
use graphene_std::render_node::RenderIntermediate;
use graphene_std::text_nodes::StringCapitalization;
use graphene_std::transform::{Footprint, ReferencePoint, ScaleType};
use graphene_std::vector::Vector;
use graphene_std::vector::misc::{BooleanOperation, BoxCorners, CentroidType, ExtrudeJoiningAlgorithm, InterpolationDistribution, MergeByDistanceAlgorithm, PointSpacingType, RowsOrColumns};
use graphene_std::vector::style::{DashPattern, GradientSpreadMethod, GradientType, PaintOrder, StrokeAlign, StrokeCap, StrokeJoin};
use graphene_std::{Artboard, Context, Graphic, NodeIO, NodeIOTypes, ProtoNodeIdentifier, concrete, fn_type_fut, future};
use node_registry_macros::{async_node, convert_node, into_node};
use std::collections::HashMap;
#[cfg(feature = "gpu")]
use wgpu_executor::WgpuExecutor;

// TODO: turn into hashmap
fn node_registry() -> HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>> {
	let mut node_types: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
		// ==========
		// INTO NODES
		// ==========
		#[cfg(feature = "gpu")]
		into_node!(from: &PlatformEditorApi, to: &WgpuExecutor),
		#[cfg(feature = "gpu")]
		into_node!(from: List<Raster<GPU>>, to: List<Raster<GPU>>),
		into_node!(from: List<Raster<CPU>>, to: List<Raster<CPU>>),
		into_node!(from: List<Graphic>, to: List<Graphic>),
		// =============
		// CONVERT NODES
		// =============
		convert_node!(from: List<Vector>, to: List<Graphic>),
		convert_node!(from: List<Raster<CPU>>, to: List<Graphic>),
		#[cfg(feature = "gpu")]
		convert_node!(from: List<Raster<GPU>>, to: List<Graphic>),
		// Type-erased attribute conversions for the `Attach Attribute` node, so it monomorphizes only over the destination `List` type.
		convert_node!(from: List<Artboard>, to: ListDyn),
		convert_node!(from: List<Graphic>, to: ListDyn),
		convert_node!(from: List<Vector>, to: ListDyn),
		convert_node!(from: List<Raster<CPU>>, to: ListDyn),
		#[cfg(feature = "gpu")]
		convert_node!(from: List<Raster<GPU>>, to: ListDyn),
		convert_node!(from: List<Color>, to: ListDyn),
		convert_node!(from: List<Gradient>, to: ListDyn),
		convert_node!(from: List<f64>, to: ListDyn),
		convert_node!(from: List<bool>, to: ListDyn),
		convert_node!(from: List<String>, to: ListDyn),
		convert_node!(from: List<u8>, to: ListDyn),
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
		convert_node!(from: Item<NodeIdPath>, to: AttributeValueDyn),
		convert_node!(from: List<Color>, to: AttributeValueDyn),
		convert_node!(from: List<Gradient>, to: AttributeValueDyn),
		convert_node!(from: List<Vector>, to: AttributeValueDyn),
		convert_node!(from: List<Raster<CPU>>, to: AttributeValueDyn),
		convert_node!(from: List<Raster<GPU>>, to: AttributeValueDyn),
		convert_node!(from: List<Graphic>, to: AttributeValueDyn),
		convert_node!(from: DVec2, to: DVec2),
		convert_node!(from: List<Vector>, to: List<Vector>),
		convert_node!(from: DVec2, to: List<Vector>),
		convert_node!(from: String, to: String),
		convert_node!(from: bool, to: String),
		convert_node!(from: DVec2, to: String),
		convert_node!(from: IVec2, to: String),
		convert_node!(from: DAffine2, to: String),
		#[cfg(feature = "gpu")]
		convert_node!(from: List<Raster<CPU>>, to: List<Raster<CPU>>, converter: &WgpuExecutor),
		#[cfg(feature = "gpu")]
		convert_node!(from: List<Raster<CPU>>, to: List<Raster<GPU>>, converter: &WgpuExecutor),
		#[cfg(feature = "gpu")]
		convert_node!(from: List<Raster<GPU>>, to: List<Raster<GPU>>, converter: &WgpuExecutor),
		#[cfg(feature = "gpu")]
		convert_node!(from: List<Raster<GPU>>, to: List<Raster<CPU>>, converter: &WgpuExecutor),
		// =============
		// MONITOR NODES
		// =============
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => ()]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<Artboard>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<Graphic>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<Vector>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<Raster<CPU>>]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<Raster<GPU>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<Color>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<Gradient>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Artboard>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Graphic>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Vector>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Raster<CPU>>]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Raster<GPU>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Color>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Gradient>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<DashPattern>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<BoxCorners>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<String>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<f64>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<f32>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<DAffine2>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Footprint>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<DVec2>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<bool>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<u32>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<u64>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<BlendMode>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Image<Color>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => String]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => IVec2]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => DVec2]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => DAffine2]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Option<DAffine2>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => bool]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => f64]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => u32]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => u64]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => BlendMode]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Texture]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::application_io::resource::Resource]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::transform::ReferencePoint]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::BooleanOperation]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::style::StrokeCap]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::style::StrokeJoin]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::style::PaintOrder]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::style::StrokeAlign]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::style::Stroke]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Box<graphene_std::vector::VectorModification>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::CentroidType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::PointSpacingType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Option<f64>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<String>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<NodeIdPath>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<f64>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<DVec2>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<u8>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<bool>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<DAffine2>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<BlendMode>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<graphene_std::vector::style::GradientType>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<graphene_std::vector::style::GradientSpreadMethod>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => AttributeValueDyn]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => ListDyn]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Graphic]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::text::Font]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<BrushStroke>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => DocumentNode]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::transform::Footprint]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::blending::BlendMode]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::LuminanceCalculation]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::extract_xy::XY]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::text_nodes::StringCapitalization]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::RedGreenBlue]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::RedGreenBlueAlpha]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::animation::RealTimeMode]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::NoiseType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::FractalType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::CellularDistanceFunction]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::CellularReturnType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::DomainWarpType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::RelativeAbsolute]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::SelectiveColorChoice]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::GridType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::ArcType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::RowsOrColumns]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::MergeByDistanceAlgorithm]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::ExtrudeJoiningAlgorithm]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::PointSpacingType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::style::GradientType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::style::GradientSpreadMethod]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::transform::ReferencePoint]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::CentroidType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::text::TextAlign]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::transform::ScaleType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::InterpolationDistribution]),
		// Context nullification
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => &PlatformEditorApi, Context => graphene_std::ContextFeatures]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => RenderIntermediate, Context => graphene_std::ContextFeatures]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => RenderOutput, Context => graphene_std::ContextFeatures]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => AttributeValueDyn, Context => graphene_std::ContextFeatures]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => ListDyn, Context => graphene_std::ContextFeatures]),
		#[cfg(target_family = "wasm")]
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => CanvasHandle, Context => graphene_std::ContextFeatures]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => &PlatformEditorApi, Context => graphene_std::ContextFeatures]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => &wgpu_executor::WgpuExecutor, Context => graphene_std::ContextFeatures]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Option<&wgpu_executor::WgpuExecutor>, Context => graphene_std::ContextFeatures]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => wgpu_executor::WgpuPipelineCache, Context => graphene_std::ContextFeatures]),
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
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<Gradient>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<String>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<NodeIdPath>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<f64>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<DVec2>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<u8>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<bool>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<DAffine2>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<BlendMode>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<graphene_std::vector::style::GradientType>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<graphene_std::vector::style::GradientSpreadMethod>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Artboard>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Graphic>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Vector>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Raster<CPU>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Raster<GPU>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Color>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Gradient>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<DashPattern>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<BoxCorners>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<String>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<f64>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<f32>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<DAffine2>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Footprint>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<DVec2>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<bool>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<u32>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<u64>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<BlendMode>]),
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
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => &PlatformEditorApi]),
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
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => graphene_std::ContextFeatures]),
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
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => &wgpu_executor::WgpuExecutor]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Option<&wgpu_executor::WgpuExecutor>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => wgpu_executor::WgpuPipelineCache]),
	];
	// The per-connector field adapter, registered per element type: a bare value wraps into an `Item`, while `Item` and `List` wires pass through unchanged
	macro_rules! field_adapter_node {
		(element: $element:ty) => {{
			let entries: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
			field_adapter_node!(from: $element, to: Item<$element>, element: $element),
			field_adapter_node!(from: Item<$element>, to: Item<$element>, element: $element),
			field_adapter_node!(from: List<$element>, to: List<$element>, element: $element),
			];
			entries
		}};
		(from: $from:ty, to: $to:ty, element: $element:ty) => {
			(
				ProtoNodeIdentifier::new(concat!["graphene_core::ops::FieldAdapterNode<", stringify!($element), ">"]),
				|mut args| {
					Box::pin(async move {
						let node = graphene_std::ops::FieldAdapterNode::new(
							graphene_std::any::downcast_node::<Context, $from>(args.pop().unwrap()),
							graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<$to>)),
						);
						let any: DynAnyNode<Context, $to, _> = graphene_std::any::DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox
					})
				},
				{
					let node = graphene_std::ops::FieldAdapterNode::new(
						graphene_std::any::PanicNode::<Context, core::pin::Pin<Box<dyn core::future::Future<Output = $from> + Send>>>::new(),
						graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<$to>)),
					);
					let params = vec![fn_type_fut!(Context, $from)];
					let node_io = NodeIO::<'_, Context>::to_async_node_io(&node, params);
					node_io
				},
			)
		};
	}
	// A conversion adapter registered under the same `FieldAdapterNode<$element>` identifier so a convertible-but-not-identical
	// ranked wire can feed an `Item<$element>` connector, converting each element via `Into`
	macro_rules! field_adapter_convert_node {
		(from_element: $from:ty, element: $element:ty) => {{
			let entries: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
				field_adapter_convert_node!(node: FieldAdapterConvertNode, from: Item<$from>, to: Item<$element>, element: $element),
				field_adapter_convert_node!(node: FieldAdapterConvertListNode, from: List<$from>, to: List<$element>, element: $element),
			];
			entries
		}};
		(node: $node:ident, from: $from:ty, to: $to:ty, element: $element:ty) => {
			(
				ProtoNodeIdentifier::new(concat!["graphene_core::ops::FieldAdapterNode<", stringify!($element), ">"]),
				|mut args| {
					Box::pin(async move {
						let node = graphene_std::ops::$node::new(
							graphene_std::any::downcast_node::<Context, $from>(args.pop().unwrap()),
							graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<$element>)),
						);
						let any: DynAnyNode<Context, $to, _> = graphene_std::any::DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox
					})
				},
				{
					let node = graphene_std::ops::$node::new(
						graphene_std::any::PanicNode::<Context, core::pin::Pin<Box<dyn core::future::Future<Output = $from> + Send>>>::new(),
						graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<$element>)),
					);
					let params = vec![fn_type_fut!(Context, $from)];
					let node_io = NodeIO::<'_, Context>::to_async_node_io(&node, params);
					node_io
				},
			)
		};
	}
	// A singleton raise adapter inserted by type resolution when an Item wire feeds a List connector
	macro_rules! item_to_list_node {
		(element: $element:ty) => {
			(
				ProtoNodeIdentifier::new(concat!["graphene_core::ops::ItemToListNode<", stringify!($element), ">"]),
				|mut args| {
					Box::pin(async move {
						let node = graphene_std::ops::FieldAdapterNode::new(
							graphene_std::any::downcast_node::<Context, Item<$element>>(args.pop().unwrap()),
							graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<List<$element>>)),
						);
						let any: DynAnyNode<Context, List<$element>, _> = graphene_std::any::DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox
					})
				},
				{
					let node = graphene_std::ops::FieldAdapterNode::new(
						graphene_std::any::PanicNode::<Context, core::pin::Pin<Box<dyn core::future::Future<Output = Item<$element>> + Send>>>::new(),
						graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<List<$element>>)),
					);
					let params = vec![fn_type_fut!(Context, Item<$element>)];
					let node_io = NodeIO::<'_, Context>::to_async_node_io(&node, params);
					node_io
				},
			)
		};
	}
	// A bare-value wrap adapter inserted by type resolution when a bare wire feeds an Item connector
	macro_rules! wrap_item_node {
		(element: $element:ty) => {
			(
				ProtoNodeIdentifier::new(concat!["graphene_core::ops::WrapItemNode<", stringify!($element), ">"]),
				|mut args| {
					Box::pin(async move {
						let node = graphene_std::ops::FieldAdapterNode::new(
							graphene_std::any::downcast_node::<Context, $element>(args.pop().unwrap()),
							graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<Item<$element>>)),
						);
						let any: DynAnyNode<Context, Item<$element>, _> = graphene_std::any::DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox
					})
				},
				{
					let node = graphene_std::ops::FieldAdapterNode::new(
						graphene_std::any::PanicNode::<Context, core::pin::Pin<Box<dyn core::future::Future<Output = $element> + Send>>>::new(),
						graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<Item<$element>>)),
					);
					let params = vec![fn_type_fut!(Context, $element)];
					let node_io = NodeIO::<'_, Context>::to_async_node_io(&node, params);
					node_io
				},
			)
		};
	}
	// A bare-value wrap-raise adapter inserted by type resolution when a bare wire feeds a List connector
	macro_rules! wrap_list_node {
		(element: $element:ty) => {
			(
				ProtoNodeIdentifier::new(concat!["graphene_core::ops::WrapListNode<", stringify!($element), ">"]),
				|mut args| {
					Box::pin(async move {
						let node = graphene_std::ops::FieldAdapterNode::new(
							graphene_std::any::downcast_node::<Context, $element>(args.pop().unwrap()),
							graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<List<$element>>)),
						);
						let any: DynAnyNode<Context, List<$element>, _> = graphene_std::any::DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox
					})
				},
				{
					let node = graphene_std::ops::FieldAdapterNode::new(
						graphene_std::any::PanicNode::<Context, core::pin::Pin<Box<dyn core::future::Future<Output = $element> + Send>>>::new(),
						graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<List<$element>>)),
					);
					let params = vec![fn_type_fut!(Context, $element)];
					let node_io = NodeIO::<'_, Context>::to_async_node_io(&node, params);
					node_io
				},
			)
		};
	}
	// A legacy unwrap adapter inserted by type resolution when an Item wire feeds a bare connector predating ranked wires
	macro_rules! unwrap_item_node {
		(element: $element:ty) => {
			(
				ProtoNodeIdentifier::new(concat!["graphene_core::ops::UnwrapItemNode<", stringify!($element), ">"]),
				|mut args| {
					Box::pin(async move {
						let node = graphene_std::ops::UnwrapItemNode::new(graphene_std::any::downcast_node::<Context, Item<$element>>(args.pop().unwrap()));
						let any: DynAnyNode<Context, $element, _> = graphene_std::any::DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox
					})
				},
				{
					let node = graphene_std::ops::UnwrapItemNode::new(graphene_std::any::PanicNode::<
						Context,
						core::pin::Pin<Box<dyn core::future::Future<Output = Item<$element>> + Send>>,
					>::new());
					let params = vec![fn_type_fut!(Context, Item<$element>)];
					let node_io = NodeIO::<'_, Context>::to_async_node_io(&node, params);
					node_io
				},
			)
		};
	}
	// ==================
	// RANK ADAPTER NODES
	// ==================
	// Registers the rank adapters (FieldAdapterNode, ItemToListNode, WrapItemNode, WrapListNode, UnwrapItemNode) for each element type
	macro_rules! rank_adapter_nodes {
		($($element:ty),* $(,)?) => {{
			let mut entries: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = Vec::new();
			$(
				entries.extend(field_adapter_node!(element: $element));
				entries.push(item_to_list_node!(element: $element));
				entries.push(wrap_item_node!(element: $element));
				entries.push(wrap_list_node!(element: $element));
				entries.push(unwrap_item_node!(element: $element));
			)*
			entries
		}};
	}
	node_types.extend(rank_adapter_nodes!(
		Vector,
		Raster<CPU>,
		Graphic,
		Color,
		Gradient,
		String,
		f64,
		f32,
		u64,
		DVec2,
		DAffine2,
		bool,
		u32,
		i32,
		i64,
		BlendMode,
		MergeByDistanceAlgorithm,
		ExtrudeJoiningAlgorithm,
		StrokeJoin,
		StrokeAlign,
		StrokeCap,
		PaintOrder,
		PointSpacingType,
		StringCapitalization,
		GradientType,
		GradientSpreadMethod,
		LuminanceCalculation,
		RedGreenBlue,
		RedGreenBlueAlpha,
		RelativeAbsolute,
		SelectiveColorChoice,
		DashPattern,
		BoxCorners,
		NodeIdPath,
		XY,
		ScaleType,
		Footprint,
		ReferencePoint,
		CentroidType,
		BooleanOperation,
		InterpolationDistribution,
		RowsOrColumns,
		Artboard,
		Resource,
	));
	// A position wire may feed a ranked vector connector, each position becoming a single-anchor vector
	node_types.extend(field_adapter_convert_node!(from_element: DVec2, element: Vector));
	// A string wire may feed the ranked `Item<DashPattern>` dash connector by parsing each element into a dash pattern
	node_types.extend(field_adapter_convert_node!(from_element: String, element: DashPattern));
	// A number wire may feed the ranked `Item<DashPattern>` dash connector, each number broadcasting element-wise as a one-length pattern
	node_types.extend(field_adapter_convert_node!(from_element: f64, element: DashPattern));
	// A string wire may feed the ranked `Item<BoxCorners>` connector by parsing each element into a set of corner values
	node_types.extend(field_adapter_convert_node!(from_element: String, element: BoxCorners));
	// A number wire may feed the ranked `Item<BoxCorners>` connector, each number becoming a uniform radius for all four corners
	node_types.extend(field_adapter_convert_node!(from_element: f64, element: BoxCorners));
	// Numeric wires cast between element types at a ranked connector, as `Convert` does for bare numeric wires
	macro_rules! field_adapter_cast_node {
		(from_element: $from:ty, element: $element:ty) => {{
			let entries: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
				field_adapter_convert_node!(node: FieldAdapterCastWrapNode, from: $from, to: Item<$element>, element: $element),
				field_adapter_convert_node!(node: FieldAdapterCastNode, from: Item<$from>, to: Item<$element>, element: $element),
				field_adapter_convert_node!(node: FieldAdapterCastListNode, from: List<$from>, to: List<$element>, element: $element),
			];
			entries
		}};
	}
	macro_rules! field_adapter_cast_star {
		(from: $from:ty, to: [$($to:ty),*]) => {{
			let mut entries: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = Vec::new();
			$(entries.extend(field_adapter_cast_node!(from_element: $from, element: $to));)*
			entries
		}};
	}
	node_types.extend(field_adapter_cast_star!(from: f64, to: [f32, u32, u64, i32, i64]));
	node_types.extend(field_adapter_cast_star!(from: f32, to: [f64, u32, u64, i32, i64]));
	node_types.extend(field_adapter_cast_star!(from: u32, to: [f64, f32, u64, i32, i64]));
	node_types.extend(field_adapter_cast_star!(from: u64, to: [f64, f32, u32, i32, i64]));
	node_types.extend(field_adapter_cast_star!(from: i32, to: [f64, f32, u32, u64, i64]));
	node_types.extend(field_adapter_cast_star!(from: i64, to: [f64, f32, u32, u64, i32]));
	// Whole-List Transform companion rows: a rank-1 content wire composes the matrix onto every item
	macro_rules! transform_list_node {
		(element: $element:ty) => {
			async_node!(
				identifier: ProtoNodeIdentifier::new("transform_nodes::transform_nodes::TransformNode"),
				graphene_std::transform_nodes::TransformListNode<_, _, _, _, _>,
				input: Context,
				fn_params: [Context => List<$element>, Context => Item<DVec2>, Context => Item<f64>, Context => Item<DVec2>, Context => Item<DVec2>]
			)
		};
	}
	let transform_list_rows: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
		transform_list_node!(element: Graphic),
		transform_list_node!(element: String),
		transform_list_node!(element: Vector),
		transform_list_node!(element: Raster<CPU>),
		transform_list_node!(element: Raster<GPU>),
		transform_list_node!(element: Color),
		transform_list_node!(element: Gradient),
	];
	node_types.extend(transform_list_rows);
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

	let mut map: HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>> = HashMap::new();

	for (id, entry) in graphene_std::registry::NODE_REGISTRY.lock().unwrap().iter() {
		for (constructor, types) in entry.iter() {
			map.entry(id.clone()).or_default().insert(types.clone(), *constructor);
		}
	}

	for (id, node_constructor, types) in node_types.into_iter() {
		// TODO: this is a hack to remove the newline from the node new_name
		// This occurs for the ChannelMixerNode presumably because of the long name.
		// This might be caused by the stringify! macro
		let mut new_name = id.as_str().replace('\n', " ");

		// Remove struct generics for all nodes except for the IntoNode and ConvertNode
		if !(new_name.contains("IntoNode")
			|| new_name.contains("ConvertNode")
			|| new_name.contains("FieldAdapterNode")
			|| new_name.contains("ItemToListNode")
			|| new_name.contains("WrapItemNode")
			|| new_name.contains("WrapListNode")
			|| new_name.contains("UnwrapItemNode"))
			&& let Some((path, _generics)) = new_name.split_once("<")
		{
			new_name = path.to_string();
		}

		map.entry(ProtoNodeIdentifier::with_owned_string(new_name)).or_default().insert(types.clone(), node_constructor);
	}

	map
}

// TODO: Replace with `core::cell::LazyCell` (<https://doc.rust-lang.org/core/cell/struct.LazyCell.html>) or similar
pub static NODE_REGISTRY: once_cell::sync::Lazy<HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>>> = once_cell::sync::Lazy::new(|| node_registry());

mod node_registry_macros {
	macro_rules! async_node {
		// TODO: we currently need to annotate the type here because the compiler would otherwise (correctly)
		// TODO: assign a Pin<Box<dyn Future<Output=T>>> type to the node, which is not what we want for now.
		//
		// This `params` variant of the macro wraps the normal `fn_params` variant and is used as a shorthand for writing `T` instead of `() => T`
		($path:ty, input: $input:ty, params: [$($type:ty),*]) => {
			async_node!($path, input: $input, fn_params: [ $(() => $type),*])
		};
		($path:ty, input: $input:ty, fn_params: [$($arg:ty => $type:ty),*]) => {
			async_node!(identifier: ProtoNodeIdentifier::new(stringify!($path)), $path, input: $input, fn_params: [$($arg => $type),*])
		};
		(identifier: $identifier:expr, $path:ty, input: $input:ty, fn_params: [$($arg:ty => $type:ty),*]) => {
			(
				$identifier,
				|mut args| {
					Box::pin(async move {
						args.reverse();
						let node = <$path>::new($(graphene_std::any::downcast_node::<$arg, $type>(args.pop().expect("Not enough arguments provided to construct node"))),*);
						let any: DynAnyNode<$input, _, _> = graphene_std::any::DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox
					})
				},
				{
					let node = <$path>::new($(
						graphene_std::any::PanicNode::<$arg, core::pin::Pin<Box<dyn core::future::Future<Output = $type> + Send>>>::new()
					),*);
					let params = vec![$(fn_type_fut!($arg, $type)),*];
					let mut node_io = NodeIO::<'_, $input>::to_async_node_io(&node, params);
					node_io.call_argument = concrete!(<$input as StaticType>::Static);
					node_io
				},
			)
		};
	}

	macro_rules! into_node {
		(from: $from:ty, to: $to:ty) => {
			(
				ProtoNodeIdentifier::new(concat!["graphene_core::ops::IntoNode<", stringify!($to), ">"]),
				|mut args| {
					Box::pin(async move {
						let node = graphene_std::ops::IntoNode::new(
							graphene_std::any::downcast_node::<Context, $from>(args.pop().unwrap()),
							graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<$to>)),
						);
						let any: DynAnyNode<Context, $to, _> = graphene_std::any::DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox
					})
				},
				{
					let node = graphene_std::ops::IntoNode::new(
						graphene_std::any::PanicNode::<Context, core::pin::Pin<Box<dyn core::future::Future<Output = $from> + Send>>>::new(),
						graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<$to>)),
					);
					let params = vec![fn_type_fut!(Context, $from)];
					let node_io = NodeIO::<'_, Context>::to_async_node_io(&node, params);
					node_io
				},
			)
		};
	}
	macro_rules! convert_node {
		(from: $from:ty, to: numbers) => {{
			let x: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
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
			let x: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
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
		(from: $from:ty, to: $to:ty, converter: $convert:ty) => {
			(
				ProtoNodeIdentifier::new(concat!["graphene_core::ops::ConvertNode<", stringify!($to), ">"]),
				|mut args| {
					Box::pin(async move {
						let mut args = args.drain(..);
						let node = graphene_std::ops::ConvertNode::new(
							graphene_std::any::downcast_node::<Context, $from>(args.next().expect("Convert node did not get first argument")),
							graphene_std::any::downcast_node::<Context, $convert>(args.next().expect("Convert node did not get converter argument")),
							graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<$to>))
						);
						let any: DynAnyNode<Context, $to, _> = graphene_std::any::DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox
					})
				},
				{
					let node = graphene_std::ops::ConvertNode::new(

						graphene_std::any::PanicNode::<Context, core::pin::Pin<Box<dyn core::future::Future<Output = $from> + Send>>>::new(),
						graphene_std::any::PanicNode::<Context, core::pin::Pin<Box<dyn core::future::Future<Output = $convert> + Send>>>::new(),
						graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<$to>))
					);
					let params = vec![fn_type_fut!(Context, $from), fn_type_fut!(Context, $convert)];
					let node_io = NodeIO::<'_, Context>::to_async_node_io(&node, params);
					node_io
				},
			)
		};
	}

	pub(crate) use async_node;
	pub(crate) use convert_node;
	pub(crate) use into_node;
}
