use glam::DVec2;

/// Structure used to represent a single anchor with up to two optional associated handles along a `Subpath`
pub struct ManipulatorGroup {
	pub anchor: DVec2,
	pub in_handle: Option<DVec2>,
	pub out_handle: Option<DVec2>,
}
