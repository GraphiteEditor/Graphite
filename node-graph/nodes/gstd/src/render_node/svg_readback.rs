use core_types::runtime::Runtime;
use core_types::transform::Footprint;
use core_types::{CacheHash, Poll, RuntimeHandle, SourceId};
use futures::future::{AbortHandle, Abortable};
use graph_craft::document::value::RenderOutputType;
use rendering::SvgRenderOutput;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use wgpu_executor::WgpuExecutor;

/// The latest SVG render that needs maps and images from the GPU, kept across the render node's evaluations while a task reads them back.
/// A render of other content or for another view replaces it, cancelling the readback still underway for the one it replaced.
#[derive(Clone, Debug, Default)]
pub struct SvgReadbackSlot(Arc<Mutex<Option<SvgReadback>>>);

#[derive(Debug)]
struct SvgReadback {
	key: u64,
	finished: Option<RenderOutputType>,
	task: AbortHandle,
}

impl Drop for SvgReadback {
	fn drop(&mut self) {
		self.task.abort();
	}
}

impl SvgReadbackSlot {
	fn lock(&self) -> MutexGuard<'_, Option<SvgReadback>> {
		self.0.lock().unwrap_or_else(PoisonError::into_inner)
	}

	/// Drops the latest render, cancelling its readback if it's underway.
	pub fn clear(&self) {
		*self.lock() = None;
	}

	/// The SVG `output` with the maps and images the GPU renders for it in place of their placeholders, partial with the placeholders while a task reads them back.
	/// The task landing evaluates the render node again through its `source`.
	pub fn poll(&self, output: SvgRenderOutput, footprint: Footprint, executor: Option<WgpuExecutor>, runtime: &RuntimeHandle, source: SourceId) -> Poll<RenderOutputType> {
		let svg = output.svg;
		let image_data: Vec<_> = output.image_data.into_iter().collect();

		if output.effect_maps.is_empty() && output.deferred_images.is_empty() {
			self.clear();
			return Poll::Final(RenderOutputType::Svg { svg, image_data });
		}

		// Evaluations of one render reuse its cached intermediate SVG, and the IDs generated for it set the key apart from other renders
		let mut hasher = DefaultHasher::new();
		svg.hash(&mut hasher);
		footprint.cache_hash(&mut hasher);
		let key = hasher.finish();

		if self.lock().as_ref().is_none_or(|readback| readback.key != key) {
			let Some(executor) = executor else {
				return Poll::Final(RenderOutputType::Svg { svg, image_data });
			};
			let effect_maps = executor.render_effect_maps(&output.effect_maps, footprint);
			let deferred_images = executor.render_deferred_svg_images(&output.deferred_images, footprint);
			if effect_maps.is_empty() && deferred_images.is_empty() {
				self.clear();
				return Poll::Final(RenderOutputType::Svg { svg, image_data });
			}

			let (task, registration) = AbortHandle::new_pair();
			*self.lock() = Some(SvgReadback { key, finished: None, task });

			let (mut svg, mut image_data) = (svg.clone(), image_data.clone());
			let readback = async move {
				executor.resolve_effect_maps(&mut svg, effect_maps).await;
				executor.resolve_deferred_svg_images(&mut svg, &mut image_data, deferred_images).await;
				RenderOutputType::Svg { svg, image_data }
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
		}

		let finished = self.lock().as_ref().filter(|readback| readback.key == key).and_then(|readback| readback.finished.clone());
		match finished {
			Some(finished) => Poll::Final(finished),
			None => Poll::Partial(RenderOutputType::Svg { svg, image_data }),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::blending::BlendMode;
	use core_types::list::{Item, List};
	use core_types::runtime::{AsyncRuntime, Spawn, Task};
	use core_types::{ATTR_TRANSFORM, Color};
	use glam::{DAffine2, DVec2, UVec2};
	use graphic_types::raster_types::{Image, Raster};
	use graphic_types::{Adjustment, Effect, EffectOperation, stamp_effect};
	use rendering::{Render, RenderParams, SvgRender};

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
			blend_mode: BlendMode::default(),
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
		let poll = || slot.poll(again(&output), view(32), Some(executor.clone()), &runtime.handle(), SourceId(1));

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
		let poll = |size| slot.poll(again(&output), view(size), Some(executor.clone()), &runtime.handle(), SourceId(1));

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

		let render = slot.poll(deferred_svg(), view(32), Some(executor), &runtime.handle(), SourceId(1));
		assert!(matches!(render, Poll::Partial(_)));
		let readback = slot.lock().as_ref().unwrap().task.clone();

		// A render node that's removed drops its slot while the task reading back for it is still to run
		drop(slot);
		assert!(readback.is_aborted(), "the task shouldn't keep the readback of a removed render node going");
		tasks.take().into_iter().for_each(futures::executor::block_on);
	}
}
