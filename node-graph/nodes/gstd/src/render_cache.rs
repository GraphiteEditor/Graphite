//! Tile-based render caching for efficient viewport panning.

use core_types::list::Item;
use core_types::math::bbox::AxisAlignedBbox;
use core_types::transform::{Footprint, RenderQuality, Transform};
use core_types::{CloneVarArgs, Context, Ctx, ExtractAll, ExtractAnimationTime, ExtractPointerPosition, ExtractRealTime, Generations, NodeError, OwnedContextImpl, Poll};
use glam::{DAffine2, DVec2, IVec2, UVec2};
use graph_craft::document::value::{RenderOutput, RenderOutputType};
use rendering::{RenderOutputType as RenderOutputTypeRequest, RenderParams};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::{Arc, Mutex};
use vector_types::vector::style::RenderMode;
use wgpu_executor::WgpuExecutor;

pub const TILE_SIZE: u32 = 256;
pub const MAX_CACHE_MEMORY_BYTES: usize = 512 * 1024 * 1024;
const BYTES_PER_PIXEL: usize = 4;

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct TileCoord {
	pub x: i32,
	pub y: i32,
}

#[derive(Debug, Clone)]
pub struct CachedRegion {
	pub output: Poll<RenderOutput>,
	pub size: UVec2,
	pub tiles: Vec<TileCoord>,
	last_access: u64,
	memory_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct CacheKey {
	pub render_mode_hash: u64,
	pub zoom: u64,
	pub rotation: u64,
	pub for_export: bool,
	pub for_mask: bool,
	pub thumbnail: bool,
	pub aligned_strokes: bool,
	pub stroke_below: bool,
	pub animation_time_ms: i64,
	pub real_time_ms: i64,
	pub pointer: [u8; 16],
	pub generations: Generations,
}

impl CacheKey {
	#[expect(clippy::too_many_arguments)]
	fn new(
		render_mode_hash: u64,
		zoom: f64,
		rotation: f64,
		for_export: bool,
		for_mask: bool,
		thumbnail: bool,
		aligned_strokes: bool,
		stroke_below: bool,
		animation_time: f64,
		real_time: f64,
		pointer: Option<DVec2>,
		generations: Generations,
	) -> Self {
		let pointer_bytes = pointer
			.map(|p| {
				let mut bytes = [0u8; 16];
				bytes[..8].copy_from_slice(&p.x.to_le_bytes());
				bytes[8..].copy_from_slice(&p.y.to_le_bytes());
				bytes
			})
			.unwrap_or([0u8; 16]);
		const ROTATION_QUANTIZATION_DIGITS: i32 = 5;
		let quantization_amount = 10f64.powi(ROTATION_QUANTIZATION_DIGITS);
		let quantized_rotation = (rotation * quantization_amount).round() * quantization_amount.recip();
		Self {
			render_mode_hash,
			zoom: zoom.to_bits(),
			rotation: quantized_rotation.to_bits(),
			for_export,
			for_mask,
			thumbnail,
			aligned_strokes,
			stroke_below,
			animation_time_ms: (animation_time * 1000.).round() as i64,
			real_time_ms: (real_time * 1000.).round() as i64,
			pointer: pointer_bytes,
			generations,
		}
	}
}

#[derive(Clone, Default, dyn_any::DynAny, Debug)]
pub struct TileCache(Arc<Mutex<TileCacheImpl>>);

impl TileCache {
	pub fn query(&self, viewport_bounds: &AxisAlignedBbox, cache_key: &CacheKey, max_region_size: u32) -> CacheQuery {
		self.0.lock().unwrap().query(viewport_bounds, cache_key, max_region_size)
	}

	pub fn store_regions(&self, regions: Vec<CachedRegion>) {
		self.0.lock().unwrap().store_regions(regions);
	}
}

#[derive(Default, Debug)]
struct TileCacheImpl {
	regions: Vec<CachedRegion>,
	timestamp: u64,
	total_memory: usize,
	cache_key: CacheKey,
}

#[derive(Debug, Clone)]
pub struct RenderRegion {
	pub tiles: Vec<TileCoord>,
}

#[derive(Debug)]
pub struct CacheQuery {
	pub cached_regions: Vec<CachedRegion>,
	pub missing_regions: Vec<RenderRegion>,
}

fn bounds_to_tiles(bounds: &AxisAlignedBbox) -> Vec<TileCoord> {
	let tile_size = TILE_SIZE as f64;
	let tile_start_x = (bounds.start.x / tile_size).floor() as i32;
	let tile_start_y = (bounds.start.y / tile_size).floor() as i32;
	let tile_end_x = (bounds.end.x / tile_size).ceil() as i32;
	let tile_end_y = (bounds.end.y / tile_size).ceil() as i32;

	let mut tiles = Vec::new();
	for y in tile_start_y..tile_end_y {
		for x in tile_start_x..tile_end_x {
			tiles.push(TileCoord { x, y });
		}
	}
	tiles
}

fn tile_bounds(coord: &TileCoord) -> AxisAlignedBbox {
	let tile_size = TILE_SIZE as f64;
	let start = DVec2::new(coord.x as f64, coord.y as f64) * tile_size;
	AxisAlignedBbox {
		start,
		end: start + DVec2::splat(tile_size),
	}
}

fn tiles_bounds(tiles: &[TileCoord]) -> AxisAlignedBbox {
	if tiles.is_empty() {
		return AxisAlignedBbox::ZERO;
	}
	let mut result = tile_bounds(&tiles[0]);
	for tile in &tiles[1..] {
		result = result.union(&tile_bounds(tile));
	}
	result
}

impl TileCacheImpl {
	fn query(&mut self, viewport_bounds: &AxisAlignedBbox, cache_key: &CacheKey, max_region_size: u32) -> CacheQuery {
		if &self.cache_key != cache_key {
			self.invalidate_all();
			self.cache_key = cache_key.clone();
		}

		let required_tiles = bounds_to_tiles(viewport_bounds);
		let required_tile_set: HashSet<_> = required_tiles.iter().cloned().collect();
		let mut cached_regions = Vec::new();
		let mut covered_tiles = HashSet::new();

		for region in &mut self.regions {
			let region_tiles: HashSet<_> = region.tiles.iter().cloned().collect();
			if region_tiles.iter().any(|t| required_tile_set.contains(t)) {
				region.last_access = self.timestamp;
				self.timestamp += 1;
				cached_regions.push(region.clone());
				covered_tiles.extend(region_tiles);
			}
		}

		let missing_tiles: Vec<_> = required_tiles.into_iter().filter(|t| !covered_tiles.contains(t)).collect();
		let missing_regions = group_into_regions(&missing_tiles, max_region_size);
		CacheQuery { cached_regions, missing_regions }
	}

	fn store_regions(&mut self, new_regions: Vec<CachedRegion>) {
		for mut region in new_regions {
			region.last_access = self.timestamp;
			self.timestamp += 1;
			self.total_memory += region.memory_size;
			self.regions.push(region);
		}
		self.evict_until_under_budget();
	}

	fn evict_until_under_budget(&mut self) {
		while self.total_memory > MAX_CACHE_MEMORY_BYTES && !self.regions.is_empty() {
			if let Some((oldest_idx, _)) = self.regions.iter().enumerate().min_by_key(|(_, r)| r.last_access) {
				let removed = self.regions.remove(oldest_idx);
				self.total_memory = self.total_memory.saturating_sub(removed.memory_size);
			} else {
				break;
			}
		}
	}

	fn invalidate_all(&mut self) {
		self.regions.clear();
		self.total_memory = 0;
	}
}

/// Groups tiles into rectangular regions no wider or taller than `max_region_size` pixels.
/// A region renders its whole bounding box, so rectangles render no more than the tiles asked for.
fn group_into_regions(tiles: &[TileCoord], max_region_size: u32) -> Vec<RenderRegion> {
	tile_rectangles(tiles).into_iter().flat_map(|region| split_oversized_region(region, max_region_size)).collect()
}

/// Splits tiles into rectangles, each a run of adjacent tiles in a row extended down through the rows below with the same run. O(n log n) for n tiles.
fn tile_rectangles(tiles: &[TileCoord]) -> Vec<RenderRegion> {
	let mut sorted = tiles.to_vec();
	sorted.sort_unstable_by_key(|tile| (tile.y, tile.x));
	sorted.dedup();

	// Each row's runs of adjacent tiles, as their row and first and last columns
	let mut runs: Vec<(i32, i32, i32)> = Vec::new();
	for tile in sorted {
		match runs.last_mut() {
			Some((row, _, last)) if *row == tile.y && *last + 1 == tile.x => *last = tile.x,
			_ => runs.push((tile.y, tile.x, tile.x)),
		}
	}

	// The top and bottom rows of the rectangle each run of columns extends, which closes once a row lacks that run
	let mut open: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
	let mut rectangles = Vec::new();
	for (row, first, last) in runs {
		match open.get_mut(&(first, last)) {
			Some((_, bottom)) if *bottom + 1 == row => *bottom = row,
			_ => {
				if let Some(rows) = open.insert((first, last), (row, row)) {
					rectangles.push(((first, last), rows));
				}
			}
		}
	}
	rectangles.extend(open);

	rectangles
		.into_iter()
		.map(|((first, last), (top, bottom))| RenderRegion {
			tiles: (top..=bottom).flat_map(|y| (first..=last).map(move |x| TileCoord { x, y })).collect(),
		})
		.collect()
}

/// Recursively subdivides a region until no sub-region is wider or taller than `max_region_size` pixels.
/// Uses axis-aligned splits on the longest dimension.
fn split_oversized_region(region: RenderRegion, max_region_size: u32) -> Vec<RenderRegion> {
	let pixel_size = tiles_bounds(&region.tiles).size();

	// Base case: region is small enough
	if pixel_size.x <= max_region_size as f64 && pixel_size.y <= max_region_size as f64 {
		return vec![region];
	}

	// Determine split axis: choose the longer dimension
	let split_horizontally = pixel_size.x > pixel_size.y;

	// Split tiles into two groups based on midpoint
	let mut group1 = Vec::new();
	let mut group2 = Vec::new();

	if split_horizontally {
		let min_x = region.tiles.iter().map(|t| t.x).min().unwrap();
		let max_x = region.tiles.iter().map(|t| t.x).max().unwrap();
		let mid_x = min_x + (max_x - min_x) / 2;

		for &tile in &region.tiles {
			if tile.x <= mid_x {
				group1.push(tile);
			} else {
				group2.push(tile);
			}
		}
	} else {
		let min_y = region.tiles.iter().map(|t| t.y).min().unwrap();
		let max_y = region.tiles.iter().map(|t| t.y).max().unwrap();
		let mid_y = min_y + (max_y - min_y) / 2;

		for &tile in &region.tiles {
			if tile.y <= mid_y {
				group1.push(tile);
			} else {
				group2.push(tile);
			}
		}
	}

	if group1.is_empty() || group2.is_empty() {
		log::error!("Failed to split oversized region.");
		return vec![region];
	}

	let mut result = Vec::new();
	for tiles in [group1, group2] {
		if !tiles.is_empty() {
			let sub_region = RenderRegion { tiles };
			result.extend(split_oversized_region(sub_region, max_region_size));
		}
	}

	result
}

#[node_macro::node(category(""))]
fn render_output_cache(
	ctx: impl Ctx + ExtractAll + CloneVarArgs + ExtractRealTime + ExtractAnimationTime + ExtractPointerPosition + Sync,
	#[scope(crate::platform_application_io::wgpu_executor::IDENTIFIER)] executor: impl Node<Context<'static>, Output = Item<WgpuExecutor>>,
	data: impl Node<Context<'static>, Output = Item<RenderOutput>> + Send + Sync,
	#[data] tile_cache: TileCache,
) -> Poll<Item<RenderOutput>> {
	let footprint = ctx.footprint();
	let Some(mut render_params) = ctx.vararg(0).ok().and_then(|v| v.downcast_ref::<RenderParams>().cloned()) else {
		log::warn!("render_output_cache: missing or invalid render params, falling back to direct render");
		let context = OwnedContextImpl::from(ctx.clone()).with_footprint(*footprint);
		return data.eval(&context.into_context());
	};

	render_params.viewport_zoom = if let RenderMode::Outline = render_params.render_mode { render_params.viewport_zoom } else { None };

	// Fall back to direct render for non-Vello or zero-size viewports
	let physical_resolution = footprint.resolution;
	if !matches!(render_params.render_output_type, RenderOutputTypeRequest::Vello) || physical_resolution.x == 0 || physical_resolution.y == 0 {
		let context = OwnedContextImpl::from(ctx.clone()).with_footprint(*footprint).with_vararg(Box::new(render_params.clone()));
		return data.eval(&context.into_context());
	}

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
	let finish = |output, partial, error: Option<NodeError>| match (partial || executor_partial, error.or(executor_error)) {
		(true, _) => Poll::Partial(output),
		(false, Some(error)) => Poll::Fallback(Box::new((output, error))),
		(false, None) => Poll::Final(output),
	};

	let zoom = footprint.scale_magnitudes().x;
	let rotation = footprint.decompose_rotation();

	let device_origin_offset = footprint.transform.translation;
	let viewport_bounds_device = AxisAlignedBbox {
		start: -device_origin_offset,
		end: footprint.resolution.as_dvec2() - device_origin_offset,
	};

	// Each region renders into a texture of its own, which Vello GPU can also draw layers across in full
	let max_texture_size = executor.context().device.limits().max_texture_dimension_2d;
	let max_region_size = max_texture_size.min(wgpu_executor::MAX_LAYER_RENDER_SIZE);

	// Outputs too large for one texture (in practice, large exports) are assembled on the CPU, which only works in native builds
	if physical_resolution.x > max_texture_size || physical_resolution.y > max_texture_size {
		let output = render_regions_to_buffer(
			&viewport_bounds_device,
			|ctx| data.eval(&ctx),
			ctx.clone(),
			&render_params,
			&footprint.transform,
			max_region_size,
			&executor,
		);
		return match output.map(Item::new_from_element) {
			Poll::Final(output) => finish(output, false, None),
			Poll::Partial(output) => finish(output, true, None),
			Poll::Fallback(boxed) => {
				let (output, error) = *boxed;
				finish(output, false, Some(error))
			}
			Poll::Pending => Poll::Pending,
			Poll::Error(error) => Poll::Error(error),
		};
	}

	let cache_key = CacheKey::new(
		render_params.render_mode as u64,
		zoom,
		rotation,
		render_params.for_export,
		render_params.for_mask,
		render_params.thumbnail,
		render_params.aligned_strokes,
		render_params.stroke_below,
		ctx.try_animation_time().unwrap_or(0.),
		ctx.try_real_time().unwrap_or(0.),
		ctx.try_pointer_position(),
		ctx.try_generations().cloned().unwrap_or_default(),
	);

	let cache_query = tile_cache.query(&viewport_bounds_device, &cache_key, max_region_size);

	// If no regions, fall back to direct render
	if cache_query.cached_regions.is_empty() && cache_query.missing_regions.iter().all(|region| region.tiles.is_empty()) {
		let context = OwnedContextImpl::from(ctx.clone()).with_footprint(*footprint).with_vararg(Box::new(render_params.clone()));
		return data.eval(&context.into_context());
	}

	let mut new_regions = Vec::new();
	for missing_region in cache_query.missing_regions.iter().filter(|region| !region.tiles.is_empty()) {
		let region = render_missing_region(missing_region, |ctx| data.eval(&ctx), ctx.clone(), &render_params, &footprint.transform, &device_origin_offset);
		match &region.output {
			Poll::Pending => return Poll::Pending,
			Poll::Error(error) => return Poll::Error(error.clone()),
			Poll::Final(_) | Poll::Partial(_) | Poll::Fallback(_) => new_regions.push(region),
		}
	}

	tile_cache.store_regions(new_regions.clone());

	let all_regions: Vec<_> = cache_query.cached_regions.into_iter().chain(new_regions).collect();
	let partial = all_regions.iter().any(|region| matches!(region.output, Poll::Partial(_)));
	let error = all_regions.iter().find_map(|region| match &region.output {
		Poll::Fallback(boxed) => Some(boxed.1.clone()),
		Poll::Final(_) | Poll::Partial(_) | Poll::Pending | Poll::Error(_) => None,
	});

	let output_texture = executor.request_texture(physical_resolution);

	let combined_metadata = composite_cached_regions(&all_regions, &output_texture, &device_origin_offset, &footprint.transform, &executor);

	let output = Item::new_from_element(RenderOutput {
		data: RenderOutputType::Texture(output_texture),
		metadata: combined_metadata,
	});
	finish(output, partial, error)
}

fn render_missing_region<F>(
	region: &RenderRegion,
	render_fn: F,
	ctx: impl Ctx + ExtractAll + CloneVarArgs,
	render_params: &RenderParams,
	viewport_transform: &DAffine2,
	viewport_origin_offset: &DVec2,
) -> CachedRegion
where
	F: Fn(Context<'static>) -> Poll<Item<RenderOutput>>,
{
	let min_tile = region.tiles.iter().fold(IVec2::new(i32::MAX, i32::MAX), |acc, t| acc.min(IVec2::new(t.x, t.y)));
	let max_tile = region.tiles.iter().fold(IVec2::new(i32::MIN, i32::MIN), |acc, t| acc.max(IVec2::new(t.x, t.y)));

	let tile_count = (max_tile - min_tile) + IVec2::ONE;
	let region_pixel_size = (tile_count * TILE_SIZE as i32).as_uvec2();

	let tile_global_offset = min_tile.as_dvec2() * TILE_SIZE as f64 + *viewport_origin_offset;
	let region_transform = DAffine2::from_translation(-tile_global_offset) * *viewport_transform;
	let region_footprint = Footprint {
		transform: region_transform,
		resolution: region_pixel_size,
		quality: RenderQuality::Full,
	};

	let region_params = render_params.clone();
	let region_ctx = OwnedContextImpl::from(ctx).with_footprint(region_footprint).with_vararg(Box::new(region_params)).into_context();
	let output = render_fn(region_ctx).map(|output| {
		let mut output = output.into_element();
		output.metadata.apply_transform(region_transform.inverse());
		output
	});

	CachedRegion {
		output,
		size: region_pixel_size,
		tiles: region.tiles.clone(),
		last_access: 0,
		memory_size: (region_pixel_size.x * region_pixel_size.y) as usize * BYTES_PER_PIXEL,
	}
}

fn composite_cached_regions(
	regions: &[CachedRegion],
	output_texture: &wgpu::Texture,
	device_origin_offset: &DVec2,
	viewport_transform: &DAffine2,
	exec: &wgpu_executor::WgpuExecutor,
) -> rendering::RenderMetadata {
	let device = &exec.context().device;
	let queue = &exec.context().queue;
	let output_resolution = UVec2::new(output_texture.width(), output_texture.height());

	let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("composite") });
	let mut combined_metadata = rendering::RenderMetadata::default();

	for region in regions {
		let output = match &region.output {
			Poll::Final(output) | Poll::Partial(output) => output,
			Poll::Fallback(boxed) => &boxed.0,
			Poll::Pending | Poll::Error(_) => unreachable!("Only rendered regions are composited"),
		};
		let RenderOutputType::Texture(texture) = &output.data else {
			unreachable!("composite_cached_regions: expected texture output from Vello render");
		};

		if let Some(placement) = region_placement(region, device_origin_offset, output_resolution) {
			encoder.copy_texture_to_texture(
				wgpu::TexelCopyTextureInfo {
					texture: texture.as_ref(),
					mip_level: 0,
					origin: wgpu::Origin3d {
						x: placement.source.x,
						y: placement.source.y,
						z: 0,
					},
					aspect: wgpu::TextureAspect::All,
				},
				wgpu::TexelCopyTextureInfo {
					texture: output_texture,
					mip_level: 0,
					origin: wgpu::Origin3d {
						x: placement.destination.x,
						y: placement.destination.y,
						z: 0,
					},
					aspect: wgpu::TextureAspect::All,
				},
				wgpu::Extent3d {
					width: placement.size.x,
					height: placement.size.y,
					depth_or_array_layers: 1,
				},
			);
		}

		let mut region_metadata = output.metadata.clone();
		region_metadata.apply_transform(*viewport_transform);
		combined_metadata.merge(&region_metadata);
	}

	queue.submit([encoder.finish()]);
	combined_metadata
}

/// The part of a region's texture that lands inside the output.
struct RegionPlacement {
	/// The origin of the part within the region's texture.
	source: UVec2,
	/// The origin of the part within the output.
	destination: UVec2,
	size: UVec2,
}

fn region_placement(region: &CachedRegion, device_origin_offset: &DVec2, output_resolution: UVec2) -> Option<RegionPlacement> {
	let min_tile = region.tiles.iter().fold(IVec2::new(i32::MAX, i32::MAX), |acc, t| acc.min(IVec2::new(t.x, t.y)));

	// Convert global tile position to physical pixel offset in the output texture
	let offset_pixels = min_tile * TILE_SIZE as i32 + device_origin_offset.round().as_ivec2();

	let (source_x, destination_x, width) = if offset_pixels.x >= 0 {
		(0, offset_pixels.x as u32, region.size.x.min(output_resolution.x.saturating_sub(offset_pixels.x as u32)))
	} else {
		let skip = (-offset_pixels.x) as u32;
		(skip, 0, region.size.x.saturating_sub(skip).min(output_resolution.x))
	};

	let (source_y, destination_y, height) = if offset_pixels.y >= 0 {
		(0, offset_pixels.y as u32, region.size.y.min(output_resolution.y.saturating_sub(offset_pixels.y as u32)))
	} else {
		let skip = (-offset_pixels.y) as u32;
		(skip, 0, region.size.y.saturating_sub(skip).min(output_resolution.y))
	};

	(width > 0 && height > 0).then_some(RegionPlacement {
		source: UVec2::new(source_x, source_y),
		destination: UVec2::new(destination_x, destination_y),
		size: UVec2::new(width, height),
	})
}

/// Renders an output too large for one texture into RGBA8 pixels on the CPU, rendering one region at a time and copying it in, so only one region's texture is in use at a time.
fn render_regions_to_buffer<F>(
	viewport_bounds_device: &AxisAlignedBbox,
	render_fn: F,
	ctx: impl Ctx + ExtractAll + CloneVarArgs,
	render_params: &RenderParams,
	viewport_transform: &DAffine2,
	max_region_size: u32,
	executor: &WgpuExecutor,
) -> Poll<RenderOutput>
where
	F: Fn(Context<'static>) -> Poll<Item<RenderOutput>>,
{
	let device_origin_offset = viewport_transform.translation;
	let resolution = (viewport_bounds_device.end - viewport_bounds_device.start).round().as_uvec2();
	let row_bytes = resolution.x as usize * BYTES_PER_PIXEL;

	let Some(buffer_size) = row_bytes.checked_mul(resolution.y as usize) else {
		return Poll::error(format!("An output of {resolution} pixels is too large to hold in memory"));
	};
	let mut pixels = vec![0_u8; buffer_size];
	let mut metadata = rendering::RenderMetadata::default();
	let mut partial = false;
	let mut error = None;

	for region in group_into_regions(&bounds_to_tiles(viewport_bounds_device), max_region_size) {
		let rendered = render_missing_region(&region, &render_fn, ctx.clone(), render_params, viewport_transform, &device_origin_offset);
		let output = match &rendered.output {
			Poll::Final(output) => output,
			Poll::Partial(output) => {
				partial = true;
				output
			}
			Poll::Fallback(boxed) => {
				error.get_or_insert_with(|| boxed.1.clone());
				&boxed.0
			}
			Poll::Pending => return Poll::Pending,
			Poll::Error(error) => return Poll::Error(error.clone()),
		};

		let mut region_metadata = output.metadata.clone();
		region_metadata.apply_transform(*viewport_transform);
		metadata.merge(&region_metadata);

		let Some(placement) = region_placement(&rendered, &device_origin_offset, resolution) else {
			continue;
		};
		let RenderOutputType::Texture(texture) = &output.data else {
			log::error!("Expected a texture from rendering a region with Vello");
			continue;
		};
		if (texture.width(), texture.height()) != (rendered.size.x, rendered.size.y) {
			return Poll::error("A rendered region's texture doesn't match the region's size");
		}
		// A failed readback, as always happens in a browser, fails the whole output rather than leaving a hole
		let region_pixels = match executor.read_texture_pixels(texture) {
			Ok(region_pixels) => region_pixels,
			Err(error) => return Poll::error(error.context("Failed to read back a rendered region")),
		};

		// Copy the overlapping rows of the region into place
		let region_row_bytes = texture.width() as usize * BYTES_PER_PIXEL;
		let copied_bytes = placement.size.x as usize * BYTES_PER_PIXEL;
		for row in 0..placement.size.y as usize {
			let source_start = (placement.source.y as usize + row) * region_row_bytes + placement.source.x as usize * BYTES_PER_PIXEL;
			let destination_start = (placement.destination.y as usize + row) * row_bytes + placement.destination.x as usize * BYTES_PER_PIXEL;
			pixels[destination_start..destination_start + copied_bytes].copy_from_slice(&region_pixels[source_start..source_start + copied_bytes]);
		}
	}

	let output = RenderOutput {
		data: RenderOutputType::Buffer {
			data: pixels,
			width: resolution.x,
			height: resolution.y,
		},
		metadata,
	};
	match (partial, error) {
		(true, _) => Poll::Partial(output),
		(false, Some(error)) => Poll::Fallback(Box::new((output, error))),
		(false, None) => Poll::Final(output),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn missing_tiles_in_an_l_shape_group_into_two_full_rectangles() {
		// The tiles a diagonal pan uncovers along the top and left of a viewport 10 tiles wide and 6 tall
		let tiles: Vec<_> = (0..6).flat_map(|y| (0..10).map(move |x| TileCoord { x, y })).filter(|tile| tile.y == 0 || tile.x < 2).collect();

		let regions = group_into_regions(&tiles, 4096);
		assert_eq!(regions.len(), 2);
		assert_eq!(regions.iter().map(|region| region.tiles.len()).sum::<usize>(), tiles.len());

		// A region renders its whole bounding box, so each must be filled by its tiles
		for region in &regions {
			let size = tiles_bounds(&region.tiles).size() / TILE_SIZE as f64;
			assert_eq!(region.tiles.len(), (size.x * size.y) as usize);
		}
	}
}
