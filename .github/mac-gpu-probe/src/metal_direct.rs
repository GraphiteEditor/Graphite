// Runs the probe's workloads through Metal directly, timed by each command buffer's own GPU start and end times,
// which Metal reports even where counter sampling (and therefore wgpu's timestamp queries) is unavailable

use std::ptr::NonNull;
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::{
	MTLBuffer, MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue, MTLCompileOptions, MTLComputeCommandEncoder, MTLComputePipelineState, MTLCounterSamplingPoint, MTLCounterSet, MTLCreateSystemDefaultDevice, MTLDevice,
	MTLLibrary, MTLResourceOptions, MTLSize,
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

// What wgpu generated for the probe's WGSL compute shader (copied from its debug log), with its default checks
const NAGA_CHECKED: &str = r#"
#include <metal_stdlib>
#include <simd/simd.h>

using metal::uint;

struct _mslBufferSizes {
    uint size0;
};

typedef metal::float4 type_1[1];

[[max_total_threads_per_threadgroup(256)]] kernel void main_(
  metal::uint3 id [[thread_position_in_grid]]
, device type_1& output [[buffer(0)]]
, constant _mslBufferSizes& _buffer_sizes [[buffer(1)]]
) {
    metal::float4 a0_ = {};
    metal::float4 a1_ = {};
    metal::float4 a2_ = {};
    metal::float4 a3_ = {};
    uint i = 0u;
    a0_ = metal::float4(static_cast<float>(id.x) * 0.0000001) + metal::float4(0.0, 1.0, 2.0, 3.0);
    metal::float4 _e13 = a0_;
    a1_ = _e13 + metal::float4(0.1);
    metal::float4 _e18 = a0_;
    a2_ = _e18 + metal::float4(0.2);
    metal::float4 _e23 = a0_;
    a3_ = _e23 + metal::float4(0.3);
    metal::float4 b0_ = metal::float4(0.9999, 0.9998, 0.9997, 0.9996);
    metal::float4 b1_ = metal::float4(0.9995, 0.9994, 0.9993, 0.9992);
    metal::float4 b2_ = metal::float4(0.9991, 0.999, 0.9989, 0.9988);
    metal::float4 b3_ = metal::float4(0.9987, 0.9986, 0.9985, 0.9984);
    metal::float4 c = metal::float4(0.0001, 0.0002, 0.0003, 0.0004);
    uint2 loop_bound = uint2(4294967295u);
    bool loop_init = true;
    while(true) {
        if (metal::all(loop_bound == uint2(0u))) { break; }
        loop_bound -= uint2(loop_bound.y == 0u, 1u);
        if (!loop_init) {
            uint _e67 = i;
            i = _e67 + 1u;
        }
        loop_init = false;
        uint _e55 = i;
        if (_e55 < 256u) {
        } else {
            break;
        }
        {
            metal::float4 _e58 = a0_;
            a0_ = metal::fma(_e58, b0_, c);
            metal::float4 _e60 = a1_;
            a1_ = metal::fma(_e60, b1_, c);
            metal::float4 _e62 = a2_;
            a2_ = metal::fma(_e62, b2_, c);
            metal::float4 _e64 = a3_;
            a3_ = metal::fma(_e64, b3_, c);
        }
    }
    metal::float4 _e72 = a0_;
    metal::float4 _e73 = a1_;
    metal::float4 _e75 = a2_;
    metal::float4 _e77 = a3_;
    output[metal::min(unsigned(id.x), (_buffer_sizes.size0 - 0 - 16) / 16)] = ((_e72 + _e73) + _e75) + _e77;
    return;
}
"#;

// What wgpu generated with its checks off, which still emits the loop as `while (true)` with a `loop_init` flag
const NAGA_UNCHECKED: &str = r#"
#include <metal_stdlib>
#include <simd/simd.h>

using metal::uint;

struct _mslBufferSizes {
    uint size0;
};

typedef metal::float4 type_1[1];

[[max_total_threads_per_threadgroup(256)]] kernel void main_(
  metal::uint3 id [[thread_position_in_grid]]
, device type_1& output [[buffer(0)]]
, constant _mslBufferSizes& _buffer_sizes [[buffer(1)]]
) {
    metal::float4 a0_ = {};
    metal::float4 a1_ = {};
    metal::float4 a2_ = {};
    metal::float4 a3_ = {};
    uint i = 0u;
    a0_ = metal::float4(static_cast<float>(id.x) * 0.0000001) + metal::float4(0.0, 1.0, 2.0, 3.0);
    metal::float4 _e13 = a0_;
    a1_ = _e13 + metal::float4(0.1);
    metal::float4 _e18 = a0_;
    a2_ = _e18 + metal::float4(0.2);
    metal::float4 _e23 = a0_;
    a3_ = _e23 + metal::float4(0.3);
    metal::float4 b0_ = metal::float4(0.9999, 0.9998, 0.9997, 0.9996);
    metal::float4 b1_ = metal::float4(0.9995, 0.9994, 0.9993, 0.9992);
    metal::float4 b2_ = metal::float4(0.9991, 0.999, 0.9989, 0.9988);
    metal::float4 b3_ = metal::float4(0.9987, 0.9986, 0.9985, 0.9984);
    metal::float4 c = metal::float4(0.0001, 0.0002, 0.0003, 0.0004);
    bool loop_init = true;
    while(true) {
        if (!loop_init) {
            uint _e67 = i;
            i = _e67 + 1u;
        }
        loop_init = false;
        uint _e55 = i;
        if (_e55 < 256u) {
        } else {
            break;
        }
        {
            metal::float4 _e58 = a0_;
            a0_ = metal::fma(_e58, b0_, c);
            metal::float4 _e60 = a1_;
            a1_ = metal::fma(_e60, b1_, c);
            metal::float4 _e62 = a2_;
            a2_ = metal::fma(_e62, b2_, c);
            metal::float4 _e64 = a3_;
            a3_ = metal::fma(_e64, b3_, c);
        }
    }
    metal::float4 _e72 = a0_;
    metal::float4 _e73 = a1_;
    metal::float4 _e75 = a2_;
    metal::float4 _e77 = a3_;
    output[id.x] = ((_e72 + _e73) + _e75) + _e77;
    return;
}
"#;

// The unchecked version with only its loop rewritten as a counted `for` loop
const NAGA_UNCHECKED_FOR_LOOP: &str = r#"
#include <metal_stdlib>
#include <simd/simd.h>

using metal::uint;

struct _mslBufferSizes {
    uint size0;
};

typedef metal::float4 type_1[1];

[[max_total_threads_per_threadgroup(256)]] kernel void main_(
  metal::uint3 id [[thread_position_in_grid]]
, device type_1& output [[buffer(0)]]
, constant _mslBufferSizes& _buffer_sizes [[buffer(1)]]
) {
    metal::float4 a0_ = {};
    metal::float4 a1_ = {};
    metal::float4 a2_ = {};
    metal::float4 a3_ = {};
    a0_ = metal::float4(static_cast<float>(id.x) * 0.0000001) + metal::float4(0.0, 1.0, 2.0, 3.0);
    metal::float4 _e13 = a0_;
    a1_ = _e13 + metal::float4(0.1);
    metal::float4 _e18 = a0_;
    a2_ = _e18 + metal::float4(0.2);
    metal::float4 _e23 = a0_;
    a3_ = _e23 + metal::float4(0.3);
    metal::float4 b0_ = metal::float4(0.9999, 0.9998, 0.9997, 0.9996);
    metal::float4 b1_ = metal::float4(0.9995, 0.9994, 0.9993, 0.9992);
    metal::float4 b2_ = metal::float4(0.9991, 0.999, 0.9989, 0.9988);
    metal::float4 b3_ = metal::float4(0.9987, 0.9986, 0.9985, 0.9984);
    metal::float4 c = metal::float4(0.0001, 0.0002, 0.0003, 0.0004);
    for (uint i = 0u; i < 256u; i++) {
        metal::float4 _e58 = a0_;
        a0_ = metal::fma(_e58, b0_, c);
        metal::float4 _e60 = a1_;
        a1_ = metal::fma(_e60, b1_, c);
        metal::float4 _e62 = a2_;
        a2_ = metal::fma(_e62, b2_, c);
        metal::float4 _e64 = a3_;
        a3_ = metal::fma(_e64, b3_, c);
    }
    metal::float4 _e72 = a0_;
    metal::float4 _e73 = a1_;
    metal::float4 _e75 = a2_;
    metal::float4 _e77 = a3_;
    output[id.x] = ((_e72 + _e73) + _e75) + _e77;
    return;
}
"#;

const SUSTAINED_WARMUP: Duration = Duration::from_secs(1);
const INTERLEAVED_ROUNDS: usize = 5;

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

	// Compiled with wgpu's own Metal compile options, so only the source differs between variants
	let options = MTLCompileOptions::new();
	options.setPreserveInvariance(true);
	let pipeline = |source: &str, name: &str| {
		let library = match device.newLibraryWithSource_options_error(&NSString::from_str(source), Some(&options)) {
			Ok(library) => library,
			Err(error) => {
				println!("RESULT: Metal shader compilation failed: {error}");
				return None;
			}
		};
		let function = library.newFunctionWithName(&NSString::from_str(name))?;
		device.newComputePipelineStateWithFunction_error(&function).ok()
	};

	let variants = [
		("hand-written", pipeline(SHADERS, "alu")),
		("wgpu's output, default checks", pipeline(NAGA_CHECKED, "main_")),
		("wgpu's output, no checks", pipeline(NAGA_UNCHECKED, "main_")),
		("wgpu's output, no checks, `for` loop", pipeline(NAGA_UNCHECKED_FOR_LOOP, "main_")),
	];
	let variants: Vec<_> = variants.into_iter().filter_map(|(name, pipeline)| Some((name, pipeline?))).collect();

	let invocations = ALU_WORKGROUPS as u64 * 256;
	let output_bytes = invocations as usize * 16;
	let Some(output) = device.newBufferWithLength_options(output_bytes, MTLResourceOptions::StorageModePrivate) else { return };
	let buffer_sizes = (output_bytes as u32).to_le_bytes();
	let Some(pointer) = NonNull::new(buffer_sizes.as_ptr() as *mut std::ffi::c_void) else { return };
	let Some(sizes) = (unsafe { device.newBufferWithBytes_length_options(pointer, buffer_sizes.len(), MTLResourceOptions::StorageModeShared) }) else { return };
	let buffers = [&output, &sizes];

	// Keeps the GPU busy long enough for its clocks to ramp up before anything is timed
	let warmup_start = Instant::now();
	let mut warmup_dispatches = 0;
	while warmup_start.elapsed() < SUSTAINED_WARMUP {
		let _ = time_once(&queue, &variants[0].1, &buffers, ALU_WORKGROUPS as usize);
		warmup_dispatches += 1;
	}

	// Each round times every variant in turn, so drift over time (clocks, other VMs on the host) affects them all alike
	let mut gpu_milliseconds = vec![Vec::new(); variants.len()];
	for _ in 0..INTERLEAVED_ROUNDS {
		for (index, (_, pipeline)) in variants.iter().enumerate() {
			for _ in 0..(TIMED_RUNS / INTERLEAVED_ROUNDS) {
				if let Some((gpu, _)) = time_once(&queue, pipeline, &buffers, ALU_WORKGROUPS as usize) {
					gpu_milliseconds[index].push(gpu);
				}
			}
		}
	}

	let gflops = |milliseconds: f64| invocations as f64 * ALU_FLOPS_PER_INVOCATION / (milliseconds * 1e6);
	println!("\n== FP32 compute, direct Metal, after {warmup_dispatches} warmup dispatches over {:?}, interleaved in {INTERLEAVED_ROUNDS} rounds", SUSTAINED_WARMUP);
	for ((name, _), times) in variants.iter().zip(&gpu_milliseconds) {
		if times.is_empty() {
			println!("-- {name}: GPU time unavailable");
		} else {
			println!("-- {name}: {:.1} GFLOPS from median GPU time; GPU time (ms): {}", gflops(median(times)), summarize(times));
		}
	}

	// Memory bandwidth, with nonzero contents in case the hardware compresses zeros
	let Some(bandwidth_pipeline) = pipeline(SHADERS, "bandwidth") else { return };
	let pattern: Vec<u8> = (0..BANDWIDTH_BUFFER_BYTES).map(|i| (i * 2_654_435_761 >> 24) as u8).collect();
	let Some(pointer) = NonNull::new(pattern.as_ptr() as *mut std::ffi::c_void) else { return };
	let source = unsafe { device.newBufferWithBytes_length_options(pointer, pattern.len(), MTLResourceOptions::StorageModeShared) };
	let destination = device.newBufferWithLength_options(BANDWIDTH_BUFFER_BYTES as usize, MTLResourceOptions::StorageModePrivate);
	let (Some(source), Some(destination)) = (source, destination) else { return };
	let workgroups = (BANDWIDTH_BUFFER_BYTES / 16 / 256) as usize;

	let mut gpu_milliseconds = Vec::new();
	let mut wall_milliseconds = Vec::new();
	for i in 0..(WARMUP_RUNS + TIMED_RUNS) {
		let Some((gpu, wall)) = time_once(&queue, &bandwidth_pipeline, &[&source, &destination], workgroups) else { continue };
		if i >= WARMUP_RUNS {
			gpu_milliseconds.push(gpu);
			wall_milliseconds.push(wall);
		}
	}
	let gigabytes_per_second = |milliseconds: f64| 2. * BANDWIDTH_BUFFER_BYTES as f64 / (milliseconds * 1e6);
	println!("\n== Memory bandwidth, direct Metal");
	crate::report(&gpu_milliseconds, &wall_milliseconds, "GB/s", gigabytes_per_second);

	let overhead: Vec<f64> = wall_milliseconds.iter().zip(&gpu_milliseconds).map(|(wall, gpu)| (wall - gpu) * 1e3).collect();
	if !overhead.is_empty() {
		println!("Wall minus GPU time per dispatch (µs): {}", summarize(&overhead));
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

// One dispatch, returning its GPU time from the command buffer (if Metal reports one) and the wall-clock time around commit-and-wait
fn time_once(queue: &Queue, pipeline: &Pipeline, buffers: &[&Buffer], workgroups: usize) -> Option<(f64, f64)> {
	let command_buffer = queue.commandBuffer()?;
	let encoder = command_buffer.computeCommandEncoder()?;
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
	(command_buffer.GPUStartTime() > 0. && gpu > 0.).then_some((gpu, wall))
}
