use crate::graphic::Graphic;
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::graphene_hash::CacheHash;
use core_types::list::List;
use core_types::render_complexity::RenderComplexity;
use dyn_any::DynAny;
use glam::DAffine2;

/// Nominal wrapper around `List<Graphic>` representing a single artboard's content.
///
/// Per-artboard metadata (location, dimensions, background, clip) lives as attributes on the
/// enclosing `List<Artboard>`, not as fields here. This keeps `Artboard` a pure type-system boundary
/// that prevents arbitrary `List<List<...<Graphic>>>` nesting.
#[derive(Clone, Debug, Default, CacheHash, PartialEq, DynAny)]
pub struct Artboard(List<Graphic>);

impl Artboard {
	pub fn new(content: List<Graphic>) -> Self {
		Self(content)
	}

	pub fn as_graphic_list(&self) -> &List<Graphic> {
		&self.0
	}

	pub fn as_graphic_list_mut(&mut self) -> &mut List<Graphic> {
		&mut self.0
	}

	pub fn into_graphic_list(self) -> List<Graphic> {
		self.0
	}

	/// The artboard with every content group converted to its legacy form, so
	/// the value owns all of its content free of arena borrows.
	pub fn with_legacy_groups(&self) -> Artboard {
		let mut content = self.0.clone();
		for element in content.iter_element_values_mut() {
			*element = crate::graphic::map_groups_to_legacy(element);
		}
		Artboard(content)
	}
}

/// The deep copy-out for `Artboard` elements: as for `Graphic`, a plain
/// clone of group content would carry frame pointers into the evaluation's
/// arena, so memo and capture seams copy out the owned-group form.
///
/// # Safety
/// `ptr` must point at a live parked `Artboard` element field.
unsafe fn deep_clone_artboard(ptr: *const u8) -> Box<dyn std::any::Any + Send + Sync> {
	let artboard = unsafe { core_types::record::borrow_element::<Artboard>(core_types::record::Rec::new(ptr)) };
	let mut content = artboard.0.clone();
	for element in content.iter_element_values_mut() {
		*element = crate::graphic::map_groups_to_owned(element);
	}
	Box::new(Artboard(content))
}

/// The deep re-park for `Artboard` elements: owned content groups replay into
/// the serving arena before the artboard parks.
///
/// # Safety
/// `value` must hold an `Artboard` and `dst` must be a live `Artboard`
/// element field.
unsafe fn deep_repark_artboard(value: &(dyn std::any::Any + Send + Sync), dst: *mut u8, arena: &core_types::arena::Arena) -> Option<()> {
	let artboard = value.downcast_ref::<Artboard>().expect("an element replays at its own type");
	let mut content = artboard.0.clone();
	for element in content.iter_element_values_mut() {
		*element = crate::graphic::map_groups_to_resident(element, arena)?;
	}
	unsafe { core_types::record::write_element(dst, Artboard(content), arena) }
}

const _: () = {
	#[cfg(not(target_family = "wasm"))]
	#[core_types::ctor::ctor]
	fn register() {
		core_types::record::register_deep_element_clone::<Artboard>(deep_clone_artboard, deep_repark_artboard);
	}

	#[cfg(target_family = "wasm")]
	#[unsafe(export_name = "__node_registry_deep_element_artboard")]
	extern "C" fn register() {
		core_types::record::register_deep_element_clone::<Artboard>(deep_clone_artboard, deep_repark_artboard);
	}
};

impl From<List<Graphic>> for Artboard {
	fn from(content: List<Graphic>) -> Self {
		Self(content)
	}
}

impl From<Artboard> for List<Graphic> {
	fn from(artboard: Artboard) -> Self {
		artboard.0
	}
}

impl BoundingBox for Artboard {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		self.0.bounding_box(transform, include_stroke)
	}

	fn thumbnail_bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		self.0.thumbnail_bounding_box(transform, include_stroke)
	}
}

impl RenderComplexity for Artboard {
	fn render_complexity(&self) -> usize {
		self.0.render_complexity()
	}
}
