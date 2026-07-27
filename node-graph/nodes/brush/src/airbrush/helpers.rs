use super::region::{Crop, Region};
use super::{CUTOFF_SIGMA, MAX_EDGE_SHIFT, MIN_SIGMA};
use brush_types::{BrushCache, BrushStyle, Sample, Stroke};
use bytemuck::{Pod, Zeroable};
use core_types::math::bbox::AxisAlignedBbox;
use core_types::transform::Footprint;
use core_types::{CacheHash, Color};
use glam::{DAffine2, DVec2, UVec2};
use raster_types::{Texture, WeakTexture};
use std::hash::Hasher;
use wgpu_executor::{AsyncWgpuPipeline, Buffer, WgpuExecutor};

/// One segment of a stroke's centerline, the per-instance vertex data of the scatter pass.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Edge {
	a: [f32; 2],
	b: [f32; 2],
	sigma: f32,
	weight: f32,
}

/// Mirrors `Uniforms` in `scatter.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ScatterUniforms {
	frame_size: [f32; 2],
}

/// Mirrors `Uniforms` in `resolve.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ResolveUniforms {
	color: [f32; 4],
	/// Texel offset of the crop-sized resolve target within the frame-sized density texture.
	density_offset: [f32; 2],
	_pad: [f32; 2],
}

fn scatter_uniform(executor: &WgpuExecutor, region: &Region) -> Buffer {
	let uniforms = ScatterUniforms {
		frame_size: [region.size.x as f32, region.size.y as f32],
	};
	executor.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("airbrush_scatter_uniform"),
		contents: bytemuck::bytes_of(&uniforms),
		usage: wgpu::BufferUsages::UNIFORM,
	})
}

fn resolve_uniform(executor: &WgpuExecutor, color: Color, crop: &Crop) -> Buffer {
	let uniforms = ResolveUniforms {
		// Linear-light channels; the composite stays linear until the convert pass encodes it.
		color: [color.r(), color.g(), color.b(), color.a()],
		density_offset: [crop.origin.x as f32, crop.origin.y as f32],
		_pad: [0.; 2],
	};
	executor.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("airbrush_resolve_uniform"),
		contents: bytemuck::bytes_of(&uniforms),
		usage: wgpu::BufferUsages::UNIFORM,
	})
}

/// Three passes, one shader file each: scatter blends every stroke segment's swept-Gaussian
/// density additively into an R16Float texture, resolve maps accumulated density to
/// premultiplied color over the previous strokes, and convert encodes the composite as
/// straight-alpha sRGB.
pub struct AirbrushPipeline {
	density_pipeline: wgpu::RenderPipeline,
	resolve_pipeline: wgpu::RenderPipeline,
	convert_pipeline: wgpu::RenderPipeline,
	density_bind_group_layout: wgpu::BindGroupLayout,
	resolve_bind_group_layout: wgpu::BindGroupLayout,
	convert_bind_group_layout: wgpu::BindGroupLayout,
}

/// Premultiplied linear-light, so "over" between groups of different colors is correct; the
/// convert pass encodes to the app-wide straight-alpha sRGB convention once per output.
const COMPOSITE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The identity of one finished stroke: a hash of its content and style, so any in-place edit
/// misses.
fn stroke_key(stroke: &Stroke, style: &BrushStyle) -> u64 {
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	stroke.cache_hash(&mut hasher);
	style.cache_hash(&mut hasher);
	hasher.finish()
}

/// What the airbrush parks in the [`BrushCache`] per footprint; validity (stroke prefix,
/// watermark, texture sizes) is checked on take.
#[derive(Default)]
struct AirbrushSlot {
	/// The finished strokes `base` shows, in composite order, each with its pad bounds.
	strokes: Vec<(u64, AxisAlignedBbox)>,
	/// Composite of `strokes`, crop-sized, with its texel origin on the region lattice; parked
	/// weakly, so a failed upgrade rebakes.
	base: Option<(WeakTexture, UVec2)>,
	/// The in-progress stroke's continuation state.
	active: Option<ActiveState>,
}

/// The active stroke's committed kept-chain density plus the decimation-walk watermark: while
/// the stroke grows only its new segments are scattered in, and on pen-up the density is
/// completed and resolved over the base in one pass instead of re-scattering the whole stroke.
struct ActiveState {
	seed: u64,
	/// Hash of the style the density was scattered with; a style edit stales the sidecar.
	style: u64,
	walk: Walk,
	/// Position of the last consumed sample, verifying the stored prefix is intact.
	last_position: DVec2,
	/// Frame-sized R16Float, parked weakly; the provisional tail is never committed here while
	/// growing.
	density: WeakTexture,
}

impl ActiveState {
	/// A watermark check rather than a content hash: pen-up often coalesces a few final samples
	/// into the finished stroke, which keeps the prefix valid but would change its hash.
	fn is_prefix_of(&self, stroke: &Stroke, style: u64) -> bool {
		self.seed == stroke.seed && self.style == style && self.walk.consumed > 0 && self.walk.consumed <= stroke.len() && stroke.position[self.walk.consumed - 1] == self.last_position
	}
}

fn style_hash(style: &BrushStyle) -> u64 {
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	style.cache_hash(&mut hasher);
	hasher.finish()
}

/// A sample resolved for rendering, in document units.
#[derive(Clone, Copy)]
struct Dab {
	position: DVec2,
	sigma: f32,
	weight: f32,
}

fn dab(sample: &Sample, style: &BrushStyle) -> Dab {
	let pressure = sample.pressure.clamp(0., 1.);
	let hardness = style.hardness.clamp(0., 1.) as f32;
	Dab {
		position: sample.position,
		sigma: style.diameter.max(0.) as f32 * pressure / 4.,
		// ~17x buildup at the default hardness saturates the core to near-solid.
		weight: style.flow.clamp(0., 1.) as f32 * (1. + hardness * hardness * 24.) * pressure,
	}
}

/// The document-space square a dab's scatter quad can reach. Sigma clamps like the shader's
/// ([`MIN_SIGMA`] texels), otherwise the pad undershoots strokes thinner than a texel.
fn dab_pad(dab: Dab, scale: f64) -> AxisAlignedBbox {
	let sigma = (dab.sigma as f64).max(MIN_SIGMA as f64 / scale);
	let pad = DVec2::splat(CUTOFF_SIGMA as f64 * sigma + 1f64.max(1. / scale));
	AxisAlignedBbox {
		start: dab.position - pad,
		end: dab.position + pad,
	}
}

fn union(bounds: &mut Option<AxisAlignedBbox>, other: AxisAlignedBbox) {
	*bounds = Some(match bounds.take() {
		Some(existing) => existing.union(&other),
		None => other,
	});
}

/// Union of every raw sample's pad — the stroke's renderable bounds. Decimation-free, so it is a
/// pure function of the stroke and style and cache hits and misses derive the same crop.
fn stroke_pad_bounds(stroke: &Stroke, style: &BrushStyle, scale: f64) -> AxisAlignedBbox {
	let mut bounds = None;
	for index in 0..stroke.len() {
		let sample = stroke.sample(index);
		union(&mut bounds, dab_pad(dab(&sample, style), scale));
	}
	bounds.unwrap_or(AxisAlignedBbox::ZERO)
}

/// A stroke's decimation-walk state. The walk is causal — each decision depends only on earlier
/// samples — so advancing over newly appended samples never re-decides the kept prefix.
#[derive(Clone)]
struct Walk {
	/// Narrowest sigma seen so far; sets the decimation step.
	sigma_min: f32,
	kept_last: Option<Dab>,
	kept: usize,
	/// Raw samples consumed so far.
	consumed: usize,
}

impl Default for Walk {
	fn default() -> Self {
		Self {
			sigma_min: f32::MAX,
			kept_last: None,
			kept: 0,
			consumed: 0,
		}
	}
}

impl Walk {
	/// Decimates the centerline — sub-sigma detail is invisible, and fewer samples means fewer
	/// scatter quads. Keeps any sample at least half the narrowest sigma from the last kept one.
	fn advance(&mut self, stroke: &Stroke, style: &BrushStyle, scale: f64) -> Vec<Dab> {
		let mut kept = Vec::new();
		for index in self.consumed..stroke.len() {
			let sample = stroke.sample(index);
			let dab = dab(&sample, style);
			self.sigma_min = self.sigma_min.min(dab.sigma);
			let min_step = (self.sigma_min as f64 * 0.5).max(0.5 / scale);
			if self.kept_last.is_none_or(|last| last.position.distance(dab.position) >= min_step) {
				kept.push(dab);
				self.kept_last = Some(dab);
				self.kept += 1;
			}
		}
		self.consumed = stroke.len();
		kept
	}

	/// The provisional closing segment from the last kept dab to the stroke's final sample, so
	/// the stroke ends where the user lifted. Never committed to the kept chain — the next
	/// advance may supersede it. A single-sample stroke closes with the zero-length lone-dab
	/// segment; `None` when the kept chain already ends at the final sample.
	fn tail(&self, stroke: &Stroke, style: &BrushStyle) -> Option<(Dab, Dab)> {
		let kept_last = self.kept_last?;
		let sample = stroke.sample(stroke.len() - 1);
		let dab = dab(&sample, style);
		if dab.position == kept_last.position {
			return (self.kept == 1).then_some((kept_last, kept_last));
		}
		Some((kept_last, dab))
	}
}

fn texel(region: &Region, p: DVec2) -> [f32; 2] {
	[((p.x - region.min.x) * region.scale) as f32, ((p.y - region.min.y) * region.scale) as f32]
}

/// Sigma and weight interpolate as endpoint averages over the segment; the shader renders a
/// zero-length edge as a lone Gaussian dab.
fn edge(region: &Region, a: Dab, b: Dab) -> Edge {
	Edge {
		a: texel(region, a.position),
		b: texel(region, b.position),
		sigma: ((a.sigma + b.sigma) / 2. * region.scale as f32).max(MIN_SIGMA),
		weight: (a.weight + b.weight) / 2.,
	}
}

fn mix(a: Dab, b: Dab, t: f32) -> Dab {
	Dab {
		position: a.position.lerp(b.position, t as f64),
		sigma: a.sigma + (b.sigma - a.sigma) * t,
		weight: a.weight + (b.weight - a.weight) * t,
	}
}

/// An edge renders with constant sigma and weight, so a sigma step shifts the stroke's visible
/// boundary — which sits up to [`CUTOFF_SIGMA`] sigmas out — while the edge gradient it hides in
/// is only as wide as the local sigma (floored at one texel of raster antialiasing). A segment
/// splits until each piece shifts the boundary by at most [`MAX_EDGE_SHIFT`] of that gradient:
/// thin crisp strokes subdivide finely, soft wide ones stay coarse. The piece cap bounds the
/// quads' end-cap overdraw at extreme zoom.
fn segment_edges(edges: &mut Vec<Edge>, region: &Region, a: Dab, b: Dab) {
	let scale = region.scale as f32;
	let gradient = (a.sigma.min(b.sigma) * scale).max(1.);
	let shift = (b.sigma - a.sigma).abs() * scale * CUTOFF_SIGMA;
	let pieces = (shift / (MAX_EDGE_SHIFT * gradient)).ceil().clamp(1., 64.) as usize;
	let mut previous = a;
	for piece in 1..=pieces {
		let next = if piece == pieces { b } else { mix(a, b, piece as f32 / pieces as f32) };
		edges.push(edge(region, previous, next));
		previous = next;
	}
}

/// The edges linking `prev` (when present), the `kept` dabs, and the closing `tail` in sequence.
fn edges(region: &Region, prev: Option<Dab>, kept: &[Dab], tail: Option<(Dab, Dab)>) -> Vec<Edge> {
	let mut edges = Vec::with_capacity(kept.len() + 1);
	let mut last = prev;
	for &dab in kept {
		if let Some(previous) = last {
			segment_edges(&mut edges, region, previous, dab);
		}
		last = Some(dab);
	}
	if let Some((a, b)) = tail {
		segment_edges(&mut edges, region, a, b);
	}
	edges
}

/// Texel-space (origin, size) of `bounds` within the crop window.
fn scissor(region: &Region, crop: &Crop, bounds: AxisAlignedBbox) -> (UVec2, UVec2) {
	let clamp = |texels: UVec2| texels.max(crop.origin).min(crop.origin + crop.size) - crop.origin;
	let min = ((bounds.start - region.min) * region.scale).floor().max(DVec2::ZERO).as_uvec2().min(region.size);
	let max = ((bounds.end - region.min) * region.scale).ceil().max(DVec2::ZERO).as_uvec2().min(region.size);
	(clamp(min), clamp(max) - clamp(min))
}

/// Walks a whole stroke, yielding its kept dabs and provisional tail.
fn walk_stroke(stroke: &Stroke, style: &BrushStyle, scale: f64) -> (Vec<Dab>, Option<(Dab, Dab)>) {
	let mut walk = Walk::default();
	let kept = walk.advance(stroke, style, scale);
	let tail = walk.tail(stroke, style);
	(kept, tail)
}

pub struct AirbrushPipelineArgs<'a> {
	pub(super) footprint: Footprint,
	/// Every stroke with its style, in composite order; the last is the actively drawn one.
	pub(super) strokes: &'a [(BrushStyle, Stroke)],
	pub(super) cache: &'a BrushCache,
}

impl AsyncWgpuPipeline for AirbrushPipeline {
	type Args<'a> = AirbrushPipelineArgs<'a>;
	type Out = Option<(Texture, DAffine2)>;

	fn create(executor: &WgpuExecutor) -> Self {
		let device = &executor.context().device;
		let scatter_shader = device.create_shader_module(wgpu::include_wgsl!("scatter.wgsl"));
		let resolve_shader = device.create_shader_module(wgpu::include_wgsl!("resolve.wgsl"));
		let convert_shader = device.create_shader_module(wgpu::include_wgsl!("convert.wgsl"));

		let uniform_entry = |binding, visibility| wgpu::BindGroupLayoutEntry {
			binding,
			visibility,
			ty: wgpu::BindingType::Buffer {
				ty: wgpu::BufferBindingType::Uniform,
				has_dynamic_offset: false,
				min_binding_size: None,
			},
			count: None,
		};
		let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
			binding,
			visibility: wgpu::ShaderStages::FRAGMENT,
			ty: wgpu::BindingType::Texture {
				sample_type: wgpu::TextureSampleType::Float { filterable: false },
				view_dimension: wgpu::TextureViewDimension::D2,
				multisampled: false,
			},
			count: None,
		};

		let density_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("airbrush_density_bind_group_layout"),
			entries: &[uniform_entry(0, wgpu::ShaderStages::VERTEX)],
		});
		let resolve_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("airbrush_resolve_bind_group_layout"),
			entries: &[uniform_entry(0, wgpu::ShaderStages::FRAGMENT), texture_entry(1)],
		});
		let convert_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("airbrush_convert_bind_group_layout"),
			entries: &[texture_entry(0)],
		});

		let density_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("airbrush_density_pipeline_layout"),
			bind_group_layouts: &[Some(&density_bind_group_layout)],
			immediate_size: 0,
		});
		let resolve_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("airbrush_resolve_pipeline_layout"),
			bind_group_layouts: &[Some(&resolve_bind_group_layout)],
			immediate_size: 0,
		});
		let convert_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("airbrush_convert_pipeline_layout"),
			bind_group_layouts: &[Some(&convert_bind_group_layout)],
			immediate_size: 0,
		});

		let instance_layout = wgpu::VertexBufferLayout {
			array_stride: std::mem::size_of::<Edge>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Instance,
			attributes: &[
				wgpu::VertexAttribute {
					offset: 0,
					shader_location: 0,
					format: wgpu::VertexFormat::Float32x2,
				},
				wgpu::VertexAttribute {
					offset: 8,
					shader_location: 1,
					format: wgpu::VertexFormat::Float32x2,
				},
				wgpu::VertexAttribute {
					offset: 16,
					shader_location: 2,
					format: wgpu::VertexFormat::Float32,
				},
				wgpu::VertexAttribute {
					offset: 20,
					shader_location: 3,
					format: wgpu::VertexFormat::Float32,
				},
			],
		};
		let additive = wgpu::BlendComponent {
			src_factor: wgpu::BlendFactor::One,
			dst_factor: wgpu::BlendFactor::One,
			operation: wgpu::BlendOperation::Add,
		};
		let scatter_options = wgpu::PipelineCompilationOptions {
			constants: &[("CUTOFF_SIGMA", CUTOFF_SIGMA as f64)],
			..Default::default()
		};
		let density_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("airbrush_density_pipeline"),
			layout: Some(&density_pipeline_layout),
			vertex: wgpu::VertexState {
				module: &scatter_shader,
				entry_point: Some("vs_main"),
				compilation_options: scatter_options.clone(),
				buffers: &[instance_layout],
			},
			fragment: Some(wgpu::FragmentState {
				module: &scatter_shader,
				entry_point: Some("fs_main"),
				compilation_options: scatter_options,
				targets: &[Some(wgpu::ColorTargetState {
					format: wgpu::TextureFormat::R16Float,
					blend: Some(wgpu::BlendState { color: additive, alpha: additive }),
					write_mask: wgpu::ColorWrites::ALL,
				})],
			}),
			primitive: wgpu::PrimitiveState {
				topology: wgpu::PrimitiveTopology::TriangleStrip,
				..Default::default()
			},
			depth_stencil: None,
			multisample: wgpu::MultisampleState::default(),
			multiview_mask: None,
			cache: None,
		});

		let resolve_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("airbrush_resolve_pipeline"),
			layout: Some(&resolve_pipeline_layout),
			vertex: wgpu::VertexState {
				module: &resolve_shader,
				entry_point: Some("vs_main"),
				compilation_options: Default::default(),
				buffers: &[],
			},
			fragment: Some(wgpu::FragmentState {
				module: &resolve_shader,
				entry_point: Some("fs_main"),
				compilation_options: Default::default(),
				targets: &[Some(wgpu::ColorTargetState {
					format: COMPOSITE_FORMAT,
					blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
					write_mask: wgpu::ColorWrites::ALL,
				})],
			}),
			primitive: wgpu::PrimitiveState {
				topology: wgpu::PrimitiveTopology::TriangleList,
				..Default::default()
			},
			depth_stencil: None,
			multisample: wgpu::MultisampleState::default(),
			multiview_mask: None,
			cache: None,
		});

		let convert_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("airbrush_convert_pipeline"),
			layout: Some(&convert_pipeline_layout),
			vertex: wgpu::VertexState {
				module: &convert_shader,
				entry_point: Some("vs_main"),
				compilation_options: Default::default(),
				buffers: &[],
			},
			fragment: Some(wgpu::FragmentState {
				module: &convert_shader,
				entry_point: Some("fs_main"),
				compilation_options: Default::default(),
				targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
			}),
			primitive: wgpu::PrimitiveState {
				topology: wgpu::PrimitiveTopology::TriangleList,
				..Default::default()
			},
			depth_stencil: None,
			multisample: wgpu::MultisampleState::default(),
			multiview_mask: None,
			cache: None,
		});

		Self {
			density_pipeline,
			resolve_pipeline,
			convert_pipeline,
			density_bind_group_layout,
			resolve_bind_group_layout,
			convert_bind_group_layout,
		}
	}

	/// Take this footprint's slot, derive the content crop, bake missing finished strokes over
	/// the placed base, render the active stroke — continuing its committed density when the
	/// sidecar still describes a prefix of it — and store the slot back. The cache is
	/// unobservable: every path produces identical pixels.
	async fn run<'a>(&'a self, executor: &'a WgpuExecutor, args: &'a Self::Args<'_>) -> Self::Out {
		let (last, finished) = args.strokes.split_last()?;
		let (active_style, active) = (&last.0, &last.1);
		let active_style_hash = style_hash(active_style);
		// The frame covers the full footprint: density textures span it, so the active stroke
		// can wander anywhere visible without reallocation.
		let region = Region::new(&args.footprint)?;
		let scale = region.scale;
		let device = &executor.context().device;
		let queue = &executor.context().queue;

		let keys: Vec<u64> = finished.iter().map(|(style, stroke)| stroke_key(stroke, style)).collect();
		let frame_sized = |texture: &wgpu::Texture| texture.width() == region.size.x && texture.height() == region.size.y;
		let slot = args.cache.take::<AirbrushSlot>(&args.footprint).unwrap_or_default();
		let prefix = slot.strokes.len() <= keys.len() && slot.strokes.iter().zip(&keys).all(|((key, _), new)| key == new);
		let known = if prefix { slot.strokes.len() } else { 0 };
		let mut bounds: Vec<AxisAlignedBbox> = if prefix { slot.strokes.iter().map(|(_, bounds)| bounds.clone()).collect() } else { Vec::new() };
		bounds.extend(finished[known..].iter().map(|(style, stroke)| stroke_pad_bounds(stroke, style, scale)));
		let active_bounds = stroke_pad_bounds(active, active_style, scale);
		let mut content = None;
		for stroke_bounds in &bounds {
			union(&mut content, stroke_bounds.clone());
		}
		union(&mut content, active_bounds.clone());
		// Base, bake, composite, and output are all crop-sized; no visible content, nothing to emit.
		let crop = Crop::new(content?, &region)?;
		// The base covers wherever its strokes had content, so any relation to the crop is fine.
		let base = slot
			.base
			.and_then(|(texture, origin)| Some((texture.upgrade()?, origin)))
			.filter(|(texture, origin)| prefix && (*origin + UVec2::new(texture.width(), texture.height())).cmple(region.size).all());
		let covered = match &base {
			Some(_) => slot.strokes.len(),
			None => 0,
		};
		let missing = &finished[covered..];
		let mut sidecar = slot
			.active
			.and_then(|state| state.density.upgrade().filter(|density| frame_sized(density)).map(|density| (density, state)));
		// The sidecar either continues the active stroke, or finishes the stroke it belonged to
		// during the bake (the pen-up event), or is stale.
		let continuation = sidecar.as_ref().is_some_and(|(_, state)| state.is_prefix_of(active, active_style_hash));

		let bake = (!missing.is_empty()).then(|| executor.request_texture_with_format(crop.size, COMPOSITE_FORMAT));
		let composite = executor.request_texture_with_format(crop.size, COMPOSITE_FORMAT);
		let scratch = executor.request_texture_with_format(region.size, wgpu::TextureFormat::R16Float);
		let output = executor.request_texture(crop.size);

		let scatter_globals = scatter_uniform(executor, &region);
		let scatter_bind = self.scatter_bind(device, &scatter_globals);
		let active_uniform = resolve_uniform(executor, active_style.color, &crop);
		let scratch_view = scratch.create_view(&wgpu::TextureViewDescriptor::default());
		let scratch_bind = self.resolve_bind(device, &active_uniform, &scratch_view);
		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("airbrush_encoder") });
		// Everything the encoder references must outlive the submit below, or destroy-on-drop
		// would invalidate it; per-pass buffers collect here, as does the sidecar density the
		// pen-up fast path consumes mid-encode.
		let mut in_flight: Vec<Buffer> = Vec::new();
		let mut spent: Vec<Texture> = Vec::new();

		// Stored base textures are never mutated, so an in-flight reader of the old slot stays valid.
		if let Some(bake) = &bake {
			let bake_view = bake.create_view(&wgpu::TextureViewDescriptor::default());
			clear(&mut encoder, &bake_view);
			if let Some((texture, origin)) = &base {
				copy_placed(&mut encoder, texture, *origin, bake, crop.origin);
			}
			for (index, (style, stroke)) in missing.iter().enumerate() {
				let stroke_scissor = scissor(&region, &crop, bounds[covered + index].clone());
				if !stroke_scissor.1.cmpgt(UVec2::ZERO).all() {
					continue;
				}
				let stroke_uniform = resolve_uniform(executor, style.color, &crop);
				// Pen-up fast path: the stroke that was active last event completes its committed
				// density (remaining segments plus the final tail) and resolves in one pass.
				if !continuation && sidecar.as_ref().is_some_and(|(_, state)| state.is_prefix_of(stroke, style_hash(style))) {
					let (density, mut state) = sidecar.take().unwrap();
					let prev = state.walk.kept_last;
					let kept = state.walk.advance(stroke, style, scale);
					let tail = state.walk.tail(stroke, style);
					let density_view = density.create_view(&wgpu::TextureViewDescriptor::default());
					in_flight.extend(self.scatter_pass(executor, &mut encoder, &density_view, &scatter_bind, &edges(&region, prev, &kept, tail)));
					self.resolve_pass(&mut encoder, &bake_view, &self.resolve_bind(device, &stroke_uniform, &density_view), stroke_scissor);
					in_flight.push(stroke_uniform);
					spent.push(density);
					continue;
				}
				let (kept, tail) = walk_stroke(stroke, style, scale);
				clear(&mut encoder, &scratch_view);
				in_flight.extend(self.scatter_pass(executor, &mut encoder, &scratch_view, &scatter_bind, &edges(&region, None, &kept, tail)));
				self.resolve_pass(&mut encoder, &bake_view, &self.resolve_bind(device, &stroke_uniform, &scratch_view), stroke_scissor);
				in_flight.push(stroke_uniform);
			}
		}

		let (mut walk, density) = match sidecar.take() {
			Some((density, state)) if continuation => (state.walk, density),
			_ => (Walk::default(), executor.request_texture_with_format(region.size, wgpu::TextureFormat::R16Float)),
		};
		let density_view = density.create_view(&wgpu::TextureViewDescriptor::default());
		if walk.consumed == 0 {
			clear(&mut encoder, &density_view);
		}
		let prev = walk.kept_last;
		let kept = walk.advance(active, active_style, scale);
		let tail = walk.tail(active, active_style);
		in_flight.extend(self.scatter_pass(executor, &mut encoder, &density_view, &scatter_bind, &edges(&region, prev, &kept, None)));

		let composite_view = composite.create_view(&wgpu::TextureViewDescriptor::default());
		match (&bake, &base) {
			(Some(baked), _) => encoder.copy_texture_to_texture(baked.as_image_copy(), composite.as_image_copy(), baked.size()),

			(None, Some((texture, origin))) => {
				clear(&mut encoder, &composite_view);
				copy_placed(&mut encoder, texture, *origin, &composite, crop.origin);
			}
			(None, None) => clear(&mut encoder, &composite_view),
		}
		let density_bind = self.resolve_bind(device, &active_uniform, &density_view);
		// The provisional tail is staged on a scratch copy, never committed to the density.
		let active_bind = match tail {
			Some((a, b)) => {
				{
					let encoder: &mut wgpu::CommandEncoder = &mut encoder;
					encoder.copy_texture_to_texture(density.as_image_copy(), scratch.as_image_copy(), density.size());
				};
				in_flight.extend(self.scatter_pass(executor, &mut encoder, &scratch_view, &scatter_bind, &edges(&region, None, &[], Some((a, b)))));
				&scratch_bind
			}
			None => &density_bind,
		};
		self.resolve_pass(&mut encoder, &composite_view, active_bind, scissor(&region, &crop, active_bounds));

		let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());
		self.convert_pass(&mut encoder, &output_view, &self.convert_bind(device, &composite_view));

		queue.submit([encoder.finish()]);
		args.cache.store(
			&args.footprint,
			AirbrushSlot {
				strokes: keys.into_iter().zip(bounds).collect(),
				base: bake
					.map(|texture| (texture.downgrade(), crop.origin))
					.or_else(|| base.map(|(texture, origin)| (texture.downgrade(), origin))),
				active: Some(ActiveState {
					seed: active.seed,
					style: active_style_hash,
					walk,
					last_position: *active.position.last().unwrap(),
					density: density.downgrade(),
				}),
			},
		);
		Some((output, crop.transform(&region)))
	}
}

impl AirbrushPipeline {
	fn scatter_bind(&self, device: &wgpu::Device, globals: &wgpu::Buffer) -> wgpu::BindGroup {
		device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("airbrush_density_bind_group"),
			layout: &self.density_bind_group_layout,
			entries: &[wgpu::BindGroupEntry {
				binding: 0,
				resource: globals.as_entire_binding(),
			}],
		})
	}

	fn resolve_bind(&self, device: &wgpu::Device, globals: &wgpu::Buffer, source: &wgpu::TextureView) -> wgpu::BindGroup {
		device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("airbrush_resolve_bind_group"),
			layout: &self.resolve_bind_group_layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: globals.as_entire_binding(),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::TextureView(source),
				},
			],
		})
	}

	fn convert_bind(&self, device: &wgpu::Device, source: &wgpu::TextureView) -> wgpu::BindGroup {
		device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("airbrush_convert_bind_group"),
			layout: &self.convert_bind_group_layout,
			entries: &[wgpu::BindGroupEntry {
				binding: 0,
				resource: wgpu::BindingResource::TextureView(source),
			}],
		})
	}

	/// Scatter `edges` as instanced quads additively into a density texture. Returns the
	/// instance buffer, which the caller keeps alive until the submit.
	fn scatter_pass(&self, executor: &WgpuExecutor, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, bind: &wgpu::BindGroup, edges: &[Edge]) -> Option<Buffer> {
		if edges.is_empty() {
			return None;
		}
		let buffer = executor.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("airbrush_segment_buffer"),
			contents: bytemuck::cast_slice(edges),
			usage: wgpu::BufferUsages::VERTEX,
		});
		let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some("airbrush_density_pass"),
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view,
				resolve_target: None,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Load,
					store: wgpu::StoreOp::Store,
				},
				depth_slice: None,
			})],
			..Default::default()
		});
		pass.set_pipeline(&self.density_pipeline);
		pass.set_bind_group(0, bind, &[]);
		pass.set_vertex_buffer(0, buffer.slice(..));
		pass.draw(0..4, 0..edges.len() as u32);
		Some(buffer)
	}

	/// Resolve a density texture to color over the target, scissored to `scissor`.
	fn resolve_pass(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, bind: &wgpu::BindGroup, scissor: (UVec2, UVec2)) {
		let (origin, size) = scissor;
		if !size.cmpgt(UVec2::ZERO).all() {
			return;
		}
		let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some("airbrush_resolve_pass"),
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view,
				resolve_target: None,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Load,
					store: wgpu::StoreOp::Store,
				},
				depth_slice: None,
			})],
			..Default::default()
		});
		pass.set_pipeline(&self.resolve_pipeline);
		pass.set_bind_group(0, bind, &[]);
		pass.set_scissor_rect(origin.x, origin.y, size.x, size.y);
		pass.draw(0..3, 0..1);
	}

	/// Encode the premultiplied linear composite into a straight-alpha sRGB target.
	fn convert_pass(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, bind: &wgpu::BindGroup) {
		let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some("airbrush_convert_pass"),
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view,
				resolve_target: None,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
					store: wgpu::StoreOp::Store,
				},
				depth_slice: None,
			})],
			..Default::default()
		});
		pass.set_pipeline(&self.convert_pipeline);
		pass.set_bind_group(0, bind, &[]);
		pass.draw(0..3, 0..1);
	}
}

fn clear(encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
	encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
		label: Some("airbrush_clear_pass"),
		color_attachments: &[Some(wgpu::RenderPassColorAttachment {
			view,
			resolve_target: None,
			ops: wgpu::Operations {
				load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
				store: wgpu::StoreOp::Store,
			},
			depth_slice: None,
		})],
		..Default::default()
	});
}

fn copy_placed(encoder: &mut wgpu::CommandEncoder, from: &wgpu::Texture, from_origin: UVec2, to: &wgpu::Texture, to_origin: UVec2) {
	let start = from_origin.max(to_origin);
	let end = (from_origin + UVec2::new(from.width(), from.height())).min(to_origin + UVec2::new(to.width(), to.height()));
	if !end.cmpgt(start).all() {
		return;
	}
	let info = |texture, origin: UVec2| wgpu::TexelCopyTextureInfo {
		texture,
		mip_level: 0,
		origin: wgpu::Origin3d { x: origin.x, y: origin.y, z: 0 },
		aspect: wgpu::TextureAspect::All,
	};
	let extent = end - start;
	encoder.copy_texture_to_texture(
		info(from, start - from_origin),
		info(to, start - to_origin),
		wgpu::Extent3d {
			width: extent.x,
			height: extent.y,
			depth_or_array_layers: 1,
		},
	);
}
