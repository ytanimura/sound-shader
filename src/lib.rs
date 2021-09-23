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

#[test]
fn play_sample() {
	play(include_str!("sample.comp"), Duration::from_millis(10000))
}
