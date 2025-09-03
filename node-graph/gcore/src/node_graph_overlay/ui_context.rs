use std::sync::{Arc, mpsc::Sender};

use glam::UVec2;
use graphene_core_shaders::{Ctx, context::ArcCtx};

use crate::node_graph_overlay::types::NodeGraphTransform;

pub type UIContext = Arc<UIContextImpl>;

#[derive(Debug, Clone, dyn_any::DynAny)]
pub struct UIContextImpl {
	pub transform: NodeGraphTransform,
	pub resolution: UVec2,
	pub response_sender: Sender<UIRuntimeResponse>,
}

#[derive(Debug, Clone, dyn_any::DynAny)]
pub enum UIRuntimeResponse {
	RuntimeReady,
	OverlaySVG(String),
	OverlayTexture(wgpu::Texture),
	// OverlayClickTargets(NodeId, ClickTarget)
}

impl Ctx for UIContextImpl {}
impl ArcCtx for UIContextImpl {}
