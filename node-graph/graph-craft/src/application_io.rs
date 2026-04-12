use dyn_any::StaticType;

#[cfg(not(target_family = "wasm"))]
mod native;
#[cfg(target_family = "wasm")]
mod wasm;

#[cfg(not(target_family = "wasm"))]
pub type PlatformApplicationIo = native::NativeApplicationIo;
#[cfg(target_family = "wasm")]
pub type PlatformApplicationIo = wasm::WasmApplicationIo;

pub type PlatformEditorApi = graphene_application_io::EditorApi<PlatformApplicationIo>;

static WGPU_AVAILABLE: std::sync::atomic::AtomicI8 = std::sync::atomic::AtomicI8::new(-1);

/// Returns:
/// - `None` if the availability of WGPU has not been determined yet
/// - `Some(true)` if WGPU is available
/// - `Some(false)` if WGPU is not available
pub fn wgpu_available() -> Option<bool> {
	match WGPU_AVAILABLE.load(std::sync::atomic::Ordering::SeqCst) {
		-1 => None,
		0 => Some(false),
		_ => Some(true),
	}
}

pub(crate) fn set_wgpu_available(available: bool) {
	WGPU_AVAILABLE.store(available as i8, std::sync::atomic::Ordering::SeqCst);
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EditorPreferences {
	/// Maximum render region size in pixels along one dimension of the square area.
	pub max_render_region_size: u32,
}

impl graphene_application_io::GetEditorPreferences for EditorPreferences {
	fn max_render_region_area(&self) -> u32 {
		let size = self.max_render_region_size.min(u32::MAX.isqrt());
		size.pow(2)
	}
}

impl Default for EditorPreferences {
	fn default() -> Self {
		Self { max_render_region_size: 1280 }
	}
}

unsafe impl StaticType for EditorPreferences {
	type Static = EditorPreferences;
}
