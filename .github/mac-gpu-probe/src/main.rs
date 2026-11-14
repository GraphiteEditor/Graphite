// Probes whether a CI runner's GPU is real enough to profile on: what adapter it exposes, whether GPU timestamps work,
// how close compute and bandwidth throughput come to the hardware's peak, and how steady repeated measurements are

use std::time::Instant;

const WARMUP_RUNS: usize = 3;
const TIMED_RUNS: usize = 20;
const SUBMIT_RUNS: usize = 100;

// 4 independent FMA chains on vec4s, 256 iterations: 4 * 4 * 2 * 256 = 8192 FLOPs per invocation
const ALU_SHADER: &str = r#"
@group(0) @binding(0) var<storage, read_write> output: array<vec4<f32>>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
	var a0 = vec4<f32>(f32(id.x) * 1e-7, 1.0, 2.0, 3.0);
	var a1 = a0 + 0.1;
	var a2 = a0 + 0.2;
	var a3 = a0 + 0.3;
	let b = vec4<f32>(0.9999, 0.9998, 0.9997, 0.9996);
	let c = vec4<f32>(1e-4, 2e-4, 3e-4, 4e-4);
	for (var i = 0u; i < 256u; i++) {
		a0 = fma(a0, b, c);
		a1 = fma(a1, b, c);
		a2 = fma(a2, b, c);
		a3 = fma(a3, b, c);
	}
	output[id.x] = a0 + a1 + a2 + a3;
}
"#;
const ALU_WORKGROUPS: u32 = 4096;
const ALU_FLOPS_PER_INVOCATION: f64 = 8192.;

const BANDWIDTH_SHADER: &str = r#"
@group(0) @binding(0) var<storage, read> source: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read_write> destination: array<vec4<f32>>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
	if (id.x < arrayLength(&source)) {
		destination[id.x] = source[id.x] * 1.0001;
	}
}
"#;
const BANDWIDTH_BUFFER_BYTES: u64 = 128 * 1024 * 1024;

struct Gpu {
	device: wgpu::Device,
	queue: wgpu::Queue,
	timestamps: Option<Timestamps>,
}

struct Timestamps {
	query_set: wgpu::QuerySet,
	resolve: wgpu::Buffer,
	readback: wgpu::Buffer,
	nanoseconds_per_tick: f64,
}

fn main() {
	pollster::block_on(run());
}

async fn run() {
	let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
		backends: wgpu::Backends::PRIMARY,
		..wgpu::InstanceDescriptor::new_without_display_handle()
	});

	println!("== Adapters");
	for adapter in instance.enumerate_adapters(wgpu::Backends::PRIMARY).await {
		let info = adapter.get_info();
		println!("{:?} | {} | {:?} | driver {} {}", info.backend, info.name, info.device_type, info.driver, info.driver_info);
	}

	let Ok(adapter) = instance
		.request_adapter(&wgpu::RequestAdapterOptions {
			power_preference: wgpu::PowerPreference::HighPerformance,
			..Default::default()
		})
		.await
	else {
		println!("RESULT: no GPU adapter available");
		return;
	};
	let info = adapter.get_info();
	let limits = adapter.limits();
	let has_timestamps = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
	println!("\n== Chosen adapter\n{:?} | {} | {:?}", info.backend, info.name, info.device_type);
	println!("timestamp queries: {has_timestamps}");
	println!("max storage buffer binding: {} MiB, max buffer: {} MiB", limits.max_storage_buffer_binding_size / (1 << 20), limits.max_buffer_size / (1 << 20));

	let required_features = if has_timestamps { wgpu::Features::TIMESTAMP_QUERY } else { wgpu::Features::empty() };
	let Ok((device, queue)) = adapter
		.request_device(&wgpu::DeviceDescriptor {
			label: None,
			required_features,
			required_limits: limits,
			memory_hints: Default::default(),
			trace: wgpu::Trace::Off,
			experimental_features: Default::default(),
		})
		.await
	else {
		println!("RESULT: the adapter refused to create a device");
		return;
	};

	let timestamps = has_timestamps.then(|| Timestamps {
		query_set: device.create_query_set(&wgpu::QuerySetDescriptor {
			label: None,
			ty: wgpu::QueryType::Timestamp,
			count: 2,
		}),
		resolve: device.create_buffer(&wgpu::BufferDescriptor {
			label: None,
			size: 16,
			usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
			mapped_at_creation: false,
		}),
		readback: device.create_buffer(&wgpu::BufferDescriptor {
			label: None,
			size: 16,
			usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
			mapped_at_creation: false,
		}),
		nanoseconds_per_tick: queue.get_timestamp_period() as f64,
	});
	let gpu = Gpu { device, queue, timestamps };

	submit_latency(&gpu);
	alu_throughput(&gpu);
	bandwidth(&gpu);
}

// Round trips of an empty submission, the fixed overhead any per-frame measurement includes
fn submit_latency(gpu: &Gpu) {
	let mut microseconds = Vec::new();
	for i in 0..(WARMUP_RUNS + SUBMIT_RUNS) {
		let start = Instant::now();
		let encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
		gpu.queue.submit(Some(encoder.finish()));
		let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
		if i >= WARMUP_RUNS {
			microseconds.push(start.elapsed().as_secs_f64() * 1e6);
		}
	}
	println!("\n== Empty submit round trip (µs)\n{}", summarize(&microseconds));
}

fn alu_throughput(gpu: &Gpu) {
	let invocations = ALU_WORKGROUPS as u64 * 256;
	let output = gpu.device.create_buffer(&wgpu::BufferDescriptor {
		label: None,
		size: invocations * 16,
		usage: wgpu::BufferUsages::STORAGE,
		mapped_at_creation: false,
	});
	let (pipeline, bind_group) = pipeline(gpu, ALU_SHADER, &[&output]);

	let (gpu_milliseconds, wall_milliseconds) = time_dispatch(gpu, &pipeline, &bind_group, ALU_WORKGROUPS);
	let gflops = |milliseconds: f64| invocations as f64 * ALU_FLOPS_PER_INVOCATION / (milliseconds * 1e6);

	println!("\n== FP32 compute ({:.1} GFLOP per dispatch)", invocations as f64 * ALU_FLOPS_PER_INVOCATION / 1e9);
	report(&gpu_milliseconds, &wall_milliseconds, "GFLOPS", gflops);
}

fn bandwidth(gpu: &Gpu) {
	let source = gpu.device.create_buffer(&wgpu::BufferDescriptor {
		label: None,
		size: BANDWIDTH_BUFFER_BYTES,
		usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
		mapped_at_creation: false,
	});
	let destination = gpu.device.create_buffer(&wgpu::BufferDescriptor {
		label: None,
		size: BANDWIDTH_BUFFER_BYTES,
		usage: wgpu::BufferUsages::STORAGE,
		mapped_at_creation: false,
	});

	// Nonzero contents, in case the hardware compresses zeros
	let pattern: Vec<u8> = (0..BANDWIDTH_BUFFER_BYTES).map(|i| (i * 2_654_435_761 >> 24) as u8).collect();
	gpu.queue.write_buffer(&source, 0, &pattern);

	let (pipeline, bind_group) = pipeline(gpu, BANDWIDTH_SHADER, &[&source, &destination]);
	let workgroups = (BANDWIDTH_BUFFER_BYTES / 16 / 256) as u32;

	let (gpu_milliseconds, wall_milliseconds) = time_dispatch(gpu, &pipeline, &bind_group, workgroups);
	let bytes_moved = 2. * BANDWIDTH_BUFFER_BYTES as f64;
	let gigabytes_per_second = |milliseconds: f64| bytes_moved / (milliseconds * 1e6);

	println!("\n== Memory bandwidth ({} MiB read + {} MiB written per dispatch)", BANDWIDTH_BUFFER_BYTES >> 20, BANDWIDTH_BUFFER_BYTES >> 20);
	report(&gpu_milliseconds, &wall_milliseconds, "GB/s", gigabytes_per_second);
}

fn pipeline(gpu: &Gpu, shader: &str, buffers: &[&wgpu::Buffer]) -> (wgpu::ComputePipeline, wgpu::BindGroup) {
	let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor {
		label: None,
		source: wgpu::ShaderSource::Wgsl(shader.into()),
	});
	let pipeline = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
		label: None,
		layout: None,
		module: &module,
		entry_point: Some("main"),
		compilation_options: Default::default(),
		cache: None,
	});

	let entries: Vec<_> = buffers
		.iter()
		.enumerate()
		.map(|(binding, buffer)| wgpu::BindGroupEntry {
			binding: binding as u32,
			resource: buffer.as_entire_binding(),
		})
		.collect();
	let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
		label: None,
		layout: &pipeline.get_bind_group_layout(0),
		entries: &entries,
	});

	(pipeline, bind_group)
}

// Returns per-run GPU times from timestamp queries (empty if unsupported) and wall-clock times around submit-and-wait
fn time_dispatch(gpu: &Gpu, pipeline: &wgpu::ComputePipeline, bind_group: &wgpu::BindGroup, workgroups: u32) -> (Vec<f64>, Vec<f64>) {
	let mut gpu_milliseconds = Vec::new();
	let mut wall_milliseconds = Vec::new();

	for i in 0..(WARMUP_RUNS + TIMED_RUNS) {
		let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
		{
			let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
				label: None,
				timestamp_writes: gpu.timestamps.as_ref().map(|timestamps| wgpu::ComputePassTimestampWrites {
					query_set: &timestamps.query_set,
					beginning_of_pass_write_index: Some(0),
					end_of_pass_write_index: Some(1),
				}),
			});
			pass.set_pipeline(pipeline);
			pass.set_bind_group(0, bind_group, &[]);
			pass.dispatch_workgroups(workgroups, 1, 1);
		}
		if let Some(timestamps) = &gpu.timestamps {
			encoder.resolve_query_set(&timestamps.query_set, 0..2, &timestamps.resolve, 0);
			encoder.copy_buffer_to_buffer(&timestamps.resolve, 0, &timestamps.readback, 0, 16);
		}

		let start = Instant::now();
		gpu.queue.submit(Some(encoder.finish()));
		let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
		let wall = start.elapsed().as_secs_f64() * 1e3;

		let gpu_time = gpu.timestamps.as_ref().and_then(|timestamps| read_timestamps(gpu, timestamps));
		if i >= WARMUP_RUNS {
			wall_milliseconds.push(wall);
			gpu_milliseconds.extend(gpu_time);
		}
	}

	(gpu_milliseconds, wall_milliseconds)
}

fn read_timestamps(gpu: &Gpu, timestamps: &Timestamps) -> Option<f64> {
	let slice = timestamps.readback.slice(..);
	slice.map_async(wgpu::MapMode::Read, |_| {});
	let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());

	let ticks = {
		let bytes = slice.get_mapped_range().ok()?;
		let start = u64::from_le_bytes(bytes[0..8].try_into().ok()?);
		let end = u64::from_le_bytes(bytes[8..16].try_into().ok()?);
		end.checked_sub(start)
	};
	timestamps.readback.unmap();

	ticks.filter(|&ticks| ticks > 0).map(|ticks| ticks as f64 * timestamps.nanoseconds_per_tick / 1e6)
}

fn report(gpu_milliseconds: &[f64], wall_milliseconds: &[f64], unit: &str, throughput: impl Fn(f64) -> f64) {
	if gpu_milliseconds.is_empty() {
		println!("GPU time: unavailable");
	} else {
		println!("GPU time (ms): {}", summarize(gpu_milliseconds));
		println!("Throughput from median GPU time: {:.1} {unit}", throughput(median(gpu_milliseconds)));
	}
	println!("Wall time (ms): {}", summarize(wall_milliseconds));
	println!("Throughput from median wall time: {:.1} {unit}", throughput(median(wall_milliseconds)));
}

fn median(values: &[f64]) -> f64 {
	let mut sorted = values.to_vec();
	sorted.sort_by(f64::total_cmp);
	sorted[sorted.len() / 2]
}

fn summarize(values: &[f64]) -> String {
	let mean = values.iter().sum::<f64>() / values.len() as f64;
	let deviation = (values.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / values.len() as f64).sqrt();
	let min = values.iter().copied().fold(f64::INFINITY, f64::min);
	let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
	format!("median {:.3}, min {min:.3}, max {max:.3}, spread {:.2}% (n={})", median(values), deviation / mean * 100., values.len())
}
