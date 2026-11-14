//! Counts the instructions the `Basic Brush` node spends rendering a brush layer after one stroke is added to it, the way the Brush tool appends strokes,
//! which shows how much of the earlier render its cache saves. Rendering a layer from scratch is measured alongside for comparison.

mod brush_util;

use brush_util::{Group, VIEWPORT, callgrind, document, lavapipe, wait};
use core_types::list::{Item, List};
use core_types::transform::{Footprint, RenderQuality};
use core_types::value::ClonedNode;
use core_types::{Context, Node, OwnedContextImpl, Poll};
use glam::DAffine2;
use graphene_std::brush::basic_brush::{BasicBrushNode, BasicBrushPipelineNode};
use graphene_std::brush::{BrushCache, BrushStrokesNode};
use graphene_std::graphic::Graphic;
use graphene_std::raster_types::{GPU, Raster};
use gungraun::prelude::*;
use std::hint::black_box;
use wgpu_executor::{WgpuExecutor, WgpuPipelineCache};

const NO_CONTEXT: Context<'static> = None;

pub struct Session {
	executor: WgpuExecutor,
	context: Context<'static>,
	cache: Item<BrushCache>,
	pipeline: Item<WgpuPipelineCache>,
	/// The layer's strokes for the measured render.
	strokes: List<Graphic>,
}

impl Session {
	/// Opens lavapipe for rendering `strokes`, which it renders once through a cache it then discards,
	/// so that the device's shaders are compiled and its textures allocated before anything is measured.
	fn open(strokes: List<Graphic>) -> Self {
		let executor = lavapipe();
		let Poll::Final(pipeline) = BasicBrushPipelineNode::new(ClonedNode::new(Item::new_from_element(executor.clone()))).eval(&NO_CONTEXT) else {
			panic!("Failed to set up the brush pipeline");
		};
		let footprint = Footprint {
			transform: DAffine2::IDENTITY,
			resolution: VIEWPORT,
			quality: RenderQuality::Full,
		};
		let mut session = Self {
			executor,
			context: OwnedContextImpl::default().with_footprint(footprint).into_context(),
			cache: Item::new_from_element(BrushCache::default()),
			pipeline,
			strokes,
		};
		black_box(session.render(session.strokes.clone()));
		session.cache = Item::new_from_element(BrushCache::default());
		session
	}

	fn render(&self, strokes: List<Graphic>) -> List<Raster<GPU>> {
		let node = BasicBrushNode::new(ClonedNode::new(strokes), ClonedNode::new(self.cache.clone()), ClonedNode::new(self.pipeline.clone()));
		let Poll::Final(rasters) = node.eval(&self.context) else {
			panic!("The brush failed to render");
		};
		wait(&self.executor);
		assert!(!rasters.is_empty(), "The brush rendered nothing");
		rasters
	}
}

/// A layer of `existing` strokes rendered once, so the added stroke meets the cache the editor would have.
fn setup_add_stroke(existing: usize) -> Session {
	let (before, after) = document(existing);
	let session = Session::open(layer(&after));
	if existing > 0 {
		black_box(session.render(layer(&before)));
	}
	session
}

/// The same layer with the stroke already added, meeting an empty brush cache, as after reopening the document.
fn setup_render_layer(existing: usize) -> Session {
	let (_, after) = document(existing);
	Session::open(layer(&after))
}

/// The stack of `Brush Strokes` groups a brush layer feeds to the `Basic Brush` node.
fn layer(groups: &[Group]) -> List<Graphic> {
	groups.iter().flat_map(group).collect()
}

fn group(group: &Group) -> List<Graphic> {
	let node = BrushStrokesNode::new(
		ClonedNode::new(List::new_from_elements(group.strokes.clone())),
		ClonedNode::new(List::new_from_element(group.style.color)),
		ClonedNode::new(Item::new_from_element(group.style.diameter)),
		ClonedNode::new(Item::new_from_element(group.style.hardness)),
		ClonedNode::new(Item::new_from_element(group.style.flow)),
	);
	let Poll::Final(graphics) = node.eval(&NO_CONTEXT) else {
		panic!("The Brush Strokes node failed");
	};
	graphics
}

#[library_benchmark]
#[benches::existing_strokes(args = [0, 16, 256], setup = setup_add_stroke)]
pub fn add_stroke(session: Session) -> Session {
	measure(session)
}

#[library_benchmark]
#[benches::existing_strokes(args = [0, 16, 256], setup = setup_render_layer)]
pub fn render_layer(session: Session) -> Session {
	measure(session)
}

fn measure(session: Session) -> Session {
	let strokes = session.strokes.clone();
	callgrind::start();
	black_box(session.render(strokes));
	callgrind::stop();

	// Return the session so its teardown happens outside the measured section
	session
}

library_benchmark_group!(name = brush_render_group; benchmarks = add_stroke, render_layer);

main!(
	config = brush_util::config();
	library_benchmark_groups = brush_render_group
);
