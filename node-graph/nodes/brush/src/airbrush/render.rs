use super::pipeline::{AirbrushPipeline, COMPOSITE_FORMAT, Field, Recorder};
use super::region::{Crop, Region};
use super::stroke::{self, StyledStroke, Walk};

use core_types::CacheHash;
use core_types::math::bbox::AxisAlignedBbox;
use glam::{DAffine2, UVec2};
use raster_types::{Texture, TextureWeakRef};
use std::hash::{Hash, Hasher};
use wgpu_executor::WgpuExecutor;

#[derive(Clone, Copy, PartialEq, Eq)]
struct StrokeKey(u64);

#[derive(Clone, Copy, PartialEq, Eq)]
struct DensityKey(u64);

#[derive(Clone, Copy, PartialEq, Eq)]
struct PrefixKey(u64);

#[derive(PartialEq, Eq)]
struct FrameKey {
	finished: Vec<StrokeKey>,
	active: StrokeKey,
}

pub(super) struct Frame<'a> {
	finished: &'a [StyledStroke],
	active: &'a StyledStroke,
}
impl<'a> Frame<'a> {
	pub(super) fn new(strokes: &'a [StyledStroke]) -> Option<Self> {
		let (active, finished) = strokes.split_last()?;
		Some(Self { finished, active })
	}
}

#[derive(Default)]
pub(super) struct State {
	finished: Finished,
	pending: Option<Pending>,
	output: Option<CachedOutput>,
}

#[derive(Default)]
struct Finished {
	strokes: Vec<Record>,
	image: Option<Placed<TextureWeakRef>>,
}

struct Record {
	key: StrokeKey,
	bounds: AxisAlignedBbox,
}

struct Placed<T> {
	texture: T,
	origin: UVec2,
}

struct CachedOutput {
	key: FrameKey,
	texture: TextureWeakRef,
}

struct Pending {
	key: PendingKey,
	walk: Walk,
	density: TextureWeakRef,
	stamp: TextureWeakRef,
}

struct LivePending {
	key: PendingKey,
	walk: Walk,
	field: Field,
}

#[derive(Clone, Copy)]
struct PendingKey {
	seed: u64,
	density: DensityKey,
	prefix: PrefixKey,
}

impl PendingKey {
	fn new(stroke: &StyledStroke, consumed: usize) -> Self {
		Self {
			seed: stroke.stroke.seed,
			density: density_key(stroke),
			prefix: prefix_key(stroke, consumed),
		}
	}

	fn matches(&self, stroke: &StyledStroke, consumed: usize) -> bool {
		self.seed == stroke.stroke.seed && self.density == density_key(stroke) && self.prefix == prefix_key(stroke, consumed)
	}
}

impl Pending {
	fn upgrade(self, region: &Region) -> Option<LivePending> {
		let density = self.density.upgrade()?;
		let stamp = self.stamp.upgrade()?;
		if density.width() != region.size.x || density.height() != region.size.y {
			return None;
		}
		Some(LivePending {
			key: self.key,
			walk: self.walk,
			field: Field { density, stamp },
		})
	}
}

impl LivePending {
	fn matches(&self, stroke: &StyledStroke) -> bool {
		self.walk.consumed > 0 && self.walk.consumed <= stroke.stroke.len() && self.key.matches(stroke, self.walk.consumed)
	}

	fn park(self) -> Pending {
		Pending {
			key: self.key,
			walk: self.walk,
			density: self.field.density.downgrade(),
			stamp: self.field.stamp.downgrade(),
		}
	}
}

pub(super) struct Rendered {
	pub(super) texture: Texture,
	pub(super) transform: DAffine2,
	pub(super) state: State,
}

pub(super) fn render(pipeline: &AirbrushPipeline, executor: &WgpuExecutor, frame: Frame<'_>, region: Region, mut state: State) -> Option<Rendered> {
	let keys: Vec<_> = frame.finished.iter().map(stroke_key).collect();
	let active_key = stroke_key(frame.active);
	let frame_key = frame_key(&keys, active_key);
	let prefix = state.finished.strokes.len() <= keys.len() && state.finished.strokes.iter().zip(&keys).all(|(cached, current)| cached.key == *current);
	let known = if prefix { state.finished.strokes.len() } else { 0 };
	let mut bounds: Vec<_> = if prefix {
		state.finished.strokes.iter().map(|record| record.bounds.clone()).collect()
	} else {
		Vec::new()
	};
	bounds.extend(frame.finished[known..].iter().map(|stroke| stroke::bounds(stroke, region.scale)));
	let active_bounds = stroke::bounds(frame.active, region.scale);
	let mut content = None;
	for bounds in &bounds {
		stroke::union(&mut content, bounds.clone());
	}
	stroke::union(&mut content, active_bounds.clone());
	let crop = Crop::new(content?, &region)?;

	if let Some(texture) = state.output.as_ref().filter(|output| output.key == frame_key).and_then(|output| output.texture.upgrade()) {
		return Some(Rendered {
			texture,
			transform: crop.transform(&region),
			state,
		});
	}

	let base = state
		.finished
		.image
		.take()
		.and_then(|placed| {
			Some(Placed {
				texture: placed.texture.upgrade()?,
				origin: placed.origin,
			})
		})
		.filter(|placed| prefix && (placed.origin + UVec2::new(placed.texture.width(), placed.texture.height())).cmple(region.size).all());
	let covered = if base.is_some() { state.finished.strokes.len() } else { 0 };
	let missing = &frame.finished[covered..];
	let pending = state.pending.take().and_then(|pending| pending.upgrade(&region));
	let (active_pending, mut finished_pending) = match pending {
		Some(pending) if pending.matches(frame.active) => (Some(pending), None),
		pending => (None, pending),
	};

	let updated = (!missing.is_empty()).then(|| executor.request_texture_with_format(crop.size, COMPOSITE_FORMAT));
	let composite = executor.request_texture_with_format(crop.size, COMPOSITE_FORMAT);
	let scratch = Field::request(executor, region.size);
	let output = executor.request_texture(crop.size);
	let mut recorder = Recorder::new(pipeline, executor, &region);

	if let Some(updated) = &updated {
		let target = updated.create_view(&wgpu::TextureViewDescriptor::default());
		recorder.clear(&target);
		if let Some(base) = &base {
			recorder.copy(&base.texture, base.origin, updated, crop.origin);
		}
		let mut strokes = StrokeRenderer {
			recorder: &mut recorder,
			executor,
			region: &region,
			crop: &crop,
			scratch: &scratch,
		};
		for (index, stroke) in missing.iter().enumerate() {
			let scissor = crop.scissor(&region, bounds[covered + index].clone());
			if !scissor.1.cmpgt(UVec2::ZERO).all() {
				continue;
			}
			let previous = if finished_pending.as_ref().is_some_and(|pending| pending.matches(stroke)) {
				finished_pending.take()
			} else {
				None
			};
			strokes.render(stroke, previous, Tail::Commit, Target { view: &target, scissor });
		}
	}

	let composite_view = composite.create_view(&wgpu::TextureViewDescriptor::default());
	match (&updated, &base) {
		(Some(updated), _) => recorder.copy_texture(updated, &composite),
		(None, Some(base)) => {
			recorder.clear(&composite_view);
			recorder.copy(&base.texture, base.origin, &composite, crop.origin);
		}
		(None, None) => recorder.clear(&composite_view),
	}
	let active_scissor = crop.scissor(&region, active_bounds);
	let pending = StrokeRenderer {
		recorder: &mut recorder,
		executor,
		region: &region,
		crop: &crop,
		scratch: &scratch,
	}
	.render(
		frame.active,
		active_pending,
		Tail::Preview,
		Target {
			view: &composite_view,
			scissor: active_scissor,
		},
	)?;

	let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());
	recorder.convert(&composite_view, &output_view);
	recorder.submit();

	let image = updated
		.map(|texture| Placed {
			texture: texture.downgrade(),
			origin: crop.origin,
		})
		.or_else(|| {
			base.map(|placed| Placed {
				texture: placed.texture.downgrade(),
				origin: placed.origin,
			})
		});
	let state = State {
		finished: Finished {
			strokes: keys.into_iter().zip(bounds).map(|(key, bounds)| Record { key, bounds }).collect(),
			image,
		},
		pending: Some(pending.park()),
		output: Some(CachedOutput {
			key: frame_key,
			texture: output.downgrade(),
		}),
	};
	Some(Rendered {
		texture: output,
		transform: crop.transform(&region),
		state,
	})
}

#[derive(Clone, Copy)]
enum Tail {
	Commit,
	Preview,
}

enum Density<'a> {
	Temporary(&'a Field),
	Owned(Field),
}

impl Density<'_> {
	fn field(&self) -> &Field {
		match self {
			Self::Temporary(field) => field,
			Self::Owned(field) => field,
		}
	}
}

struct Target<'a> {
	view: &'a wgpu::TextureView,
	scissor: (UVec2, UVec2),
}

struct StrokeRenderer<'a, 'gpu> {
	recorder: &'a mut Recorder<'gpu>,
	executor: &'gpu WgpuExecutor,
	region: &'a Region,
	crop: &'a Crop,
	scratch: &'a Field,
}

impl StrokeRenderer<'_, '_> {
	fn render(&mut self, stroke: &StyledStroke, previous: Option<LivePending>, tail: Tail, target: Target<'_>) -> Option<LivePending> {
		let (mut walk, density) = match previous {
			Some(pending) => (pending.walk, Density::Owned(pending.field)),
			None => match tail {
				Tail::Commit => (Walk::default(), Density::Temporary(self.scratch)),
				Tail::Preview => (Walk::default(), Density::Owned(Field::request(self.executor, self.region.size))),
			},
		};
		let views = density.field().views();
		if walk.consumed == 0 {
			self.recorder.clear_field(&views);
		}
		let kernel = self.recorder.kernel(stroke);
		let mut update = walk.update(stroke, self.region);
		match tail {
			Tail::Commit => {
				update.committed.append(&mut update.tail);
				self.recorder.scatter(&views, &update.committed, &kernel);
				self.recorder.resolve(stroke.color, self.crop, &views, target.view, target.scissor);
				if let Density::Owned(field) = density {
					self.recorder.keep(field.density);
					self.recorder.keep(field.stamp);
				}
				None
			}
			Tail::Preview => {
				self.recorder.scatter(&views, &update.committed, &kernel);
				if update.tail.is_empty() {
					self.recorder.resolve(stroke.color, self.crop, &views, target.view, target.scissor);
				} else {
					let field = density.field();
					self.recorder.copy_texture(&field.density, &self.scratch.density);
					self.recorder.copy_texture(&field.stamp, &self.scratch.stamp);
					let scratch_views = self.scratch.views();
					self.recorder.scatter(&scratch_views, &update.tail, &kernel);
					self.recorder.resolve(stroke.color, self.crop, &scratch_views, target.view, target.scissor);
				}
				let Density::Owned(field) = density else { unreachable!() };
				Some(LivePending {
					key: PendingKey::new(stroke, walk.consumed),
					walk,
					field,
				})
			}
		}
	}
}

fn stroke_key(stroke: &StyledStroke) -> StrokeKey {
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	stroke.stroke.cache_hash(&mut hasher);
	stroke.color.cache_hash(&mut hasher);
	stroke.diameter.cache_hash(&mut hasher);
	stroke.hardness.cache_hash(&mut hasher);
	stroke.flow.cache_hash(&mut hasher);
	StrokeKey(hasher.finish())
}

fn density_key(stroke: &StyledStroke) -> DensityKey {
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	(stroke.diameter.max(0.) as f32).to_bits().hash(&mut hasher);
	(stroke.hardness.clamp(0., 1.) as f32).to_bits().hash(&mut hasher);
	(stroke.flow.clamp(0., 1.) as f32).to_bits().hash(&mut hasher);
	DensityKey(hasher.finish())
}

fn prefix_key(stroke: &StyledStroke, consumed: usize) -> PrefixKey {
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	for sample in stroke.stroke.samples().take(consumed) {
		sample.position.x.to_bits().hash(&mut hasher);
		sample.position.y.to_bits().hash(&mut hasher);
		sample.pressure.clamp(0., 1.).to_bits().hash(&mut hasher);
	}
	PrefixKey(hasher.finish())
}

fn frame_key(finished: &[StrokeKey], active: StrokeKey) -> FrameKey {
	FrameKey { finished: finished.to_vec(), active }
}

#[cfg(test)]
mod tests {
	use super::*;
	use brush_types::{Channel, Stroke};
	use core_types::Color;
	use glam::DVec2;

	fn stroke() -> StyledStroke {
		StyledStroke {
			color: Color::BLACK,
			diameter: 20.,
			hardness: 0.8,
			flow: 1.,
			stroke: Stroke {
				position: vec![DVec2::new(1., 2.), DVec2::new(3., 4.), DVec2::new(5., 6.)],
				pressure: Channel::Samples(vec![0.2, 0.4, 0.6]),
				seed: 42,
				..Default::default()
			},
		}
	}

	#[test]
	fn pending_key_accepts_an_appended_stroke() {
		let original = stroke();
		let key = PendingKey::new(&original, original.stroke.len());
		let mut appended = stroke();
		appended.stroke.position.push(DVec2::new(7., 8.));
		let Channel::Samples(pressure) = &mut appended.stroke.pressure else { unreachable!() };
		pressure.push(0.8);
		assert!(key.matches(&appended, original.stroke.len()));
	}

	#[test]
	fn pending_key_rejects_changed_render_data() {
		let original = stroke();
		let key = PendingKey::new(&original, original.stroke.len());

		let mut position = stroke();
		position.stroke.position[0].x += 1.;
		assert!(!key.matches(&position, original.stroke.len()));

		let mut pressure = stroke();
		let Channel::Samples(samples) = &mut pressure.stroke.pressure else { unreachable!() };
		samples[1] += 0.1;
		assert!(!key.matches(&pressure, original.stroke.len()));

		let mut flow = stroke();
		flow.flow *= 0.5;
		assert!(!key.matches(&flow, original.stroke.len()));
	}

	#[test]
	fn pending_key_ignores_color() {
		let original = stroke();
		let key = PendingKey::new(&original, original.stroke.len());
		let mut recolored = stroke();
		recolored.color = Color::WHITE;
		assert!(key.matches(&recolored, original.stroke.len()));
	}
}
