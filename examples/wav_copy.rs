use hound::*;
use sound_shader::ShaderStreamDescriptor;
use std::time::Duration;

fn main() {
	env_logger::init();
	let desc = ShaderStreamDescriptor {
		shader_source: include_str!("wav-play.comp"),
		sound_storages: &["examples/vocal.wav", "examples/inst.wav"],
		..Default::default()
	};
	let buffer = sound_shader::write_buffer(desc, 44100, Duration::from_secs(100));
	let spec = WavSpec {
		channels: 2,
		sample_rate: 44100,
		bits_per_sample: 32,
		sample_format: SampleFormat::Float,
	};
	let mut writer = WavWriter::create("mixed.wav", spec).unwrap();
	buffer
		.into_iter()
		.for_each(|s| writer.write_sample(s).unwrap());
	writer.finalize().unwrap()
}
