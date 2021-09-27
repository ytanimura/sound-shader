use std::sync::{Arc, Mutex};
use std::time::Duration;

mod cpal_wrapper;
use cpal_wrapper::StreamFactory;
mod wgpu_wrapper;
use wgpu_wrapper::GPUDirector;
mod hound_wrapper;
use hound_wrapper::WavTextureMaker;

/// Options for `cpal` audio device.
pub enum AudioDevice {
	/// Loads the default audio device internally
	Default,
	/// Set the audio device manually
	Custum {
		/// Audio device
		device: cpal::Device,
		/// Stream configuation. `channels` must be 2.
		config: cpal::SupportedStreamConfig,
	},
}

/// Options for `wgpu` GPU device.
pub enum GpuDevice {
	/// Loads the default GPU device internally
	Default,
	/// Set the GPU device manually
	Custum {
		/// GPU device
		device: Arc<wgpu::Device>,
		/// GPU device queue
		queue: Arc<wgpu::Queue>,
	},
}

/// Configuation for shader stream
pub struct ShaderStreamDescriptor<'a, P: AsRef<std::path::Path> = &'static str> {
	/// Options for `cpal` audio device.
	pub audio_device: AudioDevice,
	/// Options for `wgpu` GPU device.
	pub gpu_device: GpuDevice,
	/// Sound shader code
	pub shader_source: &'a str,
	pub sound_storages: Vec<P>,
}

impl<'a> Default for ShaderStreamDescriptor<'a> {
	fn default() -> Self {
		Self {
			audio_device: AudioDevice::Default,
			gpu_device: GpuDevice::Default,
			shader_source: "",
			sound_storages: Vec::new(),
		}
	}
}

/// Creates output audio stream
pub fn stream(desc: ShaderStreamDescriptor) -> cpal::Stream {
	let sf = match desc.audio_device {
		AudioDevice::Default => StreamFactory::default_factory().unwrap(),
		AudioDevice::Custum { device, config } => StreamFactory::new(device, config),
	};
	let config = sf.config();
	let sound_storages = desc
		.sound_storages
		.into_iter()
		.map(|path| Arc::new(Mutex::new(WavTextureMaker::try_new(path).unwrap())))
		.collect();
	let mut director = match desc.gpu_device {
		GpuDevice::Default => GPUDirector::from_default_device(desc.shader_source, sound_storages),
		GpuDevice::Custum { device, queue } => {
			GPUDirector::new(device, queue, desc.shader_source, sound_storages)
		}
	};

	let sample_rate = config.sample_rate.0 as u32;
	let buffer0 = Arc::new(Mutex::new(director.render(sample_rate, sample_rate * 2)));
	let buffer1 = Arc::clone(&buffer0);

	std::thread::spawn(move || loop {
		let len = buffer0.lock().unwrap().len() as u32;
		if len < sample_rate {
			let vec = director.render(sample_rate, sample_rate * 2);
			buffer0.lock().unwrap().extend(vec);
		}
		std::thread::sleep(Duration::from_millis(200));
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

/// Creates output audio stream and play it for `duration`.
pub fn play(desc: ShaderStreamDescriptor, duration: Duration) {
	use cpal::traits::StreamTrait;
	let stream = stream(desc);
	stream.play().unwrap();
	std::thread::sleep(duration);
}

/// Returns buffer for play the shader. `ShaderStreamDescriptor::audio_device` is ignored.
pub fn write_buffer(
	desc: ShaderStreamDescriptor,
	sample_rate: u32,
	duration: Duration,
) -> Vec<f32> {
	let sound_storages = desc
		.sound_storages
		.into_iter()
		.map(|path| Arc::new(Mutex::new(WavTextureMaker::try_new(path).unwrap())))
		.collect();
	let mut director = match desc.gpu_device {
		GpuDevice::Default => GPUDirector::from_default_device(desc.shader_source, sound_storages),
		GpuDevice::Custum { device, queue } => {
			GPUDirector::new(device, queue, desc.shader_source, sound_storages)
		}
	};
	let time = duration.as_secs_f64();
	let buffer_length = (sample_rate as f64 * time) as u32 * 2;
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
	env_logger::init();
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
		ShaderStreamDescriptor {
			shader_source: include_str!("sample.comp"),
			..Default::default()
		},
		spec.sample_rate,
		Duration::from_secs(10),
	);
	buffer
		.into_iter()
		.for_each(|s| writer.write_sample(s).unwrap());
	writer.finalize().unwrap()
}
