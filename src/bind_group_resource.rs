pub enum BindGroupResource<'a> {
	Owned(wgpu::BindGroup),
	Borrowed(&'a wgpu::BindGroup),
}

impl<'a> BindGroupResource<'a> {
	pub fn borrow(&self) -> BindGroupResource {
		match self {
			BindGroupResource::Owned(ref bind_group) => BindGroupResource::Borrowed(bind_group),
			BindGroupResource::Borrowed(ref bind_group) => BindGroupResource::Borrowed(bind_group),
		}
	}
}