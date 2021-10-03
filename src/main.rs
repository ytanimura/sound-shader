use sound_shader::ShaderStreamDescriptor;
use std::path::Path;
use std::sync::{
	atomic::{AtomicBool, Ordering},
	Arc, Mutex,
};

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlayConfig<P: AsRef<Path> = &'static str> {
	shader_source: P,
	resources: Vec<P>,
	output: Option<P>,
}

fn parse_args() -> Option<PlayConfig<String>> {
	use clap::*;
	use std::fs::File;
	let matches = App::new(crate_name!())
		.author(crate_authors!())
		.version(crate_version!())
		.about(crate_description!())
		.args(&[
			Arg::from_usage("[FILE] 'run shader source'"),
			Arg::from_usage(
				"-r --resources [FILE].. 'add audio resource, wav is supported.'",
			),
			Arg::from_usage(
				"-o --output [FILE] 'recording wav file'"
			),
			Arg::from_usage("-c --config [FILE] 'read configuration json'")
			.long_help("if a co")
			,
			Arg::from_usage(
				"--init 'init default config file \"default.json\" and prepare sample shader source \"sample.comp\"'",
			),
		])
		.get_matches();

	if matches.occurrences_of("init") == 1 {
		return None;
	}
	let mut config = if let Some(config) = matches.value_of("config") {
		let file = File::open(config).expect(&format!("not found: {}", config));
		serde_json::from_reader(file).expect(&format!("json parse error: {}", config))
	} else {
		PlayConfig::default()
	};
	matches
		.value_of("FILE")
		.map(|filename| config.shader_source = filename.to_string());
	matches
		.values_of("resources")
		.map(|r| config.resources = r.map(ToString::to_string).collect());
	matches
		.value_of("output")
		.map(|output| config.output = Some(output.to_string()));
	if config == PlayConfig::default() {
		let file = File::open("default.json").expect("not found: default.json");
		config = serde_json::from_reader(file).expect("json perse error: default.json");
	}
	Some(config)
}

fn init() {
	std::fs::write("sample.comp", include_str!("sample.comp")).unwrap();
	std::fs::write(
		"default.json",
		serde_json::to_vec_pretty(&PlayConfig {
			shader_source: "sample.comp",
			resources: Vec::new(),
			output: None,
		})
		.unwrap(),
	)
	.unwrap();
}

fn main() {
	let config = match parse_args() {
		Some(got) => got,
		None => {
			init();
			return;
		}
	};
	let shader_source = std::fs::read_to_string(&config.shader_source)
		.expect(&format!("not found: {}", config.shader_source));
	let record_buffer = config
		.output
		.as_ref()
		.map(|_| Arc::new(Mutex::new(Vec::new())));
	let desc = ShaderStreamDescriptor {
		audio_device: Default::default(),
		gpu_device: Default::default(),
		shader_source: &shader_source,
		sound_storages: &config.resources,
		record_buffer: record_buffer.as_ref().map(Arc::clone),
	};
	let (stream, stream_config) = sound_shader::stream(desc);
	cpal::traits::StreamTrait::play(&stream).unwrap();

	let running = Arc::new(AtomicBool::new(true));
	let running0 = Arc::clone(&running);
	ctrlc::set_handler(move || running0.store(false, Ordering::SeqCst))
		.expect("Error setting Ctrl-C handler");
	while running.load(Ordering::SeqCst) {
		std::thread::sleep(std::time::Duration::from_millis(10));
	}
	if let Some(record_mutex) = record_buffer {
		use hound::*;
		let buffer = record_mutex.lock().unwrap();
		let filename = config.output.unwrap();
		let spec = WavSpec {
			channels: 2,
			sample_rate: stream_config.sample_rate.0,
			bits_per_sample: 32,
			sample_format: SampleFormat::Float,
		};
		let mut writer = WavWriter::create(&filename, spec)
			.expect(&format!("failed to create output file: {}", &filename));
		buffer.iter().for_each(|s| writer.write_sample(*s).unwrap());
		writer.finalize().unwrap();
	}
}
