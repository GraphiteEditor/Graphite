use std::sync::Arc;
use vulkano::{
	command_buffer::allocator::StandardCommandBufferAllocator,
	descriptor_set::allocator::StandardDescriptorSetAllocator,
	device::{Device, DeviceCreateInfo, Queue, QueueCreateInfo},
	instance::{Instance, InstanceCreateInfo},
	memory::allocator::StandardMemoryAllocator,
	VulkanLibrary,
};

#[derive(Debug)]
pub struct Context {
	pub instance: Arc<Instance>,
	pub device: Arc<Device>,
	pub queue: Arc<Queue>,
	pub allocator: StandardMemoryAllocator,
	pub command_buffer_allocator: StandardCommandBufferAllocator,
	pub descriptor_set_allocator: StandardDescriptorSetAllocator,
}

impl Context {
	pub fn new() -> Self {
		let library = VulkanLibrary::new().unwrap();
		let instance = Instance::new(library, InstanceCreateInfo::default()).expect("failed to create instance");
		let physical = instance.enumerate_physical_devices().expect("could not enumerate devices").next().expect("no device available");
		for family in physical.queue_family_properties() {
			println!("Found a queue family with {:?} queue(s)", family.queue_count);
		}
		let queue_family_index = physical
			.queue_family_properties()
			.iter()
			.enumerate()
			.position(|(_, q)| q.queue_flags.graphics)
			.expect("couldn't find a graphical queue family") as u32;

		let (device, mut queues) = Device::new(
			physical,
			DeviceCreateInfo {
				// here we pass the desired queue family to use by index
				queue_create_infos: vec![QueueCreateInfo {
					queue_family_index,
					..Default::default()
				}],
				..Default::default()
			},
		)
		.expect("failed to create device");
		let queue = queues.next().unwrap();
		let alloc = StandardMemoryAllocator::new_default(device.clone());
		let calloc = StandardCommandBufferAllocator::new(device.clone());
		let dalloc = StandardDescriptorSetAllocator::new(device.clone());

		Self {
			instance,
			device,
			queue,
			allocator: alloc,
			command_buffer_allocator: calloc,
			descriptor_set_allocator: dalloc,
		}
	}
}

impl Default for Context {
	fn default() -> Self {
		Self::new()
	}
}
