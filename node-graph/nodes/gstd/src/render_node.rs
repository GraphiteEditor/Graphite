use core_types::list::{Item, List};
use core_types::runtime::Runtime;
use core_types::transform::{Footprint, Transform};
use core_types::{CacheHash, CloneVarArgs, ExtractAll, ExtractVarArgs, Poll, RuntimeHandle, SourceId};
use core_types::{Color, Context, Ctx, ExtractFootprint, Interrupt, OwnedContextImpl, WasmNotSend};
use futures::future::{AbortHandle, Abortable};
use graph_craft::document::value::{RenderOutput, RenderOutputType};
use graphene_application_io::{ExportFormat, RenderConfig};
use graphic_types::raster_types::{CPU, Raster};
use graphic_types::{Artboard, Graphic, Vector};
use rendering::vello_scene_builder::VelloSceneBuilder;
use rendering::{CanvasImage, Render, RenderMetadata, RenderOutputType as RenderOutputTypeRequest, RenderParams, SvgRender, SvgRenderOutput};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use vector_types::Gradient;
use wgpu_executor::WgpuExecutor;

#[derive(Clone, dyn_any::DynAny)]
pub enum RenderIntermediateType {
	/// The content itself, drawn into a Vello scene at each render since the scene depends on the render's size and placement
	Vello(Arc<dyn Render + Send + Sync>),
	Svg(Arc<SvgRenderOutput>),
}
#[derive(Clone, dyn_any::DynAny)]
pub struct RenderIntermediate {
	pub(crate) ty: RenderIntermediateType,
	pub(crate) metadata: RenderMetadata,
}

#[node_macro::node(category(""))]
fn render_intermediate<T: 'static + Render + WasmNotSend + Send + Sync>(
	ctx: impl Ctx + ExtractVarArgs + ExtractAll + CloneVarArgs,
	#[implementations(
		Context -> List<Artboard>,
		Context -> List<Graphic>,
		Context -> List<Vector>,
		Context -> List<Raster<CPU>>,
		Context -> List<Color>,
		Context -> List<Gradient>,
		Context -> List<String>,
	)]
	data: impl Node<Context<'static>, Output = T>,
) -> Result<Item<RenderIntermediate>, Interrupt> {
	let render_params = ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderParams>()
		.expect("Downcasting render params yielded invalid type");

	let ctx = OwnedContextImpl::from(ctx.clone()).into_context();
	let data = data.eval(ctx)?;

	let footprint = Footprint::default();
	let mut metadata = RenderMetadata::default();
	data.collect_metadata(&mut metadata, footprint, None, None);
	let intermediate = match &render_params.render_output_type {
		RenderOutputTypeRequest::Vello => RenderIntermediate {
			ty: RenderIntermediateType::Vello(Arc::new(data)),
			metadata,
		},
		RenderOutputTypeRequest::Svg => {
			let mut render = SvgRender::new();

			data.render_svg(&mut render, render_params);

			RenderIntermediate {
				ty: RenderIntermediateType::Svg(Arc::new(render.into())),
				metadata,
			}
		}
	};

	Ok(Item::new_from_element(intermediate))
}

/// The latest SVG render that needs maps and images from the GPU, kept across the render node's evaluations while a task reads them back.
/// A render of other content or for another view replaces it, cancelling the readback still underway for the one it replaced.
#[derive(Clone, Default)]
pub struct SvgReadbackSlot(Arc<Mutex<Option<SvgReadback>>>);

impl std::fmt::Debug for SvgReadbackSlot {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("SvgReadbackSlot").finish_non_exhaustive()
	}
}

/// An SVG with its placeholders filled in, and the images it draws to canvases.
type FinishedSvg = (String, Vec<(u64, CanvasImage)>);

struct SvgReadback {
	/// A hash of the SVG with its placeholders and the view it's rendered for.
	key: u64,
	/// The SVG with the maps and images in place of their placeholders, once read back.
	finished: Option<FinishedSvg>,
	/// Cancels the readback, which ends along with the render it belongs to.
	task: AbortHandle,
}

impl Drop for SvgReadback {
	fn drop(&mut self) {
		self.task.abort();
	}
}

impl SvgReadbackSlot {
	fn lock(&self) -> MutexGuard<'_, Option<SvgReadback>> {
		// A lock poisoned by a panic elsewhere still guards a usable slot
		self.0.lock().unwrap_or_else(PoisonError::into_inner)
	}

	/// Drops the latest render, cancelling its readback if it's underway.
	fn clear(&self) {
		*self.lock() = None;
	}

	/// Whether the render `key` names has a readback, and if so, the finished SVG once it has landed.
	fn readback(&self, key: u64) -> Option<Option<FinishedSvg>> {
		self.lock().as_ref().filter(|readback| readback.key == key).map(|readback| readback.finished.clone())
	}

	/// The SVG `output` with the maps and images the GPU renders for it in place of their placeholders, partial with the placeholders while a task reads them back.
	/// The task landing evaluates the render node again through its `source`. Without a GPU, the placeholders stay and leave those effects out.
	fn poll(&self, output: SvgRenderOutput, footprint: Footprint, executor: impl FnOnce() -> Option<WgpuExecutor>, runtime: &RuntimeHandle, source: SourceId) -> Poll<RenderOutputType> {
		let image_data: Vec<_> = output.image_data.into_iter().collect();
		let unfinished = |svg, image_data| RenderOutputType::Svg { svg, image_data };

		if output.effect_maps.is_empty() && output.deferred_images.is_empty() {
			self.clear();
			return Poll::Final(unfinished(output.svg, image_data));
		}

		// Evaluations of one render reuse its cached intermediate SVG, and the IDs generated for it set the key apart from other renders
		let mut hasher = DefaultHasher::new();
		output.svg.hash(&mut hasher);
		footprint.cache_hash(&mut hasher);
		let key = hasher.finish();

		match self.readback(key) {
			Some(Some((svg, image_data))) => return Poll::Final(RenderOutputType::Svg { svg, image_data }),
			Some(None) => return Poll::Partial(unfinished(output.svg, image_data)),
			None => {}
		}

		let Some(executor) = executor() else { return Poll::Final(unfinished(output.svg, image_data)) };
		let effect_maps = executor.render_effect_maps(&output.effect_maps, footprint);
		let deferred_images = executor.render_deferred_svg_images(&output.deferred_images, footprint);
		if effect_maps.is_empty() && deferred_images.is_empty() {
			self.clear();
			return Poll::Final(unfinished(output.svg, image_data));
		}

		let (task, registration) = AbortHandle::new_pair();
		*self.lock() = Some(SvgReadback { key, finished: None, task });

		let (mut svg, mut finished_image_data) = (output.svg.clone(), image_data.clone());
		let readback = async move {
			executor.resolve_effect_maps(&mut svg, effect_maps).await;
			executor.resolve_deferred_svg_images(&mut svg, &mut finished_image_data, deferred_images).await;
			(svg, finished_image_data)
		};
		// Held weakly, the slot is dropped along with the render node, which cancels the readback
		let slot = Arc::downgrade(&self.0);
		runtime.spawn(
			source,
			Box::pin(async move {
				if let Ok(finished) = Abortable::new(readback, registration).await
					&& let Some(slot) = slot.upgrade().map(SvgReadbackSlot)
					&& let Some(readback) = slot.lock().as_mut().filter(|readback| readback.key == key)
				{
					readback.finished = Some(finished);
				}
			}),
		);

		// A spawner that runs the task right away has already finished it
		match self.readback(key).flatten() {
			Some((svg, image_data)) => Poll::Final(RenderOutputType::Svg { svg, image_data }),
			None => Poll::Partial(unfinished(output.svg, image_data)),
		}
	}
}

#[node_macro::node(category(""), version(3))]
fn render(
	ctx: impl Ctx + ExtractFootprint + ExtractVarArgs,
	data: Item<RenderIntermediate>,
	#[scope(crate::platform_application_io::wgpu_executor::IDENTIFIER)] executor: impl Node<Context<'static>, Output = Item<WgpuExecutor>>,
	#[scope("runtime")] runtime: Item<RuntimeHandle>,
	#[source_id] source: Item<SourceId>,
	#[data] svg_readback: SvgReadbackSlot,
) -> Poll<Item<RenderOutput>> {
	let footprint = ctx.footprint();
	let render_params = ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderParams>()
		.expect("Downcasting render params yielded invalid type");
	let mut render_params = render_params.clone();
	render_params.footprint = *footprint;

	let RenderIntermediate { ty, mut metadata } = data.into_element();
	metadata.apply_transform(footprint.transform);

	let data = match (render_params.render_output_type, ty) {
		(RenderOutputTypeRequest::Svg, RenderIntermediateType::Svg(data)) => {
			let logical_transform = glam::DAffine2::from_scale(glam::DVec2::splat(1.0 / render_params.scale)) * footprint.transform;
			let logical_resolution = footprint.resolution.as_dvec2() / render_params.scale;

			let mut render = SvgRender::from(data.as_ref());
			render.wrap_with_transform(logical_transform, Some(logical_resolution));

			let output = SvgRenderOutput::from(render);
			assert!(output.svg_defs.is_empty());

			let executor = || executor.eval(&Context::default()).value().map(Item::into_element);
			svg_readback.poll(output, *footprint, executor, runtime.element(), *source.element())
		}
		(RenderOutputTypeRequest::Vello, RenderIntermediateType::Vello(data)) => {
			// The latest SVG render isn't needed anymore, and a finished one holds a whole SVG export
			svg_readback.clear();

			let (executor, executor_partial, executor_error) = match executor.eval(&Context::default()) {
				Poll::Final(executor) => (executor.into_element(), false, None),
				Poll::Partial(executor) => (executor.into_element(), true, None),
				Poll::Fallback(boxed) => {
					let (executor, error) = *boxed;
					(executor.into_element(), false, Some(error))
				}
				Poll::Pending => return Poll::Pending,
				Poll::Error(error) => return Poll::Error(error),
			};

			let mut draw = |scene: &mut VelloSceneBuilder| data.render_to_vello(scene, Default::default(), &render_params);
			let texture = match executor.render_vello(footprint.resolution, footprint.transform, &mut draw) {
				Ok(texture) => texture,
				// Leave the output transparent, at a size the GPU can hold, rather than taking down the editor
				Err(error) => {
					log::error!("Failed to render Vello scene: {error}");
					let limit = glam::UVec2::splat(executor.context().device.limits().max_texture_dimension_2d);
					match executor.render_vello(footprint.resolution.min(limit), footprint.transform, &mut |_| {}) {
						Ok(texture) => texture,
						Err(error) => return Poll::error(error),
					}
				}
			};

			let texture = RenderOutputType::Texture(texture);
			match (executor_partial, executor_error) {
				(true, _) => Poll::Partial(texture),
				(false, Some(error)) => Poll::Fallback(Box::new((texture, error))),
				(false, None) => Poll::Final(texture),
			}
		}
		_ => unreachable!("Render node did not receive its requested data type"),
	};

	data.map(|data| Item::new_from_element(RenderOutput { data, metadata }))
}

#[node_macro::node(category(""))]
fn create_context(
	// The executor boundary supplies the render config as the sole vararg (see `wrap_network_in_scope()`)
	ctx: impl Ctx + ExtractAll + CloneVarArgs + Sync,
	data: impl Node<Context<'static>, Output = Item<RenderOutput>>,
) -> Result<Item<RenderOutput>, Interrupt> {
	let render_config = ctx.vararg(0).ok().and_then(|config| config.downcast_ref::<RenderConfig>()).copied().unwrap_or_else(|| {
		log::error!("The boundary context is missing its render config vararg");
		RenderConfig::default()
	});

	let render_output_type = match render_config.export_format {
		ExportFormat::Svg => RenderOutputTypeRequest::Svg,
		ExportFormat::Raster => RenderOutputTypeRequest::Vello,
	};

	let logical_viewport = render_config.viewport;
	let footprint = Footprint {
		transform: glam::DAffine2::from_scale(glam::DVec2::splat(render_config.scale)) * logical_viewport.transform,
		..logical_viewport
	};

	// Rounded up so no image is sent below its on-screen resolution, with a tolerance for float noise at exact powers of two
	let canvas_image_scale = match render_output_type {
		RenderOutputTypeRequest::Svg => (footprint.scale_magnitudes().max_element().log2() - 1e-9).ceil().exp2(),
		RenderOutputTypeRequest::Vello => 0.,
	};

	let render_params = RenderParams {
		render_mode: render_config.render_mode,
		for_export: render_config.for_export,
		render_output_type,
		scale: render_config.scale,
		viewport_zoom: Some(logical_viewport.scale_magnitudes().x),
		canvas_image_scale,
		..Default::default()
	};

	let ctx = OwnedContextImpl::default()
		.with_footprint(footprint)
		.with_real_time(render_config.time.time)
		.with_animation_time(render_config.time.animation_time.as_secs_f64())
		.with_pointer_position(render_config.pointer)
		.with_vararg(Box::new(render_params))
		.with_generations(ctx.try_generations().cloned().unwrap_or_default())
		.into_context();

	let mut result = data.eval(ctx)?;

	result.element_mut().metadata.apply_transform(glam::DAffine2::from_scale(glam::DVec2::splat(1. / render_config.scale)));
	Ok(result)
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::ATTR_TRANSFORM;
	use core_types::runtime::{AsyncRuntime, Spawn, Task};
	use glam::{DAffine2, DVec2, UVec2};
	use graphic_types::raster_types::Image;
	use graphic_types::{Adjustment, Effect, EffectOperation, stamp_effect};

	/// Holds the tasks the runtime spawns until a test runs them.
	#[derive(Default)]
	struct HeldTasks(Mutex<Vec<Task>>);

	impl Spawn for HeldTasks {
		fn spawn(&self, task: Task) {
			self.0.lock().unwrap().push(task);
		}
	}

	impl HeldTasks {
		fn take(&self) -> Vec<Task> {
			std::mem::take(&mut *self.0.lock().unwrap())
		}
	}

	/// An SVG of an orange square casting a shadow, then drained of its color by Vibrance, which no SVG filter can carry out after the shadow.
	fn deferred_svg() -> SvgRenderOutput {
		let shadow = EffectOperation::DropShadow {
			color: Color::BLACK,
			angle: 180.,
			distance: 4.,
			spread: 0.,
			size: 0.,
			contour: Default::default(),
			knockout: true,
		};
		let mut item =
			Item::new_from_element(Raster::new_cpu(Image::new(2, 2, Color::from_gamma_srgb_channels(1., 0.5, 0., 1.)))).with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::splat(10.)));
		stamp_effect(&mut item, Effect::new(&shadow));
		stamp_effect(&mut item, Effect::new(&EffectOperation::Adjustment(Adjustment::Vibrance { vibrance: 0., saturation: -100. })));

		let mut render = SvgRender::new();
		let render_params = RenderParams {
			for_export: true,
			..Default::default()
		};
		List::new_from_item(item).render_svg(&mut render, &render_params);
		SvgRenderOutput::from(render)
	}

	/// The same render's output again, as another evaluation of the render node gets it.
	fn again(output: &SvgRenderOutput) -> SvgRenderOutput {
		SvgRenderOutput {
			svg: output.svg.clone(),
			svg_defs: output.svg_defs.clone(),
			image_data: output.image_data.clone(),
			effect_maps: output.effect_maps.clone(),
			deferred_images: output.deferred_images.clone(),
		}
	}

	/// A view of the document at the identity transform, `size` pixels across.
	fn view(size: u32) -> Footprint {
		Footprint {
			transform: DAffine2::IDENTITY,
			resolution: UVec2::splat(size),
			..Default::default()
		}
	}

	#[test]
	fn an_svg_render_is_partial_until_its_readback_lands() {
		let Some(executor) = futures::executor::block_on(WgpuExecutor::new()) else { return };
		let tasks = Arc::new(HeldTasks::default());
		let runtime = AsyncRuntime::new(tasks.clone());
		let slot = SvgReadbackSlot::default();
		let output = deferred_svg();
		let poll = || slot.poll(again(&output), view(32), || Some(executor.clone()), &runtime.handle(), SourceId(1));

		// Until the readback lands, the SVG keeps its placeholders, and evaluating the same render again doesn't read it back again
		for _ in 0..2 {
			let Poll::Partial(RenderOutputType::Svg { svg, .. }) = poll() else {
				panic!("expected a partial SVG")
			};
			assert!(svg.contains("deferred-image"), "{svg}");
		}
		let spawned = tasks.take();
		assert_eq!(spawned.len(), 1, "the readback should be spawned once");

		spawned.into_iter().for_each(futures::executor::block_on);
		let Poll::Final(RenderOutputType::Svg { svg, .. }) = poll() else {
			panic!("expected a finished SVG")
		};
		assert!(svg.contains("data:image/png;base64,") && !svg.contains("deferred-image"), "{svg}");
	}

	#[test]
	fn a_render_for_another_view_cancels_the_readback_of_the_one_it_replaces() {
		let Some(executor) = futures::executor::block_on(WgpuExecutor::new()) else { return };
		let tasks = Arc::new(HeldTasks::default());
		let runtime = AsyncRuntime::new(tasks.clone());
		let slot = SvgReadbackSlot::default();
		let output = deferred_svg();
		let poll = |size| slot.poll(again(&output), view(size), || Some(executor.clone()), &runtime.handle(), SourceId(1));

		assert!(matches!(poll(32), Poll::Partial(_)));
		assert!(matches!(poll(64), Poll::Partial(_)));
		let spawned = tasks.take();
		assert_eq!(spawned.len(), 2);

		// The first view's readback, cancelled, leaves nothing finished, so that view reads back anew
		let mut spawned = spawned.into_iter();
		futures::executor::block_on(spawned.next().unwrap());
		assert!(matches!(poll(32), Poll::Partial(_)), "the cancelled readback shouldn't finish its view");
		assert_eq!(tasks.take().len(), 1, "the first view should be read back anew");

		// The second view's readback was cancelled in turn by the first view replacing it
		futures::executor::block_on(spawned.next().unwrap());
		assert!(matches!(poll(64), Poll::Partial(_)), "the second view should be read back anew too");
	}

	#[test]
	fn a_readback_underway_is_cancelled_once_its_slot_is_dropped() {
		let Some(executor) = futures::executor::block_on(WgpuExecutor::new()) else { return };
		let tasks = Arc::new(HeldTasks::default());
		let runtime = AsyncRuntime::new(tasks.clone());
		let slot = SvgReadbackSlot::default();

		let render = slot.poll(deferred_svg(), view(32), || Some(executor.clone()), &runtime.handle(), SourceId(1));
		assert!(matches!(render, Poll::Partial(_)));
		let readback = slot.lock().as_ref().unwrap().task.clone();

		// A render node that's removed drops its slot while the task reading back for it is still to run
		drop(slot);
		assert!(readback.is_aborted(), "the task shouldn't keep the readback of a removed render node going");
		tasks.take().into_iter().for_each(futures::executor::block_on);
	}
}
