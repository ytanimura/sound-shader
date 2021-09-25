extern crate cpal;
extern crate wgpu;

use std::sync::{Arc, Mutex};
use std::time::Duration;

mod cpal_wrapper;
pub use cpal_wrapper::StreamFactory;
pub mod wgpu_wrapper;
pub use wgpu_wrapper::GPUDirector;

pub enum AudioDevice {
	Default,
	Custum {
		device: cpal::Device,
		config: cpal::SupportedStreamConfig,
	},
}

pub enum GpuDevice {
	Default,
	Custum {
		device: Arc<wgpu::Device>,
		queue: Arc<wgpu::Queue>,
	},
}

pub struct ShaderStreamDescriptor<'a> {
	audio_device: AudioDevice,
	gpu_device: GpuDevice,
	shader_source: &'a str,
}

impl<'a> Default for ShaderStreamDescriptor<'a> {
	fn default() -> Self {
		Self {
			audio_device: AudioDevice::Default,
			gpu_device: GpuDevice::Default,
			shader_source: ""
		}
	}
}

pub fn stream(desc: ShaderStreamDescriptor) -> cpal::Stream {
	let sf = match desc.audio_device {
		AudioDevice::Default => StreamFactory::default_factory().unwrap(),
		AudioDevice::Custum { device, config } => StreamFactory::new(device, config),
	};
	let config = sf.config();
	let mut director = match desc.gpu_device {
		GpuDevice::Default => GPUDirector::from_default_device(),
		GpuDevice::Custum { device, queue } => GPUDirector::new(device, queue),
	};
	director.read_source(desc.shader_source);

	let sample_rate = config.sample_rate.0 as u32;
	let buffer0 = Arc::new(Mutex::new(director.render(sample_rate, sample_rate * 2)));
	let buffer1 = Arc::clone(&buffer0);

	std::thread::spawn(move || {
		loop {
			let len = buffer0.lock().unwrap().len() as u32;
			if len < sample_rate {
				let vec = director.render(sample_rate, sample_rate * 2);
				buffer0.lock().unwrap().extend(vec);
			}
			std::thread::sleep(Duration::from_millis(200));
		}
	});

	sf.create_stream(move |len| match buffer1.lock() {
		Err(e) => {
			eprintln!("{}", e);
			vec![0.0; len]
		}
		Ok(mut buffer) => {
			if buffer.len() < len {
				eprintln!("buffer length is not enough");
				buffer.resize(len, 0.0);
			}
			let latter = buffer.split_off(len);
			let front = buffer.clone();
			*buffer = latter;
			front
		}
	})
	.unwrap()
}

pub fn play(desc: ShaderStreamDescriptor, duration: Duration) {
	use cpal::traits::StreamTrait;
	let stream = stream(desc);
	stream.play().unwrap();
	std::thread::sleep(duration);
}

pub fn write_buffer(code: &str, sample_rate: u32, duration: Duration) -> Vec<f32> {
	let mut director = GPUDirector::from_default_device();
	let time = duration.as_secs_f64();
	let buffer_length = (sample_rate as f64 * time) as u32 * 2;
	director.read_source(code);
	director.render(sample_rate, buffer_length)
}

#[test]
fn play_sample() {
	let desc = ShaderStreamDescriptor {
		shader_source: include_str!("sample.comp"),
		..Default::default()
	};
	play(desc, Duration::from_millis(10000))
}

#[test]
fn sample_stream() {
	use cpal::traits::StreamTrait;
	let desc = ShaderStreamDescriptor {
		shader_source: include_str!("sample.comp"),
		..Default::default()
	};
	let stream = stream(desc);
	stream.play().unwrap();
	std::thread::sleep(Duration::from_millis(1000));
	stream.pause().unwrap();
	std::thread::sleep(Duration::from_millis(1000));
	stream.play().unwrap();
	std::thread::sleep(Duration::from_millis(1000));
}

#[test]
fn write_sample() {
	use hound::*;
	let spec = WavSpec {
		channels: 2,
		sample_rate: 44100,
		bits_per_sample: 32,
		sample_format: SampleFormat::Float,
	};
	let mut writer = WavWriter::create("sample.wav", spec).unwrap();
	let buffer = write_buffer(
		include_str!("sample.comp"),
		spec.sample_rate,
		Duration::from_secs(10),
	);
	buffer
		.into_iter()
		.for_each(|s| writer.write_sample(s).unwrap());
	writer.finalize().unwrap()
}
