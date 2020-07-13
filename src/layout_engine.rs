//! The layout engine converts the layout tree to rectangles of fixed sizes on the screen,
//! in order to know how to draw the various components.

use crate::layout_abstract_syntax::*;
use crate::layout_abstract_types::*;

/// Unit for offsets and sizes, expressed in pixels.
pub struct Pixel;

/// A 2D rectangular extent of pixels.
pub type Extent = euclid::Size2D<f64, Pixel>;

/// Constructs a new extent from values along an axis.
fn extent_from_axis_values(main_axis_size: f64, cross_axis_size: f64, axis: Axis) -> Extent {
	match axis {
		Axis::Horizontal => Extent::new(main_axis_size, cross_axis_size),
		Axis::Vertical => Extent::new(cross_axis_size, main_axis_size),
	}
}

/// Returns the length of the rectangle's sides along the main and the cross axis.
fn extent_sizes_along(extent: Extent, main_axis: Axis) -> (f64, f64) {
	match main_axis {
		Axis::Horizontal => (extent.width, extent.height),
		Axis::Vertical => (extent.height, extent.width),
	}
}

// ====================================================================================================

/// Recurisvely computes the layout of a node.
pub fn compute_layout(max_size: Extent, root: &LayoutComponentTag) {
	println!("Begin layout for {:?}", root.name);

	let (namespace, name) = &root.name;
	let main_axis = if namespace == "" && name == "col" { Axis::Vertical } else { Axis::Horizontal };
	let cross_axis = main_axis.perpendicular();

	let (max_main_axis_size, max_cross_axis_size) = extent_sizes_along(max_size, main_axis);

	// Assume the cross axis is always 100% for now
	let our_cross_axis_size = max_cross_axis_size;

	let children = match &root.content {
		Some(children) => children,
		None => {
			println!("End layout (no children)");
			return;
		},
	};

	// Go through all this node's children and partition them based on
	// what sort of sizing they use along the main axis.
	let mut fixed_size_children = Vec::new();
	let mut inner_size_children = Vec::new();
	let mut percent_size_children = Vec::new();
	let mut remainder_size_children = Vec::new();
	for child in children {
		let child_attribs = child.borrow().layout_attributes();
		match child_attribs.size_along(main_axis) {
			Dimension::AbsolutePx(size) => fixed_size_children.push((child, size)),
			Dimension::Inner => inner_size_children.push(child),
			Dimension::Percent(percent) => percent_size_children.push((child, percent)),
			Dimension::PercentRemainder(percent) => remainder_size_children.push((child, percent)),
			dimension => todo!("unsupported dimension type in layout: {:?}", dimension),
		}
		// TODO: on the cross axis, we'll need to expand to fit the children.
		assert_eq!(child_attribs.size_along(cross_axis), Dimension::Percent(100.0), "only 100% on cross axis supported for now");
	}

	// The minimum size we need along the main axis to lay out all of the children,
	// without overflowing.
	let mut min_main_axis_size = 0.0;

	println!("Laying out fixed size children");
	for (child, size) in fixed_size_children {
		let child_max_size = extent_from_axis_values(size, our_cross_axis_size, main_axis);
		match &*child.borrow() {
			LayoutComponentNode::Tag(tag) => compute_layout(child_max_size, tag),
			LayoutComponentNode::Text(_) => (),
		}
		println!("Laid out child with {:?} size {}", main_axis, size);
		min_main_axis_size += size;
	}

	println!("Laying out inner size children");
	for child in inner_size_children {
		let child_max_size = extent_from_axis_values(min_main_axis_size, our_cross_axis_size, main_axis);
		match &*child.borrow() {
			LayoutComponentNode::Tag(tag) => compute_layout(child_max_size, tag),
			LayoutComponentNode::Text(_) => (),
		}
		println!("Laid out child with {:?} size `inner`", main_axis);
	}

	// TODO: add padding to the axis.
	let our_main_axis_size = min_main_axis_size;

	println!("Laying out percent of parent size children");
	for (child, percent) in percent_size_children {
		let child_main_axis_size = percent * our_main_axis_size;
		let child_max_size = extent_from_axis_values(child_main_axis_size, our_cross_axis_size, main_axis);
		match &*child.borrow() {
			LayoutComponentNode::Tag(tag) => compute_layout(child_max_size, tag),
			LayoutComponentNode::Text(_) => (),
		}
	}

	// Check if we still fit in the parent container's defined size
	if min_main_axis_size > max_main_axis_size {
		todo!("main axis overflow is not supported yet")
	}

	// At the end, we lay out the elements which use the `@` specifier.
	// They split up the remaining free space.
	let free_space = max_main_axis_size - min_main_axis_size;
	let mut free_space_used = 0.0;

	println!("Laying out remaining space percent size children");
	for (child, percent) in remainder_size_children {
		let child_main_axis_size = percent * free_space / 100.0;
		let child_max_size = extent_from_axis_values(child_main_axis_size, our_cross_axis_size, main_axis);
		match &*child.borrow() {
			LayoutComponentNode::Tag(tag) => compute_layout(child_max_size, tag),
			LayoutComponentNode::Text(_) => (),
		}
		println!("Laid out child with {:?} size {}", main_axis, child_main_axis_size);
		free_space_used += free_space;
	}

	// For now, assume the percentages add up to no more than 100%.
	if free_space_used > free_space {
		panic!("children using free space percentages won't fit");
	}
}
