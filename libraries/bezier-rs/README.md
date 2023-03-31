[crates.io](https://crates.io/crates/bezier-rs) • [docs.rs](https://docs.rs/bezier-rs/latest/bezier_rs/) • [repo](https://github.com/GraphiteEditor/Graphite/tree/master/libraries/bezier-rs)

# Bezier-rs

Computational geometry algorithms for Bézier segments and shapes useful in the context of 2D graphics.

Play with the interactive documentation which visualizes each API function in a fun manner:

### [**View the interactive API**](https://graphite.rs/libraries/bezier-rs/)

---

Bezier-rs is built for the needs of [Graphite](https://graphite.rs), an open source 2D vector graphics editor. We hope it may be useful to others, but presently Graphite is its primary user. Pull requests are welcomed for new features, code cleanup, ergonomic enhancements, performance improvements, and documentation clarifications.

The library currently provides functions dealing with single Bézier curve segments and open-or-closed multi-segment paths (which we call _subpaths_).

In the future, the library will be expanded to include compound paths (multiple subpaths forming a single shape, where the winding order determines inside-or-outside-ness) and operations between paths (e.g. boolean operations, convex hull). Pull requests for these additional features would be highly desirable.

Bezier-rs is inspired by [Bezier.js](https://pomax.github.io/bezierjs/) and [_A Primer on Bézier Curves_](https://pomax.github.io/bezierinfo/) by Pomax. Bezier-rs is not a port of Bezier.js so the API for single-segment Bézier curves has some differences, and the intention is to offer a broader scope that provides algorithms beyond single curve segments (as noted above) to eventually service full vector shapes.

## Terminology

Graphite and Bezier-rs use the following terminology for vector data. These depictions are given for cubic Bézier curves.

![Manipulators](https://static.graphite.rs/libraries/bezier-rs/manipulator-groups.png)
![Curve/Bezier Segment](https://static.graphite.rs/libraries/bezier-rs/curve-bezier-segment.png)
![Subpath/Path](https://static.graphite.rs/libraries/bezier-rs/subpath-path.png)
![Open/Closed](https://static.graphite.rs/libraries/bezier-rs/closed-open-subpath.png)
