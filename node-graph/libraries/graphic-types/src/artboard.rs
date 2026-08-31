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
pub struct Artboard<'e>(List<Graphic<'e>>);

impl<'e> Artboard<'e> {
	pub fn new(content: List<Graphic<'e>>) -> Self {
		Self(content)
	}

	pub fn as_graphic_list(&self) -> &List<Graphic> {
		&self.0
	}

	pub fn as_graphic_list_mut(&mut self) -> &mut List<Graphic<'e>> {
		&mut self.0
	}

	pub fn into_graphic_list(self) -> List<Graphic<'e>> {
		self.0
	}

	/// The artboard with every content group converted to its legacy form, so
	/// the value owns all of its content free of arena borrows.
	pub fn with_legacy_groups(&self) -> Artboard<'e> {
		let mut content = self.0.clone();
		for element in content.iter_element_values_mut() {
			*element = crate::graphic::map_groups_to_legacy(element);
		}
		crate::graphic::map_paint_attrs_to_legacy(&mut content);
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

/// The promote for `Artboard` elements: as for `Graphic`, content groups the
/// persistent region already holds are shared rather than copied.
///
/// # Safety
/// `src` must point at a live parked `Artboard` element field, and `dst` at
/// the element field the promoted reference is written to.
unsafe fn promote_artboard(src: *const u8, dst: *mut u8, promotion: &core_types::record::Promotion<'_>) -> Option<()> {
	let artboard = unsafe { core_types::record::borrow_element::<Artboard>(core_types::record::Rec::new(src)) };
	let mut content = List::new();
	for item in artboard.0.clone().into_iter() {
		let (element, attributes) = item.into_parts();
		content.push(core_types::list::Item::from_parts(crate::graphic::map_groups_to_persistent(&element, promotion)?, attributes));
	}
	unsafe { core_types::record::write_element(dst, Artboard(content), promotion.persistent()) }
}

const _: () = {
	fn register_all() {
		core_types::record::register_deep_element_clone::<Artboard>(deep_clone_artboard, deep_repark_artboard);
		core_types::record::register_element_promote::<Artboard>(promote_artboard);
	}

	#[cfg(not(target_family = "wasm"))]
	#[core_types::ctor::ctor]
	fn register() {
		register_all();
	}

	#[cfg(target_family = "wasm")]
	#[unsafe(export_name = "__node_registry_deep_element_artboard")]
	extern "C" fn register() {
		register_all();
	}
};

impl<'e> From<List<Graphic<'e>>> for Artboard<'e> {
	fn from(content: List<Graphic<'e>>) -> Self {
		Self(content)
	}
}

impl<'e> From<Artboard<'e>> for List<Graphic<'e>> {
	fn from(artboard: Artboard<'e>) -> Self {
		artboard.0
	}
}

impl BoundingBox for Artboard<'_> {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		self.0.bounding_box(transform, include_stroke)
	}

	fn thumbnail_bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		self.0.thumbnail_bounding_box(transform, include_stroke)
	}
}

impl RenderComplexity for Artboard<'_> {
	fn render_complexity(&self) -> usize {
		self.0.render_complexity()
	}
}
