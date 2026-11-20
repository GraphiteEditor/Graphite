//! Tints what a step changed onto a close-up of the document, colored by how the layer under each changed pixel changed.

use super::utility_types::Pixels;
use glam::{DVec2, UVec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
	Added,
	Removed,
	Changed,
}

/// Where a layer's change shows, in document coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChangedArea {
	pub change: Change,
	pub bounds: [DVec2; 2],
}

/// How far apart two premultiplied channels must be for a pixel to count as changed, out of 255.
const THRESHOLD: u8 = 16;
/// How much of a tint's color covers a changed pixel.
const TINT_STRENGTH: f32 = 0.6;
/// How opaque unchanged pixels stay, so the tints stand out.
const UNCHANGED_OPACITY: f32 = 0.35;

fn tint_color(change: Change) -> [u8; 3] {
	match change {
		Change::Added => [64, 200, 96],
		Change::Removed => [232, 64, 64],
		Change::Changed => [240, 176, 32],
	}
}

/// The close-up `after` of `frame` with each pixel that differs from `before`, the whole document before the step
/// showing `before_frame`, tinted by the change of the area it falls in. A pixel that lost coverage shows as it was.
pub fn tint_changes(after: &Pixels, frame: [DVec2; 2], before: &Pixels, before_frame: [DVec2; 2], areas: &[ChangedArea]) -> Pixels {
	let size = after.size;
	let mut rgba = Vec::with_capacity(after.rgba.len());
	for y in 0..size.y {
		for x in 0..size.x {
			let point = frame[0] + (DVec2::new(x as f64, y as f64) + 0.5) / size.as_dvec2() * (frame[1] - frame[0]);
			let now = pixel(after, UVec2::new(x, y));
			let then = sample(before, before_frame, point);
			let shown = if differs(now, then) {
				let lost = now[3] < then[3];
				tint(if lost { then } else { now }, change_at(areas, point, lost))
			} else {
				[now[0], now[1], now[2], (now[3] as f32 * UNCHANGED_OPACITY) as u8]
			};
			rgba.extend_from_slice(&shown);
		}
	}
	Pixels { size, rgba }
}

/// The area around every change, with a margin, at the thumbnails' aspect ratio. None when nothing changed.
pub fn close_up(areas: &[ChangedArea]) -> Option<[DVec2; 2]> {
	/// The thumbnails' width over their height.
	const ASPECT_RATIO: f64 = 1.5;

	let [min, max] = areas.iter().map(|area| area.bounds).reduce(|a, b| [a[0].min(b[0]), a[1].max(b[1])])?;
	let margin = ((max - min) * 0.15).max(DVec2::splat(4.));
	let (min, max) = (min - margin, max + margin);

	let size = max - min;
	let size = if size.x >= size.y * ASPECT_RATIO {
		DVec2::new(size.x, size.x / ASPECT_RATIO)
	} else {
		DVec2::new(size.y * ASPECT_RATIO, size.y)
	};
	let center = (min + max) / 2.;
	Some([center - size / 2., center + size / 2.])
}

/// The change of the areas holding `point`, preferring a removal where the pixel lost coverage and an addition elsewhere.
fn change_at(areas: &[ChangedArea], point: DVec2, lost: bool) -> Change {
	let holding = |change: Change| areas.iter().any(|area| area.change == change && contains(area.bounds, point));
	let order = match lost {
		true => [Change::Removed, Change::Changed, Change::Added],
		false => [Change::Added, Change::Changed, Change::Removed],
	};
	order.into_iter().find(|&change| holding(change)).unwrap_or(Change::Changed)
}

fn contains(bounds: [DVec2; 2], point: DVec2) -> bool {
	point.cmpge(bounds[0]).all() && point.cmple(bounds[1]).all()
}

fn pixel(pixels: &Pixels, at: UVec2) -> [u8; 4] {
	let index = (at.y * pixels.size.x + at.x) as usize * 4;
	pixels.rgba.get(index..index + 4).and_then(|pixel| pixel.try_into().ok()).unwrap_or([0; 4])
}

/// The pixel of `pixels`, which shows `frame`, at a document point, transparent outside it.
fn sample(pixels: &Pixels, frame: [DVec2; 2], point: DVec2) -> [u8; 4] {
	let at = (point - frame[0]) / (frame[1] - frame[0]) * pixels.size.as_dvec2();
	if at.cmplt(DVec2::ZERO).any() || at.cmpge(pixels.size.as_dvec2()).any() {
		return [0; 4];
	}
	pixel(pixels, at.as_uvec2())
}

fn differs(a: [u8; 4], b: [u8; 4]) -> bool {
	let premultiplied = |pixel: [u8; 4]| pixel.map(|channel| (channel as u16 * pixel[3] as u16 / 255) as u8);
	let (a, b) = (premultiplied(a), premultiplied(b));
	a.iter().zip(&b).any(|(a, b)| a.abs_diff(*b) > THRESHOLD) || a[3].abs_diff(b[3]) > THRESHOLD
}

fn tint(pixel: [u8; 4], change: Change) -> [u8; 4] {
	let color = tint_color(change);
	let mix = |channel: u8, tint: u8| (channel as f32 + (tint as f32 - channel as f32) * TINT_STRENGTH) as u8;
	[mix(pixel[0], color[0]), mix(pixel[1], color[1]), mix(pixel[2], color[2]), pixel[3]]
}

#[cfg(test)]
mod tests {
	use super::*;

	const FRAME: [DVec2; 2] = [DVec2::ZERO, DVec2::new(4., 2.)];
	const CLEAR: [u8; 4] = [0; 4];
	const WHITE: [u8; 4] = [255; 4];

	/// A 4×2 image of the given pixels, row by row.
	fn image(pixels: [[u8; 4]; 8]) -> Pixels {
		Pixels {
			size: UVec2::new(4, 2),
			rgba: pixels.concat(),
		}
	}

	fn left_half(change: Change) -> ChangedArea {
		ChangedArea {
			change,
			bounds: [DVec2::ZERO, DVec2::new(2., 2.)],
		}
	}

	#[test]
	fn what_stayed_fades_and_what_came_is_tinted_as_added() {
		let before = image([CLEAR; 8]);
		let after = image([WHITE, WHITE, CLEAR, CLEAR, WHITE, WHITE, CLEAR, CLEAR]);
		let tinted = tint_changes(&after, FRAME, &before, FRAME, &[left_half(Change::Added)]);

		assert_eq!(pixel(&tinted, UVec2::new(0, 0)), tint(WHITE, Change::Added));
		assert_eq!(pixel(&tinted, UVec2::new(3, 1))[3], 0, "transparent pixels that stayed stay transparent");

		let unchanged = tint_changes(&after, FRAME, &after, FRAME, &[]);
		assert_eq!(pixel(&unchanged, UVec2::new(0, 0))[3], (255. * UNCHANGED_OPACITY) as u8);
	}

	#[test]
	fn what_went_shows_as_it_was_tinted_as_removed() {
		let before = image([WHITE, WHITE, CLEAR, CLEAR, WHITE, WHITE, CLEAR, CLEAR]);
		let after = image([CLEAR; 8]);
		let tinted = tint_changes(&after, FRAME, &before, FRAME, &[left_half(Change::Removed)]);

		let removed = pixel(&tinted, UVec2::new(1, 1));
		assert_eq!(removed, tint(WHITE, Change::Removed));
		assert_eq!(removed[3], 255, "a removed pixel keeps the coverage it had");
	}

	#[test]
	fn a_moved_layer_is_tinted_as_changed_where_it_was_and_where_it_is() {
		let before = image([WHITE, CLEAR, CLEAR, CLEAR, CLEAR, CLEAR, CLEAR, CLEAR]);
		let after = image([CLEAR, WHITE, CLEAR, CLEAR, CLEAR, CLEAR, CLEAR, CLEAR]);
		let tinted = tint_changes(&after, FRAME, &before, FRAME, &[left_half(Change::Changed)]);

		assert_eq!(pixel(&tinted, UVec2::new(0, 0)), tint(WHITE, Change::Changed), "where it was");
		assert_eq!(pixel(&tinted, UVec2::new(1, 0)), tint(WHITE, Change::Changed), "where it is");
	}

	#[test]
	fn the_close_up_samples_the_whole_document_before_at_its_own_scale() {
		// The before image shows twice the area, so its left pixel column covers the close-up's left two
		let before = image([WHITE, CLEAR, CLEAR, CLEAR, WHITE, CLEAR, CLEAR, CLEAR]);
		let before_frame = [DVec2::ZERO, DVec2::new(8., 4.)];
		let after = image([WHITE, WHITE, CLEAR, CLEAR, WHITE, WHITE, CLEAR, CLEAR]);
		let tinted = tint_changes(&after, FRAME, &before, before_frame, &[]);

		assert_eq!(pixel(&tinted, UVec2::new(1, 0))[3], (255. * UNCHANGED_OPACITY) as u8, "still covered, so unchanged");
		assert_eq!(pixel(&tinted, UVec2::new(2, 0))[3], 0);
	}
}
