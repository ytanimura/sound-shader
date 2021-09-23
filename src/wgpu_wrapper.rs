use wgpu::{util::DeviceExt, *};

const SHADER_PREFIX: &'static str = "#version 450
layout(local_size_x = 1) in;

layout(set = 0, binding = 0) buffer OutputStorage {
	float[] output;
};

layout(set = 0, binding = 1) uniform DeviceInfo {
	uint sample_rate;
	uint base_frame;
};";

const SHADER_SUFFIX: &'static str = "void main() {
	uint idx = gl_GlobalInvocationID.x;
	uint frame = base_frame + idx;
	vec2 c = mainSound(int(sample_rate), float(frame) / float(sample_rate));
	output[idx * 2] = c.x;
	output[idx * 2 + 1] = c.y;
}";

pub struct GPUDirector {
	device: Device,
	queue: Queue,
	bind_group_layout: BindGroupLayout,
	pipeline: Option<ComputePipeline>,
	base_frame: u32,
}

impl GPUDirector {
	pub fn new() -> Self {
		let (device, queue) = init_device();
		let bind_group_layout = create_bind_group_layout(&device);
		Self {
			device,
			queue,
			bind_group_layout,
			pipeline: None,
			base_frame: 0,
		}
	}
	pub fn read_source(&mut self, code: &str) {
		let code = SHADER_PREFIX.to_string() + code + SHADER_SUFFIX;
		let wgsl = glsl_to_wgsl(&code);
		let module = self.device.create_shader_module(&ShaderModuleDescriptor {
			label: None,
			source: ShaderSource::Wgsl(wgsl.into()),
		});
		let pipeline_layout = self
			.device
			.create_pipeline_layout(&PipelineLayoutDescriptor {
				label: None,
				bind_group_layouts: &[&self.bind_group_layout],
				push_constant_ranges: &[],
			});
		let pipeline = self
			.device
			.create_compute_pipeline(&ComputePipelineDescriptor {
				label: None,
				layout: Some(&pipeline_layout),
				module: &module,
				entry_point: "main",
			});
		self.pipeline = Some(pipeline);
	}
	pub fn render(&mut self, sample_rate: u32, buffer_length: u32) -> Vec<f32> {
		let Self {
			ref device,
			ref queue,
			ref bind_group_layout,
			ref pipeline,
			ref mut base_frame,
		} = self;
		let (storage, staging) = create_output_buffers(&self.device, buffer_length as u64);
		let device_info = device.create_buffer_init(&util::BufferInitDescriptor {
			label: None,
			contents: bytemuck::cast_slice(&[sample_rate, *base_frame]),
			usage: BufferUsages::UNIFORM,
		});
		*base_frame += buffer_length / 2;
		let bind_group = device.create_bind_group(&BindGroupDescriptor {
			label: None,
			layout: bind_group_layout,
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
		let mut encoder = device.create_command_encoder(&Default::default());
		{
			let mut cpass = encoder.begin_compute_pass(&Default::default());
			cpass.set_pipeline(pipeline.as_ref().expect("pipeline not initialized"));
			cpass.set_bind_group(0, &bind_group, &[]);
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
		adaptor
			.request_device(&Default::default(), None)
			.await
			.expect("failed to create device")
	})
}

fn create_bind_group_layout(device: &Device) -> BindGroupLayout {
	device.create_bind_group_layout(&BindGroupLayoutDescriptor {
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
