extern crate cpal;
extern crate wgpu;

mod cpal_wrapper;
pub use cpal_wrapper::StreamFactory;
pub mod wgpu_wrapper;
pub use wgpu_wrapper::GPUDirector;

use std::time::Duration;
pub fn play(code: &str, duration: Duration) {
	use cpal::traits::StreamTrait;
	let sf = StreamFactory::default_factory().unwrap();
	let config = sf.config();
	let mut director = GPUDirector::new();
	director.read_source(code);

	let stream = sf
		.create_stream(move |len| director.render(config.sample_rate.0 as u32, len as u32))
		.unwrap();
	stream.play().unwrap();
	std::thread::sleep(duration);
}

pub fn stream(code: &str) -> cpal::Stream {
	let sf = StreamFactory::default_factory().unwrap();
	let config = sf.config();
	let mut director = GPUDirector::new();
	director.read_source(code);

	sf.create_stream(move |len| director.render(config.sample_rate.0 as u32, len as u32))
		.unwrap()
}

pub fn write_buffer(code: &str, sample_rate: u32, duration: Duration) -> Vec<f32> {
	let mut director = GPUDirector::new();
	let time = duration.as_secs_f64();
	let buffer_length = (sample_rate as f64 * time) as u32 * 2;
	director.read_source(code);
	director.render(sample_rate, buffer_length)
}

#[test]
fn play_sample() {
	play(include_str!("sample.comp"), Duration::from_millis(10000))
}

#[test]
fn sample_stream() {
	use cpal::traits::StreamTrait;
	let stream = stream(include_str!("sample.comp"));
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
	buffer.into_iter().for_each(|s| writer.write_sample(s).unwrap());
	writer.finalize().unwrap()
}
