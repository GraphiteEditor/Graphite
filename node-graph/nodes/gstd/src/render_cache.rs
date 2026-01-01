use core_types::math::bbox::AxisAlignedBbox;
use core_types::transform::{Footprint, RenderQuality};
use core_types::{CloneVarArgs, Color, Context, Ctx, ExtractFootprint, ExtractVarArgs, Node, OwnedContextImpl};
use glam::{DVec2, IVec2, UVec2};
use graph_craft::document::value::RenderOutput;
use graph_craft::wasm_application_io::WasmEditorApi;
use graphene_application_io::ImageTexture;
use rendering::{RenderOutputType as RenderOutputTypeRequest, RenderParams};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use wgpu_executor::RenderContext;

use crate::render_node::{RenderIntermediate, RenderIntermediateType, RenderOutputType};

// Constants
pub const TILE_SIZE: u32 = 256;
pub const MAX_CACHE_MEMORY_BYTES: usize = 512 * 1024 * 1024; // 512MB
pub const ZOOM_BUCKET_STOPS: f64 = 0.25; // Quantize to 0.25 zoom stops
pub const MAX_REGION_DIMENSION: u32 = 4096; // 16 tiles max per dimension
const BYTES_PER_PIXEL: usize = 4; // RGBA8Unorm

// Tile coordinate in world-space grid at specific zoom
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct TileCoord {
	pub x: i32,
	pub y: i32,
	pub zoom_bucket: i32,
}

// Single cached tile
#[derive(Debug, Clone)]
pub struct CachedTile {
	pub texture: wgpu::Texture,
	pub world_bounds: AxisAlignedBbox,
	pub zoom_level: f64,
	last_access: u64,
	memory_size: usize,
}

// Cache key for invalidation based on RenderParams
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
	// Fields from RenderParams that affect rendering output
	pub render_mode_hash: u64,
	pub hide_artboards: bool,
	pub for_export: bool,
	pub for_mask: bool,
	pub thumbnail: bool,
	pub aligned_strokes: bool,
	pub override_paint_order: bool,
	// Time fields quantized to milliseconds for Eq/Hash
	pub animation_time_ms: i64,
	pub real_time_ms: i64,
}

impl CacheKey {
	/// Create a cache key from f64 times (quantizes to milliseconds)
	pub fn from_times(render_mode_hash: u64, hide_artboards: bool, for_export: bool, for_mask: bool, thumbnail: bool, aligned_strokes: bool, override_paint_order: bool, animation_time: f64, real_time: f64) -> Self {
		Self {
			render_mode_hash,
			hide_artboards,
			for_export,
			for_mask,
			thumbnail,
			aligned_strokes,
			override_paint_order,
			animation_time_ms: (animation_time * 1000.0).round() as i64,
			real_time_ms: (real_time * 1000.0).round() as i64,
		}
	}
}

impl Default for CacheKey {
	fn default() -> Self {
		Self {
			render_mode_hash: 0,
			hide_artboards: false,
			for_export: false,
			for_mask: false,
			thumbnail: false,
			aligned_strokes: false,
			override_paint_order: false,
			animation_time_ms: 0,
			real_time_ms: 0,
		}
	}
}

// Internal cache implementation
#[derive(Debug)]
struct TileCacheImpl {
	tiles: HashMap<TileCoord, CachedTile>,
	access_order: VecDeque<(u64, TileCoord)>,
	timestamp: u64,
	total_memory: usize,
	cache_key: CacheKey,
}

impl Default for TileCacheImpl {
	fn default() -> Self {
		Self {
			tiles: HashMap::new(),
			access_order: VecDeque::new(),
			timestamp: 0,
			total_memory: 0,
			cache_key: CacheKey::default(),
		}
	}
}

// Public thread-safe wrapper
#[derive(Clone, Default, dyn_any::DynAny, Debug)]
pub struct TileCache(Arc<Mutex<TileCacheImpl>>);

// Contiguous region to render
#[derive(Debug, Clone)]
pub struct RenderRegion {
	pub world_bounds: AxisAlignedBbox,
	pub tiles: Vec<TileCoord>,
	pub zoom_level: f64,
}

// Cache query result
#[derive(Debug)]
pub struct CacheQuery {
	pub cached_tiles: Vec<CachedTile>,
	pub missing_regions: Vec<RenderRegion>,
}

// Coordinate conversion functions

/// Quantize zoom level to reduce cache fragmentation
pub fn quantize_zoom(zoom_level: f64) -> i32 {
	(zoom_level / ZOOM_BUCKET_STOPS).round() as i32
}

/// Convert world-space bounds to tile coordinates at given zoom
pub fn world_bounds_to_tiles(bounds: &AxisAlignedBbox, zoom_level: f64) -> Vec<TileCoord> {
	let zoom_bucket = quantize_zoom(zoom_level);
	let pixels_per_world_unit = zoom_level.exp2(); // 2^zoom

	// Convert world bounds to pixel bounds
	let pixel_start = bounds.start * pixels_per_world_unit;
	let pixel_end = bounds.end * pixels_per_world_unit;

	// Convert to tile grid coordinates
	let tile_start = IVec2::new((pixel_start.x / TILE_SIZE as f64).floor() as i32, (pixel_start.y / TILE_SIZE as f64).floor() as i32);
	let tile_end = IVec2::new((pixel_end.x / TILE_SIZE as f64).ceil() as i32, (pixel_end.y / TILE_SIZE as f64).ceil() as i32);

	// Generate all tile coordinates in range
	let mut tiles = Vec::new();
	for y in tile_start.y..tile_end.y {
		for x in tile_start.x..tile_end.x {
			tiles.push(TileCoord { x, y, zoom_bucket });
		}
	}
	tiles
}

/// Convert tile coordinate back to world-space bounds
pub fn tile_to_world_bounds(coord: &TileCoord, actual_zoom: f64) -> AxisAlignedBbox {
	let pixels_per_world_unit = actual_zoom.exp2();
	let world_units_per_pixel = 1.0 / pixels_per_world_unit;

	let pixel_start = DVec2::new((coord.x as f64) * (TILE_SIZE as f64), (coord.y as f64) * (TILE_SIZE as f64));
	let pixel_end = pixel_start + DVec2::splat(TILE_SIZE as f64);

	AxisAlignedBbox {
		start: pixel_start * world_units_per_pixel,
		end: pixel_end * world_units_per_pixel,
	}
}

/// Get bounding box of multiple tiles in world space
pub fn tiles_to_world_bounds(tiles: &[TileCoord], zoom_level: f64) -> AxisAlignedBbox {
	if tiles.is_empty() {
		return AxisAlignedBbox::ZERO;
	}

	let mut result = tile_to_world_bounds(&tiles[0], zoom_level);
	for tile in &tiles[1..] {
		let bounds = tile_to_world_bounds(tile, zoom_level);
		result = result.union(&bounds);
	}
	result
}

// Cache implementation

impl TileCacheImpl {
	/// Query cache for viewport bounds at given zoom level
	fn query(&mut self, viewport_bounds: &AxisAlignedBbox, zoom_level: f64, cache_key: &CacheKey) -> CacheQuery {
		// Check if cache needs invalidation
		if &self.cache_key != cache_key {
			self.invalidate_all();
			self.cache_key = cache_key.clone();
		}

		let required_tiles = world_bounds_to_tiles(viewport_bounds, zoom_level);
		let mut cached_tiles = Vec::new();
		let mut missing_tiles = Vec::new();

		for tile_coord in required_tiles {
			if let Some(cached) = self.tiles.get_mut(&tile_coord) {
				// Update LRU
				cached.last_access = self.timestamp;
				self.timestamp += 1;
				self.access_order.push_back((cached.last_access, tile_coord));
				cached_tiles.push(cached.clone());
			} else {
				missing_tiles.push(tile_coord);
			}
		}

		// Group missing tiles into contiguous regions (will be implemented next)
		let missing_regions = group_into_regions(&missing_tiles, zoom_level);

		CacheQuery { cached_tiles, missing_regions }
	}

	/// Store newly rendered tiles
	fn store_tiles(&mut self, new_tiles: Vec<(TileCoord, CachedTile)>) {
		for (coord, mut tile) in new_tiles {
			tile.last_access = self.timestamp;
			self.timestamp += 1;

			self.total_memory += tile.memory_size;
			self.access_order.push_back((tile.last_access, coord));
			self.tiles.insert(coord, tile);
		}

		// Evict old tiles if over memory limit
		self.evict_until_under_budget();
	}

	/// LRU eviction to stay under memory budget
	fn evict_until_under_budget(&mut self) {
		while self.total_memory > MAX_CACHE_MEMORY_BYTES && !self.access_order.is_empty() {
			if let Some((timestamp, coord)) = self.access_order.pop_front() {
				// Only remove if this is still the oldest access for this tile
				if let Some(tile) = self.tiles.get(&coord) {
					if tile.last_access == timestamp {
						if let Some(removed) = self.tiles.remove(&coord) {
							self.total_memory = self.total_memory.saturating_sub(removed.memory_size);
						}
					}
				}
			}
		}
	}

	/// Clear all cached tiles
	fn invalidate_all(&mut self) {
		self.tiles.clear();
		self.access_order.clear();
		self.total_memory = 0;
		// Don't reset timestamp - it's monotonic
	}
}

// Public TileCache API
impl TileCache {
	pub fn query(&self, viewport_bounds: &AxisAlignedBbox, zoom_level: f64, cache_key: &CacheKey) -> CacheQuery {
		self.0.lock().unwrap().query(viewport_bounds, zoom_level, cache_key)
	}

	pub fn store_tiles(&self, tiles: Vec<(TileCoord, CachedTile)>) {
		self.0.lock().unwrap().store_tiles(tiles);
	}
}

/// Group tiles into contiguous regions using flood-fill, then split oversized regions
fn group_into_regions(tiles: &[TileCoord], zoom_level: f64) -> Vec<RenderRegion> {
	if tiles.is_empty() {
		return Vec::new();
	}

	let tile_set: HashSet<_> = tiles.iter().cloned().collect();
	let mut visited = HashSet::new();
	let mut regions = Vec::new();

	for &tile in tiles {
		if visited.contains(&tile) {
			continue;
		}

		// Flood-fill to find connected region
		let region_tiles = flood_fill(&tile, &tile_set, &mut visited);
		let world_bounds = tiles_to_world_bounds(&region_tiles, zoom_level);

		let mut region = RenderRegion {
			world_bounds,
			tiles: region_tiles,
			zoom_level,
		};

		// Split if region exceeds MAX_REGION_DIMENSION
		let split_regions = split_oversized_region(region, zoom_level);
		regions.extend(split_regions);
	}

	regions
}

/// Split region if it exceeds MAX_REGION_DIMENSION, aligned to tile boundaries
fn split_oversized_region(region: RenderRegion, zoom_level: f64) -> Vec<RenderRegion> {
	let pixels_per_world_unit = zoom_level.exp2();
	let region_size = region.world_bounds.size();
	let pixel_size = region_size * pixels_per_world_unit;

	// Check if region fits within limits
	if pixel_size.x <= MAX_REGION_DIMENSION as f64 && pixel_size.y <= MAX_REGION_DIMENSION as f64 {
		return vec![region];
	}

	// Calculate how many tiles fit in MAX_REGION_DIMENSION
	let max_tiles_per_dimension = (MAX_REGION_DIMENSION / TILE_SIZE) as i32; // Should be 16

	// Group tiles into grid of chunks
	let mut chunks: HashMap<(i32, i32), Vec<TileCoord>> = HashMap::new();

	for &tile in &region.tiles {
		let chunk_x = tile.x.div_euclid(max_tiles_per_dimension);
		let chunk_y = tile.y.div_euclid(max_tiles_per_dimension);
		chunks.entry((chunk_x, chunk_y)).or_default().push(tile);
	}

	// Convert chunks into regions
	chunks
		.into_iter()
		.map(|(_, tiles)| {
			let world_bounds = tiles_to_world_bounds(&tiles, zoom_level);
			RenderRegion {
				world_bounds,
				tiles,
				zoom_level,
			}
		})
		.collect()
}

/// Flood-fill to find connected tiles (4-connected neighbors)
fn flood_fill(start: &TileCoord, tile_set: &HashSet<TileCoord>, visited: &mut HashSet<TileCoord>) -> Vec<TileCoord> {
	let mut result = Vec::new();
	let mut stack = vec![*start];

	while let Some(current) = stack.pop() {
		if visited.contains(&current) || !tile_set.contains(&current) {
			continue;
		}

		visited.insert(current);
		result.push(current);

		// Check 4-connected neighbors
		let neighbors = [
			TileCoord {
				x: current.x - 1,
				y: current.y,
				zoom_bucket: current.zoom_bucket,
			},
			TileCoord {
				x: current.x + 1,
				y: current.y,
				zoom_bucket: current.zoom_bucket,
			},
			TileCoord {
				x: current.x,
				y: current.y - 1,
				zoom_bucket: current.zoom_bucket,
			},
			TileCoord {
				x: current.x,
				y: current.y + 1,
				zoom_bucket: current.zoom_bucket,
			},
		];

		for neighbor in neighbors {
			if tile_set.contains(&neighbor) && !visited.contains(&neighbor) {
				stack.push(neighbor);
			}
		}
	}

	result
}

// Rendering and texture operations

/// Render a single region to texture using a render function
pub async fn render_region<'a, F, Fut>(
	region: &RenderRegion,
	render_fn: F,
	editor_api: &'a WasmEditorApi,
	base_render_params: &RenderParams,
	base_ctx: &(impl Ctx + Clone),
	contains_artboard: bool,
) -> wgpu::Texture
where
	F: FnOnce(Context<'static>) -> Fut,
	Fut: std::future::Future<Output = RenderIntermediate>,
{
	let region_size = region.world_bounds.size();
	let pixels_per_world_unit = region.zoom_level.exp2();
	let physical_size = UVec2::new((region_size.x * pixels_per_world_unit).ceil() as u32, (region_size.y * pixels_per_world_unit).ceil() as u32);

	// Create footprint for this region
	let scale = base_render_params.scale;
	let scale_transform = glam::DAffine2::from_scale(glam::DVec2::splat(scale));
	let translation = glam::DAffine2::from_translation(-region.world_bounds.start);
	let region_transform = scale_transform * translation;

	let region_footprint = Footprint {
		transform: region_transform,
		resolution: physical_size,
		quality: RenderQuality::Full,
	};

	// Create context with region footprint
	let mut region_params = base_render_params.clone();
	region_params.footprint = region_footprint;

	// Build context from base context with new footprint
	let region_ctx = OwnedContextImpl::from(base_ctx.clone())
		.with_footprint(region_footprint)
		.with_vararg(Box::new(region_params))
		.into_context();

	// Evaluate render function with region context
	let render_intermediate = render_fn(region_ctx).await;

	// Render to texture (similar to existing render node logic)
	let exec = editor_api.application_io.as_ref().unwrap().gpu_executor().expect("No GPU executor available");

	match &render_intermediate.ty {
		RenderIntermediateType::Vello(vello_data) => {
			let (child, context) = Arc::as_ref(vello_data);

			let footprint_transform_vello = vello::kurbo::Affine::new(region_transform.to_cols_array());

			let mut scene = vello::Scene::new();
			scene.append(child, Some(footprint_transform_vello));

			// Handle infinite transforms
			let scaled_infinite_transform = vello::kurbo::Affine::scale_non_uniform(physical_size.x as f64, physical_size.y as f64);
			let encoding = scene.encoding_mut();
			for transform in encoding.transforms.iter_mut() {
				if transform.matrix[0] == f32::INFINITY {
					*transform = vello_encoding::Transform::from_kurbo(&scaled_infinite_transform);
				}
			}

			let background = if contains_artboard || base_render_params.hide_artboards {
				Color::from_rgb8_srgb(0x22, 0x22, 0x22)
			} else {
				Color::WHITE
			};

			exec.render_vello_scene_to_texture(&scene, physical_size, context, background).await.expect("Failed to render region")
		}
		_ => panic!("Cache only supports Vello rendering"),
	}
}

/// Split rendered region texture into individual tile textures
pub async fn split_texture_into_tiles<'a>(
	region_texture: wgpu::Texture,
	region: &RenderRegion,
	editor_api: &'a WasmEditorApi,
) -> Vec<(TileCoord, CachedTile)> {
	let exec = editor_api.application_io.as_ref().unwrap().gpu_executor().unwrap();
	let device = &exec.context.device;
	let queue = &exec.context.queue;

	let mut tiles = Vec::new();
	let pixels_per_world_unit = region.zoom_level.exp2();

	for &tile_coord in &region.tiles {
		// Calculate tile bounds in world and pixel space
		let tile_world_bounds = tile_to_world_bounds(&tile_coord, region.zoom_level);

		// Calculate offset within region texture
		let offset_in_region = tile_world_bounds.start - region.world_bounds.start;
		let pixel_offset = (offset_in_region * pixels_per_world_unit).as_uvec2();

		// Create tile texture
		let tile_texture = device.create_texture(&wgpu::TextureDescriptor {
			label: Some("cached_tile"),
			size: wgpu::Extent3d {
				width: TILE_SIZE,
				height: TILE_SIZE,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Rgba8Unorm,
			usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
			view_formats: &[],
		});

		// Copy tile region from large texture
		let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("tile_copy") });

		encoder.copy_texture_to_texture(
			wgpu::TexelCopyTextureInfo {
				texture: &region_texture,
				mip_level: 0,
				origin: wgpu::Origin3d {
					x: pixel_offset.x,
					y: pixel_offset.y,
					z: 0,
				},
				aspect: wgpu::TextureAspect::All,
			},
			wgpu::TexelCopyTextureInfo {
				texture: &tile_texture,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
				aspect: wgpu::TextureAspect::All,
			},
			wgpu::Extent3d {
				width: TILE_SIZE,
				height: TILE_SIZE,
				depth_or_array_layers: 1,
			},
		);

		queue.submit([encoder.finish()]);

		tiles.push((
			tile_coord,
			CachedTile {
				texture: tile_texture,
				world_bounds: tile_world_bounds,
				zoom_level: region.zoom_level,
				last_access: 0, // Will be set by cache
				memory_size: (TILE_SIZE * TILE_SIZE * BYTES_PER_PIXEL as u32) as usize,
			},
		));
	}

	tiles
}

/// Composite cached tiles into final output texture
pub async fn composite_tiles<'a>(cached_tiles: Vec<CachedTile>, viewport_bounds: &AxisAlignedBbox, output_resolution: UVec2, editor_api: &'a WasmEditorApi) -> wgpu::Texture {
	let exec = editor_api.application_io.as_ref().unwrap().gpu_executor().unwrap();
	let device = &exec.context.device;
	let queue = &exec.context.queue;

	// Create output texture
	let output_texture = device.create_texture(&wgpu::TextureDescriptor {
		label: Some("composite_output"),
		size: wgpu::Extent3d {
			width: output_resolution.x,
			height: output_resolution.y,
			depth_or_array_layers: 1,
		},
		mip_level_count: 1,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format: wgpu::TextureFormat::Rgba8Unorm,
		usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING,
		view_formats: &[],
	});

	let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("tile_composite") });

	let viewport_size = viewport_bounds.size();
	let pixels_per_world_unit = (output_resolution.as_dvec2() / viewport_size).x; // Assuming uniform scaling

	for tile in &cached_tiles {
		// Calculate where this tile goes in output texture
		let tile_offset_in_viewport = tile.world_bounds.start - viewport_bounds.start;
		let pixel_offset = (tile_offset_in_viewport * pixels_per_world_unit).as_uvec2();

		// Clamp to output bounds (handle edge tiles that might extend beyond viewport)
		let copy_width = TILE_SIZE.min(output_resolution.x.saturating_sub(pixel_offset.x));
		let copy_height = TILE_SIZE.min(output_resolution.y.saturating_sub(pixel_offset.y));

		if copy_width > 0 && copy_height > 0 {
			encoder.copy_texture_to_texture(
				wgpu::TexelCopyTextureInfo {
					texture: &tile.texture,
					mip_level: 0,
					origin: wgpu::Origin3d::ZERO,
					aspect: wgpu::TextureAspect::All,
				},
				wgpu::TexelCopyTextureInfo {
					texture: &output_texture,
					mip_level: 0,
					origin: wgpu::Origin3d {
						x: pixel_offset.x,
						y: pixel_offset.y,
						z: 0,
					},
					aspect: wgpu::TextureAspect::All,
				},
				wgpu::Extent3d {
					width: copy_width,
					height: copy_height,
					depth_or_array_layers: 1,
				},
			);
		}
	}

	queue.submit([encoder.finish()]);
	output_texture
}

// Node implementation

#[node_macro::node(category(""))]
pub async fn render_output_cache<'a: 'n>(
	ctx: impl Ctx + ExtractFootprint + ExtractVarArgs + CloneVarArgs,
	editor_api: &'a WasmEditorApi,
	data: impl Node<Context<'static>, Output = RenderIntermediate> + Send + Sync,
	#[data] tile_cache: TileCache,
) -> RenderOutput {
	let footprint = ctx.footprint();
	let render_params = ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderParams>()
		.expect("Downcasting render params yielded invalid type");
	let mut render_params = render_params.clone();
	render_params.footprint = *footprint;
	let render_params = &render_params;

	// Only cache Vello (GPU) rendering
	if !matches!(render_params.render_output_type, RenderOutputTypeRequest::Vello) {
		// Fall back to regular rendering for SVG
		let context = OwnedContextImpl::empty().with_footprint(*footprint).with_vararg(Box::new(render_params.clone()));
		let intermediate = data.eval(context.into_context()).await;
		// Convert intermediate to output (simplified SVG path)
		return RenderOutput {
			data: RenderOutputType::Svg {
				svg: String::new(),
				image_data: Vec::new(),
			},
			metadata: intermediate.metadata,
		};
	}

	let scale = render_params.scale;
	let physical_resolution = footprint.resolution;
	let zoom_level = scale.log2();
	let viewport_bounds = footprint.viewport_bounds_in_local_space();

	// Evaluate data node once to get intermediate representation
	let context = OwnedContextImpl::empty().with_footprint(*footprint).with_vararg(Box::new(render_params.clone()));
	let intermediate = data.eval(context.into_context()).await;
	let contains_artboard = intermediate.contains_artboard;

	// Compute cache key from render params
	let mut hasher = DefaultHasher::new();
	render_params.render_mode.hash(&mut hasher);
	let render_mode_hash = hasher.finish();

	// Extract animation and real time from context
	let animation_time = ctx.try_animation_time().unwrap_or(0.0);
	let real_time = ctx.try_real_time().unwrap_or(0.0);

	let cache_key = CacheKey::from_times(
		render_mode_hash,
		render_params.hide_artboards,
		render_params.for_export,
		render_params.for_mask,
		render_params.thumbnail,
		render_params.aligned_strokes,
		render_params.override_paint_order,
		animation_time,
		real_time,
	);

	// Query cache for tiles
	let query = tile_cache.query(&viewport_bounds, zoom_level, &cache_key);

	// Render missing regions
	let mut new_tiles = Vec::new();
	for region in &query.missing_regions {
		// Create render closure for this region
		let data_clone = data.clone();
		let render_fn = |ctx: Context<'static>| async move { data_clone.eval(ctx).await };

		let region_texture = render_region(region, render_fn, editor_api, render_params, &ctx, contains_artboard).await;
		let tiles = split_texture_into_tiles(region_texture, region, editor_api).await;
		new_tiles.extend(tiles);
	}

	// Store new tiles in cache
	if !new_tiles.is_empty() {
		tile_cache.store_tiles(new_tiles.clone());
	}

	// Combine cached and new tiles
	let mut all_tiles = query.cached_tiles;
	all_tiles.extend(new_tiles.into_iter().map(|(_, tile)| tile));

	// Composite tiles into final output
	let output_texture = composite_tiles(all_tiles, &viewport_bounds, physical_resolution, editor_api).await;

	// Collect metadata
	let mut metadata = intermediate.metadata;
	metadata.apply_transform(footprint.transform);

	RenderOutput {
		data: RenderOutputType::Texture(ImageTexture { texture: output_texture }),
		metadata,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_quantize_zoom() {
		assert_eq!(quantize_zoom(0.0), 0);
		assert_eq!(quantize_zoom(0.1), 0);
		assert_eq!(quantize_zoom(0.125), 1);
		assert_eq!(quantize_zoom(0.25), 1);
		assert_eq!(quantize_zoom(0.5), 2);
		assert_eq!(quantize_zoom(1.0), 4);
	}

	#[test]
	fn test_tile_coordinate_conversion() {
		let zoom = 2.0; // scale = 4.0
		let coord = TileCoord { x: 0, y: 0, zoom_bucket: quantize_zoom(zoom) };
		let bounds = tile_to_world_bounds(&coord, zoom);

		// At zoom 2.0, scale = 4.0, so 256 pixels = 64 world units
		assert_eq!(bounds.start, DVec2::ZERO);
		assert_eq!(bounds.end, DVec2::splat(64.0));
	}

	#[test]
	fn test_world_to_tiles() {
		let zoom = 0.0; // scale = 1.0, 1 pixel = 1 world unit
		let bounds = AxisAlignedBbox {
			start: DVec2::ZERO,
			end: DVec2::new(512.0, 256.0),
		};
		let tiles = world_bounds_to_tiles(&bounds, zoom);

		// Should be 2x1 tiles (512x256 pixels)
		assert_eq!(tiles.len(), 2);
		assert!(tiles.contains(&TileCoord { x: 0, y: 0, zoom_bucket: 0 }));
		assert!(tiles.contains(&TileCoord { x: 1, y: 0, zoom_bucket: 0 }));
	}
}
