use crate::hound_wrapper::WavTextureMaker;
use std::sync::{Arc, Mutex};
use wgpu::{util::DeviceExt, *};

const SHADER_PREFIX: &'static str = "#version 450
layout(local_size_x = 1) in;

layout(set = 0, binding = 0) buffer OutputStorage {
	vec2[] output;
};

layout(set = 0, binding = 1) uniform DeviceInfo {
	uint iSampleRate;
	uint iBaseFrame;
};
";

const SHADER_SUFFIX: &'static str = "
void main() {
	uint idx = gl_GlobalInvocationID.x;
	uint frame = iBaseFrame + idx;
	output[idx] = mainSound(idx, float(frame) / float(iSampleRate));
}
";

pub struct GPUDirector {
	device: Arc<Device>,
	queue: Arc<Queue>,
	bind_group_layouts: Vec<BindGroupLayout>,
	pipeline: ComputePipeline,
	base_frame: u32,
	sound_storages: Vec<Arc<Mutex<WavTextureMaker>>>,
}

impl GPUDirector {
	pub fn new(
		device: Arc<Device>,
		queue: Arc<Queue>,
		shader_source: &str,
		sound_storages: Vec<Arc<Mutex<WavTextureMaker>>>,
	) -> Self {
		let bind_group_layouts = create_bind_group_layouts(&device, sound_storages.len());
		let pipeline = read_source(&device, &bind_group_layouts, shader_source, &sound_storages);
		Self {
			device,
			queue,
			bind_group_layouts,
			pipeline,
			base_frame: 0,
			sound_storages,
		}
	}
	pub fn from_default_device(
		shader_source: &str,
		sound_storages: Vec<Arc<Mutex<WavTextureMaker>>>,
	) -> Self {
		let (device, queue) = init_device();
		Self::new(
			Arc::new(device),
			Arc::new(queue),
			shader_source,
			sound_storages,
		)
	}
	pub fn render(&mut self, sample_rate: u32, buffer_length: u32) -> Vec<f32> {
		let Self {
			ref device,
			ref queue,
			ref bind_group_layouts,
			ref pipeline,
			ref mut base_frame,
			ref mut sound_storages,
		} = self;
		let (storage, staging) = create_output_buffers(&self.device, buffer_length as u64);
		let device_info = device.create_buffer_init(&util::BufferInitDescriptor {
			label: None,
			contents: bytemuck::cast_slice(&[sample_rate, *base_frame]),
			usage: BufferUsages::UNIFORM,
		});
		*base_frame += buffer_length / 2;
		let bind_group0 = device.create_bind_group(&BindGroupDescriptor {
			label: None,
			layout: &bind_group_layouts[0],
			entries: &[
				BindGroupEntry {
					binding: 0,
					resource: storage.as_entire_binding(),
				},
				BindGroupEntry {
					binding: 1,
					resource: device_info.as_entire_binding(),
				},
			],
		});
		let sound_buffers =
			sound_storage_buffers(device, sound_storages, buffer_length as usize / 2, sample_rate);
		let entries = buffers_to_entries(&sound_buffers);
		let bind_group1 = device.create_bind_group(&BindGroupDescriptor {
			label: None,
			layout: &bind_group_layouts[1],
			entries: &entries,
		});
		let mut encoder = device.create_command_encoder(&Default::default());
		{
			let mut cpass = encoder.begin_compute_pass(&Default::default());
			cpass.set_pipeline(pipeline);
			cpass.set_bind_group(0, &bind_group0, &[]);
			cpass.set_bind_group(1, &bind_group1, &[]);
			cpass.insert_debug_marker("rendering sound");
			cpass.dispatch(buffer_length / 2, 1, 1);
		}
		encoder.copy_buffer_to_buffer(&storage, 0, &staging, 0, buffer_length as u64 * 4);
		queue.submit(Some(encoder.finish()));

		let buffer_slice = staging.slice(..);
		let buffer_future = buffer_slice.map_async(MapMode::Read);
		device.poll(wgpu::Maintain::Wait);

		pollster::block_on(async {
			if buffer_future.await.is_ok() {
				let data = buffer_slice.get_mapped_range();
				let result: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
				drop(data);
				staging.unmap();
				result
			} else {
				panic!("failed to run compute on gpu");
			}
		})
	}
}

fn init_device() -> (Device, Queue) {
	let instance = Instance::new(Backends::PRIMARY);
	pollster::block_on(async {
		let adaptor = instance
			.request_adapter(&Default::default())
			.await
			.expect("failed to find an appropriate adapter");
		println!("GPU Device: {:?}", adaptor.get_info());
		adaptor
			.request_device(&Default::default(), None)
			.await
			.expect("failed to create device")
	})
}

fn create_bind_group_layouts(device: &Device, len: usize) -> Vec<BindGroupLayout> {
	let bgl0 = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
		label: None,
		entries: &[
			BindGroupLayoutEntry {
				binding: 0,
				visibility: ShaderStages::COMPUTE,
				ty: BindingType::Buffer {
					ty: BufferBindingType::Storage { read_only: false },
					has_dynamic_offset: false,
					min_binding_size: None,
				},
				count: None,
			},
			BindGroupLayoutEntry {
				binding: 1,
				visibility: ShaderStages::COMPUTE,
				ty: BindingType::Buffer {
					ty: BufferBindingType::Uniform,
					has_dynamic_offset: false,
					min_binding_size: None,
				},
				count: None,
			},
		],
	});
	let bgl1 = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
		label: None,
		entries: &sound_storage_bind_group_layout_entries(len as u32),
	});
	vec![bgl0, bgl1]
}

pub fn read_source(
	device: &Device,
	bind_group_layouts: &[BindGroupLayout],
	code: &str,
	resources: &Vec<Arc<Mutex<WavTextureMaker>>>,
) -> ComputePipeline {
	let mut code_buf = SHADER_PREFIX.to_string();
	(0..resources.len()).for_each(|idx| code_buf += &sound_storage_bindingshader(idx));
	(0..resources.len()).for_each(|idx| code_buf += &sound_storage_fetchfunction(idx));
	code_buf = code_buf + code + SHADER_SUFFIX;
	let wgsl = glsl_to_wgsl(&code_buf);
	let module = device.create_shader_module(&ShaderModuleDescriptor {
		label: None,
		source: ShaderSource::Wgsl(wgsl.into()),
	});
	let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
		label: None,
		bind_group_layouts: &bind_group_layouts.iter().collect::<Vec<_>>(),
		push_constant_ranges: &[],
	});
	device.create_compute_pipeline(&ComputePipelineDescriptor {
		label: None,
		layout: Some(&pipeline_layout),
		module: &module,
		entry_point: "main",
	})
}

fn glsl_to_wgsl(code: &str) -> String {
	let glsl_module = naga::front::glsl::Parser::default()
		.parse(
			&naga::front::glsl::Options {
				stage: naga::ShaderStage::Compute,
				defines: Default::default(),
			},
			&code,
		)
		.unwrap_or_else(|e| panic!("GLSL Parse Error: {:?}", e));
	let glsl_module_info = naga::valid::Validator::new(
		naga::valid::ValidationFlags::all(),
		naga::valid::Capabilities::empty(),
	)
	.validate(&glsl_module)
	.unwrap_or_else(|e| panic!("GLSL Validation Error: {:?}", e));
	naga::back::wgsl::write_string(&glsl_module, &glsl_module_info)
		.unwrap_or_else(|e| panic!("WGSL write error: {}", e))
}

#[test]
fn glsl_to_wgsl_test() {
	let code = SHADER_PREFIX.to_string() + include_str!("sample.comp") + SHADER_SUFFIX;
	let code = glsl_to_wgsl(&code);
	println!("{}", code);
}

fn create_output_buffers(device: &Device, len: u64) -> (Buffer, Buffer) {
	let storage = device.create_buffer(&BufferDescriptor {
		label: None,
		size: len * 4,
		usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
		mapped_at_creation: false,
	});
	let staging = device.create_buffer(&BufferDescriptor {
		label: None,
		size: len * 4,
		usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
		mapped_at_creation: false,
	});
	(storage, staging)
}

fn sound_storage_bindingshader(idx: usize) -> String {
	format!(
		"layout(set = 1, binding = {}) buffer AudioTexture{} {{
	vec4[] iAudioTexture{1};
}};
layout(set = 1, binding = {}) uniform AudioTextureInfo{1} {{
	uint iChannelSampleRate{1};
	uint channels{1};
}};
	",
		idx * 2,
		idx,
		idx * 2 + 1
	)
}

fn sound_storage_fetchfunction(idx: usize) -> String {
	format!(
		"vec2 soundTexture{}(float time) {{
	float t = time - float(iBaseFrame) / float(iSampleRate);
	uint idx = uint(float(iChannelSampleRate{0}) * t);
	float p = fract(float(iChannelSampleRate{0}) * t);
	return iAudioTexture{0}[idx].xy * (1.0 - p) + iAudioTexture{0}[idx + 1].xy * p;
}}
vec2 soundTexelFetch{0}(uint idx) {{
	uint baseIdx = iChannelSampleRate{0} * (iBaseFrame / iSampleRate);
	return iAudioTexture{0}[idx - baseIdx].xy;
}}
vec2 soundDFTFetch{0}(uint idx) {{
	uint baseIdx = iChannelSampleRate{0} * (iBaseFrame / iSampleRate);
	return iAudioTexture{0}[idx - baseIdx].zw;
}}
",
		idx
	)
}

fn sound_storage_bind_group_layout_entries(len: u32) -> Vec<BindGroupLayoutEntry> {
	(0..len)
		.flat_map(|i| {
			vec![
				BindGroupLayoutEntry {
					binding: i * 2,
					visibility: ShaderStages::COMPUTE,
					ty: BindingType::Buffer {
						ty: BufferBindingType::Storage { read_only: false },
						has_dynamic_offset: false,
						min_binding_size: None,
					},
					count: None,
				},
				BindGroupLayoutEntry {
					binding: i * 2 + 1,
					visibility: ShaderStages::COMPUTE,
					ty: BindingType::Buffer {
						ty: BufferBindingType::Uniform,
						has_dynamic_offset: false,
						min_binding_size: None,
					},
					count: None,
				},
			]
		})
		.collect()
}

fn sound_storage_buffers(
	device: &Device,
	storages: &Vec<Arc<Mutex<WavTextureMaker>>>,
	buffer_length: usize,
	device_sample_rate: u32,
) -> Vec<Buffer> {
	storages
		.iter()
		.flat_map(|storage| {
			let mut storage = storage.lock().unwrap();
			let hound::WavSpec {
				sample_rate,
				channels,
				..
			} = storage.spec();
			let buffer_length =
				(buffer_length as f64 * sample_rate as f64 / device_sample_rate as f64) as usize;
			if storage.buffer_len() < buffer_length {
				println!("not enough textures!");
			}
			let vec = storage.next_buffer(buffer_length);
			let storage_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
				label: None,
				contents: bytemuck::cast_slice(&vec),
				usage: BufferUsages::STORAGE,
			});
			drop(storage);
			let uniform_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
				label: None,
				contents: bytemuck::cast_slice(&[sample_rate, channels as u32]),
				usage: BufferUsages::UNIFORM,
			});
			vec![storage_buffer, uniform_buffer]
		})
		.collect()
}

fn buffers_to_entries(buffers: &Vec<Buffer>) -> Vec<BindGroupEntry> {
	buffers
		.iter()
		.enumerate()
		.flat_map(|(i, buffer)| {
			vec![BindGroupEntry {
				binding: i as u32,
				resource: buffer.as_entire_binding(),
			}]
		})
		.collect()
}
