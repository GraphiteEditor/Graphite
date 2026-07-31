trait Attr {
	fn name() -> &'static str
}

impl Attr for bool {
	fn name() {"condition"}
}

struct Opacity(f64);

impl Attr for Opacity {
	fn name() {"opacity"}
}


#[node]
fn opacity<T>(_: impl Ctx, input: T, x: ReadAttr<bool>, opacity: WriteAttr<Opacity>) -> T {
	if x {
		*opacity = 1.;
	}
	input
}
