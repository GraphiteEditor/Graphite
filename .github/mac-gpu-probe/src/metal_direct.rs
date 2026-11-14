// Runs the same workloads as hand-written Metal, timed by each command buffer's own GPU start and end times,
// which Metal reports even where counter sampling (and therefore wgpu's timestamp queries) is unavailable

use std::ptr::NonNull;
use std::time::Instant;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::{
	MTLBuffer, MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue, MTLComputeCommandEncoder, MTLComputePipelineState, MTLCounterSamplingPoint, MTLCounterSet, MTLCreateSystemDefaultDevice, MTLDevice, MTLLibrary,
	MTLResourceOptions, MTLSize,
};

use crate::{ALU_FLOPS_PER_INVOCATION, ALU_WORKGROUPS, BANDWIDTH_BUFFER_BYTES, TIMED_RUNS, WARMUP_RUNS, median, summarize};

const SHADERS: &str = r#"
#include <metal_stdlib>
using namespace metal;

kernel void alu(device float4 *output [[buffer(0)]], uint id [[thread_position_in_grid]]) {
	float4 a0 = float(id) * 1e-7 + float4(0.0, 1.0, 2.0, 3.0);
	float4 a1 = a0 + 0.1;
	float4 a2 = a0 + 0.2;
	float4 a3 = a0 + 0.3;
	const float4 b0 = float4(0.9999, 0.9998, 0.9997, 0.9996);
	const float4 b1 = float4(0.9995, 0.9994, 0.9993, 0.9992);
	const float4 b2 = float4(0.9991, 0.9990, 0.9989, 0.9988);
	const float4 b3 = float4(0.9987, 0.9986, 0.9985, 0.9984);
	const float4 c = float4(1e-4, 2e-4, 3e-4, 4e-4);
	for (uint i = 0; i < 256; i++) {
		a0 = fma(a0, b0, c);
		a1 = fma(a1, b1, c);
		a2 = fma(a2, b2, c);
		a3 = fma(a3, b3, c);
	}
	output[id] = a0 + a1 + a2 + a3;
}

kernel void bandwidth(device const float4 *source [[buffer(0)]], device float4 *destination [[buffer(1)]], uint id [[thread_position_in_grid]]) {
	destination[id] = source[id] * 1.0001;
}
"#;

type Device = ProtocolObject<dyn MTLDevice>;
type Queue = ProtocolObject<dyn MTLCommandQueue>;
type Pipeline = Retained<ProtocolObject<dyn MTLComputePipelineState>>;
type Buffer = Retained<ProtocolObject<dyn MTLBuffer>>;

pub fn run() {
	println!("\n######## Direct Metal");
	let Some(device) = MTLCreateSystemDefaultDevice() else {
		println!("RESULT: no Metal device");
		return;
	};
	describe(&device);

	let Some(queue) = device.newCommandQueue() else {
		println!("RESULT: no Metal command queue");
		return;
	};
	let library = match device.newLibraryWithSource_options_error(&NSString::from_str(SHADERS), None) {
		Ok(library) => library,
		Err(error) => {
			println!("RESULT: Metal shader compilation failed: {error}");
			return;
		}
	};
	let pipeline = |name: &str| {
		let function = library.newFunctionWithName(&NSString::from_str(name))?;
		device.newComputePipelineStateWithFunction_error(&function).ok()
	};
	let (Some(alu_pipeline), Some(bandwidth_pipeline)) = (pipeline("alu"), pipeline("bandwidth")) else {
		println!("RESULT: Metal pipeline creation failed");
		return;
	};

	// FP32 compute
	let invocations = ALU_WORKGROUPS as u64 * 256;
	let Some(output) = device.newBufferWithLength_options(invocations as usize * 16, MTLResourceOptions::StorageModePrivate) else { return };
	let (gpu_milliseconds, wall_milliseconds) = time_dispatch(&queue, &alu_pipeline, &[&output], ALU_WORKGROUPS as usize);
	let gflops = |milliseconds: f64| invocations as f64 * ALU_FLOPS_PER_INVOCATION / (milliseconds * 1e6);
	println!("\n== FP32 compute, direct Metal");
	crate::report(&gpu_milliseconds, &wall_milliseconds, "GFLOPS", gflops);

	// Memory bandwidth, with nonzero contents in case the hardware compresses zeros
	let pattern: Vec<u8> = (0..BANDWIDTH_BUFFER_BYTES).map(|i| (i * 2_654_435_761 >> 24) as u8).collect();
	let Some(pointer) = NonNull::new(pattern.as_ptr() as *mut std::ffi::c_void) else { return };
	let source = unsafe { device.newBufferWithBytes_length_options(pointer, pattern.len(), MTLResourceOptions::StorageModeShared) };
	let destination = device.newBufferWithLength_options(BANDWIDTH_BUFFER_BYTES as usize, MTLResourceOptions::StorageModePrivate);
	let (Some(source), Some(destination)) = (source, destination) else { return };
	let workgroups = (BANDWIDTH_BUFFER_BYTES / 16 / 256) as usize;
	let (gpu_milliseconds, wall_milliseconds) = time_dispatch(&queue, &bandwidth_pipeline, &[&source, &destination], workgroups);
	let gigabytes_per_second = |milliseconds: f64| 2. * BANDWIDTH_BUFFER_BYTES as f64 / (milliseconds * 1e6);
	println!("\n== Memory bandwidth, direct Metal");
	crate::report(&gpu_milliseconds, &wall_milliseconds, "GB/s", gigabytes_per_second);

	if !gpu_milliseconds.is_empty() {
		let overhead: Vec<f64> = wall_milliseconds.iter().zip(&gpu_milliseconds).map(|(wall, gpu)| (wall - gpu) * 1e3).collect();
		println!("Wall minus GPU time per dispatch (µs): {}, median {:.1}", summarize(&overhead), median(&overhead));
	}
}

fn describe(device: &Device) {
	println!("name: {}, unified memory: {}, recommended working set: {} MiB", device.name(), device.hasUnifiedMemory(), device.recommendedMaxWorkingSetSize() >> 20);

	let sampling_points = [
		("stage boundary", MTLCounterSamplingPoint::AtStageBoundary),
		("draw boundary", MTLCounterSamplingPoint::AtDrawBoundary),
		("dispatch boundary", MTLCounterSamplingPoint::AtDispatchBoundary),
		("tile dispatch boundary", MTLCounterSamplingPoint::AtTileDispatchBoundary),
		("blit boundary", MTLCounterSamplingPoint::AtBlitBoundary),
	];
	let supported: Vec<_> = sampling_points.iter().filter(|(_, point)| device.supportsCounterSampling(*point)).map(|(name, _)| *name).collect();
	println!("counter sampling supported at: {}", if supported.is_empty() { "none".to_string() } else { supported.join(", ") });

	let sets = device.counterSets().map(|sets| sets.iter().map(|set| set.name().to_string()).collect::<Vec<_>>()).unwrap_or_default();
	println!("counter sets: {}", if sets.is_empty() { "none".to_string() } else { sets.join(", ") });
}

// Returns per-run GPU times from the command buffer (empty if Metal reports none) and wall-clock times around commit-and-wait
fn time_dispatch(queue: &Queue, pipeline: &Pipeline, buffers: &[&Buffer], workgroups: usize) -> (Vec<f64>, Vec<f64>) {
	let mut gpu_milliseconds = Vec::new();
	let mut wall_milliseconds = Vec::new();

	for i in 0..(WARMUP_RUNS + TIMED_RUNS) {
		let Some(command_buffer) = queue.commandBuffer() else { break };
		let Some(encoder) = command_buffer.computeCommandEncoder() else { break };
		encoder.setComputePipelineState(pipeline);
		for (index, buffer) in buffers.iter().enumerate() {
			unsafe { encoder.setBuffer_offset_atIndex(Some(buffer), 0, index) };
		}
		encoder.dispatchThreadgroups_threadsPerThreadgroup(MTLSize { width: workgroups, height: 1, depth: 1 }, MTLSize { width: 256, height: 1, depth: 1 });
		encoder.endEncoding();

		let start = Instant::now();
		command_buffer.commit();
		command_buffer.waitUntilCompleted();
		let wall = start.elapsed().as_secs_f64() * 1e3;

		let gpu = (command_buffer.GPUEndTime() - command_buffer.GPUStartTime()) * 1e3;
		if i >= WARMUP_RUNS {
			wall_milliseconds.push(wall);
			if command_buffer.GPUStartTime() > 0. && gpu > 0. {
				gpu_milliseconds.push(gpu);
			}
		}
	}

	(gpu_milliseconds, wall_milliseconds)
}
