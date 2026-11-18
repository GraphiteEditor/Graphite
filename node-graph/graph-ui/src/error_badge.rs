use crate::metrics::{ERROR_LINE_HEIGHT, ERROR_MAX_WIDTH, ERROR_PADDING, ERROR_TAIL_LEFT, ERROR_TAIL_SIZE, GRID_SIZE, LAYER_LEFT_MARGIN};
use crate::replicated_map::Keyed;
use crate::text;
use core_types::uuid::NodeId;
use glam::{DVec2, IVec2};
use graphene_hash::CacheHash;

#[derive(Clone, Debug, PartialEq, CacheHash)]
pub struct ErrorBadge {
	pub key: NodeId,
	pub cell: IVec2,
	pub on_layer: bool,
	pub text: String,
	pub hovered: bool,
	pub glide: DVec2,
}

impl Keyed for ErrorBadge {
	type Key = NodeId;

	fn key(&self) -> NodeId {
		self.key
	}
}

impl ErrorBadge {
	fn anchor(&self) -> DVec2 {
		let offset = if self.on_layer { DVec2::new(LAYER_LEFT_MARGIN, -GRID_SIZE / 2.) } else { DVec2::ZERO };
		self.cell.as_dvec2() * GRID_SIZE + offset + self.glide
	}

	pub fn text_size(&self) -> DVec2 {
		text::paragraph_size(&self.text, ERROR_LINE_HEIGHT, ERROR_MAX_WIDTH)
	}

	pub fn body(&self, text_size: DVec2) -> [DVec2; 2] {
		let bottom_left = self.anchor();
		let size = text_size + 2. * ERROR_PADDING;
		[bottom_left - DVec2::Y * size.y, bottom_left + DVec2::X * size.x]
	}

	pub fn text_top_left(&self, text_size: DVec2) -> DVec2 {
		let [top_left, _] = self.body(text_size);
		top_left + ERROR_PADDING
	}

	pub fn tail(&self) -> [DVec2; 3] {
		let top_left = self.anchor() + DVec2::X * ERROR_TAIL_LEFT;
		[top_left, top_left + DVec2::X * ERROR_TAIL_SIZE.x, top_left + DVec2::new(ERROR_TAIL_SIZE.x / 2., ERROR_TAIL_SIZE.y)]
	}

	pub fn covers(&self, point: DVec2, text_size: DVec2) -> bool {
		let [tail, ..] = self.tail();
		let within = |[min, max]: [DVec2; 2]| min.cmple(point).all() && point.cmple(max).all();
		within(self.body(text_size)) || within([tail, tail + ERROR_TAIL_SIZE])
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::test_rows::badge;

	#[test]
	fn a_badge_stands_half_a_cell_above_its_card_or_above_the_thumbnail_of_its_layer() {
		let badge = ErrorBadge {
			cell: IVec2::new(13, 22),
			..badge(0, "")
		};
		assert_eq!(badge.anchor(), DVec2::new(312., 528.));
		let on_layer = ErrorBadge { on_layer: true, ..badge };
		assert_eq!(on_layer.anchor(), DVec2::new(324., 516.));
	}

	#[test]
	fn a_badge_grows_up_and_right_from_its_anchor_with_its_tail_below() {
		let badge = ErrorBadge {
			cell: IVec2::new(2, 1),
			..badge(0, "")
		};
		let text_size = DVec2::new(100., 36.);
		assert_eq!(badge.body(text_size), [DVec2::new(48., -28.), DVec2::new(164., 24.)]);
		assert_eq!(badge.text_top_left(text_size), DVec2::new(56., -20.), "the text is padded");
		assert_eq!(badge.tail(), [DVec2::new(54., 24.), DVec2::new(66., 24.), DVec2::new(60., 32.)]);
		let glide = DVec2::new(3., -4.);
		let gliding = ErrorBadge { glide, ..badge.clone() };
		assert_eq!(gliding.body(text_size), badge.body(text_size).map(|corner| corner + glide), "a gliding badge is drawn its glide away");
		assert_eq!(gliding.tail(), badge.tail().map(|corner| corner + glide), "its tail glides with it");
	}

	#[test]
	fn a_badge_covers_its_box_and_the_box_around_its_tail() {
		let badge = ErrorBadge {
			cell: IVec2::new(2, 1),
			..badge(0, "Error")
		};
		let text_size = badge.text_size();
		let covers = |point| badge.covers(point, text_size);
		let [min, max] = badge.body(text_size);
		assert!(covers(min) && covers(max - 1.) && covers((min + max) / 2.), "the box");
		assert!(covers(DVec2::new(55., 31.)), "beside the tip of the tail, inside the box around it");
		assert!(!covers(DVec2::new(52., 30.)), "left of the tail");
		assert!(!covers(max + 1.), "beyond the box");
	}
}
