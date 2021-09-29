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
	/// File names of sound storages
	pub sound_storages: &'a [P],
	/// Buffer for recording result
	pub record_buffer: Option<Arc<Mutex<Vec<f32>>>>,
}

impl<'a> Default for ShaderStreamDescriptor<'a> {
	fn default() -> Self {
		Self {
			audio_device: AudioDevice::Default,
			gpu_device: GpuDevice::Default,
			shader_source: "",
			sound_storages: &[],
			record_buffer: None,
		}
	}
}

/// Creates output audio stream
pub fn stream(mut desc: ShaderStreamDescriptor) -> cpal::Stream {
	let sf = match desc.audio_device {
		AudioDevice::Default => StreamFactory::default_factory().unwrap(),
		AudioDevice::Custum { device, config } => StreamFactory::new(device, config),
	};
	let config = sf.config();
	let sound_storages = desc
		.sound_storages
		.into_iter()
		.map(|path| {
			let mut maker = WavTextureMaker::try_new(path).unwrap();
			let spec = maker.spec();
			maker.reserve(spec.sample_rate as usize * 3);
			Arc::new(Mutex::new(maker))
		})
		.collect::<Vec<_>>();
	let sound_storages0 = sound_storages.clone();
	let mut director = match desc.gpu_device {
		GpuDevice::Default => GPUDirector::from_default_device(desc.shader_source, sound_storages),
		GpuDevice::Custum { device, queue } => {
			GPUDirector::new(device, queue, desc.shader_source, sound_storages)
		}
	};

	let sample_rate = config.sample_rate.0 as u32;
	let buffer0 = Arc::new(Mutex::new(director.render(sample_rate, sample_rate * 2)));
	let buffer1 = Arc::clone(&buffer0);

	if !sound_storages0.is_empty() {
		std::thread::spawn(move || loop {
			sound_storages0.iter().for_each(|wav| {
				let mut wav = wav.lock().unwrap();
				let spec = wav.spec();
				let unit_len = spec.sample_rate as usize;
				let current_len = wav.buffer_len();
				if current_len < unit_len * 2 {
					wav.reserve(unit_len * 3 - current_len);
				}
			});
			std::thread::sleep(Duration::from_millis(100));
		});
	}

	std::thread::spawn(move || loop {
		let len = buffer0.lock().unwrap().len() as u32;
		if len < sample_rate {
			let vec = director.render(sample_rate, sample_rate * 2);
			buffer0.lock().unwrap().extend(vec);
		}
		std::thread::sleep(Duration::from_millis(200));
	});

	let record = desc.record_buffer.take();
	sf.create_stream(move |len| match buffer1.lock() {
		Err(e) => {
			eprintln!("{}", e);
			vec![0.0; len]
		}
		Ok(mut buffer) => {
			if buffer.len() < len {
				eprintln!(
					"buffer length is not enough.\nbuffer length: {}\nrequired: {}",
					buffer.len(),
					len
				);
				buffer.resize(len, 0.0);
			}
			let latter = buffer.split_off(len);
			let front = buffer.clone();
			if let Some(record) = record.as_ref() {
				match record.try_lock() {
					Ok(mut record) => record.extend(&front),
					Err(_) => eprintln!("record buffer is locked"),
				}
			}
			*buffer = latter;
			front
		}
	})
	.unwrap()
}

/// Creates output audio stream and play it for `duration`.
pub fn play(desc: ShaderStreamDescriptor, duration: Duration) -> Result<(), String> {
	use cpal::traits::StreamTrait;
	let stream = stream(desc);
	stream.play().map_err(|e| format!("{}", e))?;
	std::thread::sleep(duration);
	Ok(())
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
		.map(|path| {
			let mut wav = WavTextureMaker::try_new(path).unwrap();
			let spec = wav.spec();
			let len = spec.sample_rate as usize * spec.channels as usize * (duration.as_secs() as usize + 1);
			wav.reserve(len);
			Arc::new(Mutex::new(wav))
		})
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
