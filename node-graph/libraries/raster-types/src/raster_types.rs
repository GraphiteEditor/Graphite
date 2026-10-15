use crate::image::Image;
use core::ops::Deref;
use core_types::Color;
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::math::quad::Quad;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use std::fmt::Debug;
use std::ops::DerefMut;

mod __private {
	pub trait Sealed {}
}

pub trait Storage: __private::Sealed + Clone + Debug + 'static {
	fn is_empty(&self) -> bool;
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Raster<T>
where
	Raster<T>: Storage,
{
	storage: T,
}

unsafe impl<T> dyn_any::StaticType for Raster<T>
where
	Raster<T>: Storage,
{
	type Static = Raster<T>;
}

impl<T> Raster<T>
where
	Raster<T>: Storage,
{
	pub fn new(t: T) -> Self {
		Self { storage: t }
	}
}

impl<T> Deref for Raster<T>
where
	Raster<T>: Storage,
{
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.storage
	}
}

impl<T> DerefMut for Raster<T>
where
	Raster<T>: Storage,
{
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.storage
	}
}

impl<T> core_types::CacheHash for Raster<T>
where
	Raster<T>: Storage,
	T: core_types::CacheHash,
{
	fn cache_hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
		core_types::CacheHash::cache_hash(&self.storage, state);
	}
}

pub use cpu::{CPU, WeakCPU};

mod cpu {
	use super::*;
	use crate::raster_types::__private::Sealed;
	use std::sync::atomic::{AtomicU64, Ordering};
	use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak};

	/// Pixels in CPU memory, shared between clones and identified by an ID that changes whenever they are modified.
	/// So cloning is a reference count increment, and hashing for caches reads the ID rather than every pixel.
	#[derive(Clone, Debug, Default, DynAny)]
	pub struct CPU(Arc<Buffer>);

	#[derive(Debug, Default)]
	struct Buffer {
		/// Zero until the ID is first read after the pixels last changed, so a run of modifications takes only one new ID.
		id: AtomicU64,
		image: Image<Color>,
		/// The pixels encoded as a PNG data URL, made the first time an SVG embeds them.
		png_data_url: OnceLock<Arc<str>>,
		/// The pixels most recently derived from these, with the key they were derived for.
		derived: Mutex<Option<(u64, Raster<CPU>)>>,
	}

	// A copy is made only to be modified, so it starts without an ID
	impl Clone for Buffer {
		fn clone(&self) -> Self {
			Self {
				id: AtomicU64::new(0),
				image: self.image.clone(),
				png_data_url: OnceLock::new(),
				derived: Mutex::new(None),
			}
		}
	}

	impl CPU {
		/// The ID of these pixels, shared by every clone until one of them is modified.
		pub fn buffer_id(&self) -> u64 {
			// Counts from one, since zero marks an ID not yet taken
			static NEXT_ID: AtomicU64 = AtomicU64::new(1);

			let id = self.0.id.load(Ordering::Relaxed);
			if id != 0 {
				return id;
			}

			let new_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
			match self.0.id.compare_exchange(0, new_id, Ordering::Relaxed, Ordering::Relaxed) {
				Ok(_) => new_id,
				// Another thread took one first
				Err(existing_id) => existing_id,
			}
		}
	}

	/// Refers to a CPU raster's pixels without keeping them in memory.
	#[derive(Clone, Debug)]
	pub struct WeakCPU {
		buffer: Weak<Buffer>,
		id: u64,
	}

	impl WeakCPU {
		/// Whether a clone still holds these pixels without having modified them.
		pub fn is_alive(&self) -> bool {
			self.buffer.upgrade().is_some_and(|buffer| buffer.id.load(Ordering::Relaxed) == self.id)
		}
	}

	impl PartialEq for CPU {
		fn eq(&self, other: &Self) -> bool {
			Arc::ptr_eq(&self.0, &other.0) || self.0.image == other.0.image
		}
	}

	// Pixels can't change without taking a new ID, so the ID stands in for them
	impl core_types::CacheHash for CPU {
		fn cache_hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
			core_types::CacheHash::cache_hash(&self.buffer_id(), state);
		}
	}

	impl Sealed for Raster<CPU> {}

	impl Storage for Raster<CPU> {
		fn is_empty(&self) -> bool {
			self.height == 0 || self.width == 0
		}
	}

	impl Raster<CPU> {
		pub fn new_cpu(image: Image<Color>) -> Self {
			Self::new(CPU(Arc::new(Buffer { image, ..Default::default() })))
		}

		pub fn data(&self) -> &Image<Color> {
			self
		}

		pub fn data_mut(&mut self) -> &mut Image<Color> {
			self
		}

		/// A reference to these pixels that doesn't keep them in memory.
		pub fn downgrade(&self) -> WeakCPU {
			WeakCPU {
				id: self.buffer_id(),
				buffer: Arc::downgrade(&self.storage.0),
			}
		}

		/// The pixels, copied only if another clone shares them.
		pub fn into_data(self) -> Image<Color> {
			Arc::try_unwrap(self.storage.0).map_or_else(|shared| shared.image.clone(), |buffer| buffer.image)
		}

		/// The pixels encoded as a PNG `data:` URL, encoded once and shared by every clone.
		pub fn png_data_url(&self) -> Arc<str> {
			self.0
				.png_data_url
				.get_or_init(|| {
					use base64::Engine;

					let preamble = "data:image/png;base64,";
					let png = self.to_png();
					let mut url = String::with_capacity(preamble.len() + png.len().div_ceil(3) * 4);
					url.push_str(preamble);
					base64::engine::general_purpose::STANDARD.encode_string(png, &mut url);
					url.into()
				})
				.clone()
		}

		/// Pixels derived from these by `derive` for `key`.
		/// The latest are kept with these pixels and shared by every clone, so deriving them again for the same key reuses them, ID and all.
		pub fn derived(&self, key: u64, derive: impl FnOnce(&Self) -> Raster<CPU>) -> Raster<CPU> {
			let mut derived = self.0.derived.lock().unwrap_or_else(PoisonError::into_inner);
			if let Some((derived_key, raster)) = &*derived
				&& *derived_key == key
			{
				return raster.clone();
			}

			let raster = derive(self);
			*derived = Some((key, raster.clone()));
			raster
		}
	}

	impl Deref for CPU {
		type Target = Image<Color>;

		fn deref(&self) -> &Self::Target {
			&self.0.image
		}
	}

	impl DerefMut for CPU {
		/// Copies the pixels if another clone shares them, and drops their ID, PNG encoding, and derived pixels since they may change.
		fn deref_mut(&mut self) -> &mut Self::Target {
			let buffer = Arc::make_mut(&mut self.0);
			*buffer.id.get_mut() = 0;
			buffer.png_data_url = OnceLock::new();
			*buffer.derived.get_mut().unwrap_or_else(PoisonError::into_inner) = None;
			&mut buffer.image
		}
	}

	impl<'de> serde::Deserialize<'de> for Raster<CPU> {
		fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
		where
			D: serde::Deserializer<'de>,
		{
			Ok(Raster::new_cpu(Image::deserialize(deserializer)?))
		}
	}

	impl serde::Serialize for Raster<CPU> {
		fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
		where
			S: serde::Serializer,
		{
			self.data().serialize(serializer)
		}
	}

	#[cfg(test)]
	mod tests {
		use super::*;

		#[test]
		fn clones_share_pixels_until_one_is_modified() {
			let original = Raster::new_cpu(Image::new(2, 2, Color::BLACK));
			let mut copy = original.clone();
			assert_eq!(copy.buffer_id(), original.buffer_id());
			assert!(Arc::ptr_eq(&copy.storage.0, &original.storage.0));

			// Modifying the copy leaves the original's pixels and ID untouched
			copy.data_mut().data[0] = Color::WHITE;
			assert_ne!(copy.buffer_id(), original.buffer_id());
			assert_eq!(original.data().data[0], Color::BLACK);
			assert_eq!(copy.data().data[0], Color::WHITE);
		}

		#[test]
		fn modifying_unshared_pixels_takes_a_new_id_in_place() {
			let mut raster = Raster::new_cpu(Image::new(2, 2, Color::BLACK));
			let (id, pixels) = (raster.buffer_id(), raster.data().data.as_ptr());

			raster.data_mut().data[0] = Color::WHITE;
			assert_ne!(raster.buffer_id(), id, "caches keyed by the old ID must not see the new pixels");
			assert_eq!(raster.data().data.as_ptr(), pixels, "pixels nothing else shares are modified without a copy");
		}

		#[test]
		fn derived_pixels_are_reused_for_the_same_key_until_the_pixels_change() {
			let mut raster = Raster::new_cpu(Image::new(2, 2, Color::BLACK));
			let derive = |_: &Raster<CPU>| Raster::new_cpu(Image::new(1, 1, Color::WHITE));

			let first = raster.derived(1, derive);
			let again = raster.clone().derived(1, |_| panic!("the pixels derived for this key are kept"));
			assert_eq!(again.buffer_id(), first.buffer_id(), "reused pixels keep their ID");

			let second = raster.derived(2, derive);
			assert_ne!(second.buffer_id(), first.buffer_id(), "another key derives anew");

			raster.data_mut().data[0] = Color::WHITE;
			assert_ne!(raster.derived(2, derive).buffer_id(), second.buffer_id(), "modified pixels derive anew");
		}

		#[test]
		fn equal_pixels_compare_equal_whatever_their_ids() {
			let first = Raster::new_cpu(Image::new(2, 2, Color::BLACK));
			let second = Raster::new_cpu(Image::new(2, 2, Color::BLACK));

			assert_ne!(first.buffer_id(), second.buffer_id());
			assert_eq!(first, second);
		}
	}
}

pub use gpu::GPU;
#[cfg(feature = "wgpu")]
pub use gpu::{Texture, TextureWeakRef};

#[cfg(feature = "wgpu")]
mod gpu {
	use super::*;
	use crate::raster_types::__private::Sealed;
	use std::sync::Arc;

	#[derive(Clone, Debug, PartialEq, Eq, Hash, DynAny)]
	pub struct Texture(Arc<TextureInner>);

	#[derive(Debug, PartialEq, Eq, Hash)]
	struct TextureInner(wgpu::Texture);

	impl Drop for TextureInner {
		fn drop(&mut self) {
			self.0.destroy();
		}
	}

	impl Texture {
		pub fn is_shared(&self) -> bool {
			Arc::strong_count(&self.0) > 1
		}

		pub fn is_weakly_shared(&self) -> bool {
			Arc::weak_count(&self.0) > 0
		}

		pub fn downgrade(&self) -> TextureWeakRef {
			TextureWeakRef(Arc::downgrade(&self.0))
		}
	}

	#[derive(Clone, Debug)]
	pub struct TextureWeakRef(std::sync::Weak<TextureInner>);

	impl TextureWeakRef {
		pub fn upgrade(&self) -> Option<Texture> {
			self.0.upgrade().map(Texture)
		}
	}

	impl Deref for Texture {
		type Target = wgpu::Texture;

		fn deref(&self) -> &Self::Target {
			&self.0.0
		}
	}

	impl AsRef<wgpu::Texture> for Texture {
		fn as_ref(&self) -> &wgpu::Texture {
			&self.0.0
		}
	}

	impl From<wgpu::Texture> for Texture {
		fn from(texture: wgpu::Texture) -> Self {
			Self(Arc::new(TextureInner(texture)))
		}
	}

	impl core_types::CacheHash for Texture {
		fn cache_hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
			use ::core::hash::Hash;
			self.hash(state);
		}
	}

	#[derive(Clone, Debug, PartialEq, Hash)]
	pub struct GPU {
		pub texture: Texture,
	}

	impl core_types::CacheHash for GPU {
		fn cache_hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
			use ::core::hash::Hash;
			self.texture.hash(state);
		}
	}

	impl Sealed for Raster<GPU> {}

	impl Storage for Raster<GPU> {
		fn is_empty(&self) -> bool {
			self.texture.width() == 0 || self.texture.height() == 0
		}
	}

	impl Raster<GPU> {
		pub fn new_gpu(texture: impl Into<Texture>) -> Self {
			Self::new(GPU { texture: texture.into() })
		}

		pub fn data(&self) -> &wgpu::Texture {
			&self.texture
		}
	}
}

#[cfg(not(feature = "wgpu"))]
mod gpu {
	use super::*;
	use crate::raster_types::__private::Sealed;

	#[derive(Clone, Debug, PartialEq, Hash, core_types::CacheHash)]
	pub struct GPU;

	impl Sealed for Raster<GPU> {}

	impl Storage for Raster<GPU> {
		fn is_empty(&self) -> bool {
			true
		}
	}
}

mod gpu_common {
	use super::*;

	impl<'de> serde::Deserialize<'de> for Raster<GPU> {
		fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
		where
			D: serde::Deserializer<'de>,
		{
			unimplemented!()
		}
	}

	impl serde::Serialize for Raster<GPU> {
		fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
		where
			S: serde::Serializer,
		{
			unimplemented!()
		}
	}
}

impl<T> BoundingBox for Raster<T>
where
	Raster<T>: Storage,
{
	fn bounding_box(&self, transform: DAffine2, _include_stroke: bool) -> RenderBoundingBox {
		if self.is_empty() || transform.matrix2.determinant() == 0. {
			return RenderBoundingBox::None;
		}

		let unit_rectangle = Quad::from_box([DVec2::ZERO, DVec2::ONE]);
		RenderBoundingBox::Rectangle((transform * unit_rectangle).bounding_box())
	}

	fn thumbnail_bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		self.bounding_box(transform, include_stroke)
	}
}
