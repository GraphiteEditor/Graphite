use crate::messages::prelude::*;
use crate::messages::tool::tool_messages::tool_prelude::DVec2;

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type, ExtractField)]
pub struct ViewportMessageHandler {
	bounds: LogicalBounds,
	// Ratio of logical pixels to physical pixels
	scale: f64,
}
impl Default for ViewportMessageHandler {
	fn default() -> Self {
		Self {
			bounds: LogicalBounds {
				x: 0.0,
				y: 0.0,
				width: 0.0,
				height: 0.0,
			},
			scale: 1.0,
		}
	}
}

#[message_handler_data]
impl MessageHandler<ViewportMessage, ()> for ViewportMessageHandler {
	fn process_message(&mut self, message: ViewportMessage, responses: &mut VecDeque<Message>, _: ()) {
		match message {
			ViewportMessage::UpdateScale { scale } => {
				assert_ne!(scale, 0.0, "Viewport scale cannot be zero");
				self.scale = scale;
			}
			ViewportMessage::UpdateBounds { x, y, width, height } => {
				self.bounds = LogicalBounds { x, y, width, height };
			}
			ViewportMessage::Trigger => {}
		}

		responses.add(NavigationMessage::CanvasPan { delta: DVec2::ZERO });
		responses.add(NodeGraphMessage::SetGridAlignedEdges);

		responses.add(DeferMessage::AfterGraphRun {
			messages: vec![
				DeferMessage::AfterGraphRun {
					messages: vec![DeferMessage::TriggerNavigationReady.into()],
				}
				.into(),
			],
		});
	}

	advertise_actions!(ViewportMessageDiscriminant;);
}

impl ViewportMessageHandler {
	pub fn logical_bounds(&self) -> LogicalBounds {
		self.bounds
	}

	pub fn physical_bounds(&self) -> PhysicalBounds {
		self.logical_bounds().to_physical(self.scale)
	}

	pub fn logical_offset(&self) -> LogicalPoint {
		LogicalPoint { x: self.bounds.x, y: self.bounds.y }
	}

	pub fn physical_offset(&self) -> PhysicalPoint {
		self.logical_offset().to_physical(self.scale)
	}

	pub fn logical_size(&self) -> LogicalPoint {
		LogicalPoint {
			x: self.bounds.width,
			y: self.bounds.height,
		}
	}

	pub fn physical_size(&self) -> PhysicalPoint {
		self.logical_size().to_physical(self.scale)
	}

	pub fn logical_center_in_viewport_space(&self) -> LogicalPoint {
		let logical_size = self.logical_size();
		LogicalPoint {
			x: logical_size.x / 2.0,
			y: logical_size.y / 2.0,
		}
	}

	pub fn physical_center_in_viewport_space(&self) -> PhysicalPoint {
		self.logical_center_in_viewport_space().to_physical(self.scale)
	}

	pub fn logical_center_in_window_space(&self) -> LogicalPoint {
		self.apply_offset_to_logical_point(self.logical_center_in_viewport_space())
	}

	pub fn physical_center_in_window_space(&self) -> PhysicalPoint {
		self.logical_center_in_window_space().to_physical(self.scale)
	}

	pub fn in_logical_bounds<T: Into<LogicalPoint>>(&self, point: T) -> bool {
		let point = point.into();
		point.x >= self.bounds.x && point.y >= self.bounds.y && point.x <= self.bounds.x + self.bounds.width && point.y <= self.bounds.y + self.bounds.height
	}

	pub fn in_physical_bounds<T: Into<PhysicalPoint>>(&self, point: T) -> bool {
		let point = self.convert_physical_to_logical_point(point.into());
		self.in_logical_bounds(point)
	}

	pub fn convert_physical_to_logical(&self, physical: f64) -> f64 {
		physical.to_logical(self.scale)
	}

	pub fn convert_logical_to_physical(&self, logical: f64) -> f64 {
		logical.to_physical(self.scale)
	}

	pub fn convert_physical_to_logical_point<T: Into<PhysicalPoint>>(&self, physical: T) -> LogicalPoint {
		physical.into().to_logical(self.scale)
	}

	pub fn convert_logical_to_physical_point<T: Into<LogicalPoint>>(&self, logical: T) -> PhysicalPoint {
		logical.into().to_physical(self.scale)
	}

	pub fn convert_physical_to_logical_bounds<T: Into<PhysicalBounds>>(&self, physical: T) -> LogicalBounds {
		physical.into().to_logical(self.scale)
	}

	pub fn convert_logical_to_physical_bounds<T: Into<LogicalBounds>>(&self, logical: T) -> PhysicalBounds {
		logical.into().to_physical(self.scale)
	}

	pub fn apply_offset_to_logical_point<T: Into<LogicalPoint>>(&self, logical: T) -> LogicalPoint {
		let logical = logical.into();
		let offset = self.logical_offset();
		LogicalPoint {
			x: logical.x + offset.x,
			y: logical.y + offset.y,
		}
	}

	pub fn apply_offset_to_physical_point<T: Into<PhysicalPoint>>(&self, physical: T) -> PhysicalPoint {
		let physical = physical.into();
		let offset = self.physical_offset();
		PhysicalPoint {
			x: physical.x + offset.x,
			y: physical.y + offset.y,
		}
	}

	pub fn remove_offset_from_logical_point<T: Into<LogicalPoint>>(&self, logical: T) -> LogicalPoint {
		let logical = logical.into();
		let offset = self.logical_offset();
		LogicalPoint {
			x: logical.x - offset.x,
			y: logical.y - offset.y,
		}
	}

	pub fn remove_offset_from_physical_point<T: Into<PhysicalPoint>>(&self, physical: T) -> PhysicalPoint {
		let physical = physical.into();
		let offset = self.physical_offset();
		PhysicalPoint {
			x: physical.x - offset.x,
			y: physical.y - offset.y,
		}
	}

	pub fn convert_logical_window_point_to_physical_viewport_point<T: Into<LogicalPoint>>(&self, logical: T) -> PhysicalPoint {
		let physical_point = self.convert_logical_to_physical_point(logical);
		self.apply_offset_to_physical_point(physical_point)
	}

	pub fn convert_physical_window_point_to_logical_viewport_point<T: Into<PhysicalPoint>>(&self, physical: T) -> LogicalPoint {
		let logical_point = self.convert_physical_to_logical_point(physical);
		self.apply_offset_to_logical_point(logical_point)
	}

	pub fn convert_logical_viewport_point_to_physical_window_point<T: Into<LogicalPoint>>(&self, offset_logical: T) -> PhysicalPoint {
		let logical = self.remove_offset_from_logical_point(offset_logical);
		self.convert_logical_to_physical_point(logical)
	}

	pub fn convert_physical_viewport_point_to_logical_window_point<T: Into<PhysicalPoint>>(&self, offset_physical: T) -> LogicalPoint {
		let physical = self.remove_offset_from_physical_point(offset_physical);
		self.convert_physical_to_logical_point(physical)
	}
}

trait ToPhysical<P: ToLogical<Self> + ?Sized> {
	fn to_physical(self, scale: f64) -> P;
}
impl ToPhysical<f64> for f64 {
	fn to_physical(self, scale: f64) -> f64 {
		assert_ne!(scale, 0.0, "Cannot convert to physical with a scale of zero");
		self * scale
	}
}

trait ToLogical<L: ToPhysical<Self> + ?Sized> {
	fn to_logical(self, scale: f64) -> L;
}
impl ToLogical<f64> for f64 {
	fn to_logical(self, scale: f64) -> f64 {
		assert_ne!(scale, 0.0, "Cannot convert to logical with a scale of zero");
		self / scale
	}
}

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct LogicalPoint {
	pub x: f64,
	pub y: f64,
}
impl ToPhysical<PhysicalPoint> for LogicalPoint {
	fn to_physical(self, scale: f64) -> PhysicalPoint {
		PhysicalPoint {
			x: self.x.to_physical(scale),
			y: self.y.to_physical(scale),
		}
	}
}

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct PhysicalPoint {
	pub x: f64,
	pub y: f64,
}
impl ToLogical<LogicalPoint> for PhysicalPoint {
	fn to_logical(self, scale: f64) -> LogicalPoint {
		LogicalPoint {
			x: self.x.to_logical(scale),
			y: self.y.to_logical(scale),
		}
	}
}

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct LogicalBounds {
	pub x: f64,
	pub y: f64,
	pub width: f64,
	pub height: f64,
}
impl ToPhysical<PhysicalBounds> for LogicalBounds {
	fn to_physical(self, scale: f64) -> PhysicalBounds {
		PhysicalBounds {
			x: self.x.to_physical(scale),
			y: self.y.to_physical(scale),
			width: self.width.to_physical(scale),
			height: self.height.to_physical(scale),
		}
	}
}

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct PhysicalBounds {
	pub x: f64,
	pub y: f64,
	pub width: f64,
	pub height: f64,
}
impl ToLogical<LogicalBounds> for PhysicalBounds {
	fn to_logical(self, scale: f64) -> LogicalBounds {
		LogicalBounds {
			x: self.x.to_logical(scale),
			y: self.y.to_logical(scale),
			width: self.width.to_logical(scale),
			height: self.height.to_logical(scale),
		}
	}
}

impl From<(f64, f64)> for LogicalPoint {
	fn from((x, y): (f64, f64)) -> Self {
		Self { x, y }
	}
}
impl From<(f64, f64)> for PhysicalPoint {
	fn from((x, y): (f64, f64)) -> Self {
		Self { x, y }
	}
}
impl From<(f64, f64, f64, f64)> for LogicalBounds {
	fn from((x, y, width, height): (f64, f64, f64, f64)) -> Self {
		Self { x, y, width, height }
	}
}
impl From<(f64, f64, f64, f64)> for PhysicalBounds {
	fn from((x, y, width, height): (f64, f64, f64, f64)) -> Self {
		Self { x, y, width, height }
	}
}

impl From<LogicalPoint> for (f64, f64) {
	fn from(point: LogicalPoint) -> Self {
		(point.x, point.y)
	}
}
impl From<PhysicalPoint> for (f64, f64) {
	fn from(point: PhysicalPoint) -> Self {
		(point.x, point.y)
	}
}
impl From<LogicalBounds> for (f64, f64, f64, f64) {
	fn from(bounds: LogicalBounds) -> Self {
		(bounds.x, bounds.y, bounds.width, bounds.height)
	}
}
impl From<PhysicalBounds> for (f64, f64, f64, f64) {
	fn from(bounds: PhysicalBounds) -> Self {
		(bounds.x, bounds.y, bounds.width, bounds.height)
	}
}

impl From<glam::DVec2> for LogicalPoint {
	fn from(vec: glam::DVec2) -> Self {
		Self { x: vec.x, y: vec.y }
	}
}
impl From<glam::DVec2> for PhysicalPoint {
	fn from(vec: glam::DVec2) -> Self {
		Self { x: vec.x, y: vec.y }
	}
}
impl From<LogicalPoint> for glam::DVec2 {
	fn from(val: LogicalPoint) -> Self {
		glam::DVec2::new(val.x, val.y)
	}
}
impl From<PhysicalPoint> for glam::DVec2 {
	fn from(val: PhysicalPoint) -> Self {
		glam::DVec2::new(val.x, val.y)
	}
}

impl From<[glam::DVec2; 2]> for LogicalBounds {
	fn from(bounds: [glam::DVec2; 2]) -> Self {
		Self {
			x: bounds[0].x,
			y: bounds[0].y,
			width: bounds[1].x - bounds[0].x,
			height: bounds[1].y - bounds[0].y,
		}
	}
}
impl From<[glam::DVec2; 2]> for PhysicalBounds {
	fn from(bounds: [glam::DVec2; 2]) -> Self {
		Self {
			x: bounds[0].x,
			y: bounds[0].y,
			width: bounds[1].x - bounds[0].x,
			height: bounds[1].y - bounds[0].y,
		}
	}
}
impl From<LogicalBounds> for [glam::DVec2; 2] {
	fn from(bounds: LogicalBounds) -> Self {
		[glam::DVec2::new(bounds.x, bounds.y), glam::DVec2::new(bounds.x + bounds.width, bounds.y + bounds.height)]
	}
}
impl From<PhysicalBounds> for [glam::DVec2; 2] {
	fn from(bounds: PhysicalBounds) -> Self {
		[glam::DVec2::new(bounds.x, bounds.y), glam::DVec2::new(bounds.x + bounds.width, bounds.y + bounds.height)]
	}
}

impl LogicalPoint {
	pub fn into_dvec2(self) -> DVec2 {
		DVec2::new(self.x, self.y)
	}
}
impl PhysicalPoint {
	pub fn into_dvec2(self) -> DVec2 {
		DVec2::new(self.x, self.y)
	}
}
