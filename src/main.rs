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
    silent: Option<f32>,
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
			Arg::from_usage("-c --config [FILE] 'read configuration json'"),
			Arg::from_usage(
				"--init 'init default config file \"default.json\" and prepare sample shader source \"sample.comp\"'",
			),
			Arg::from_usage(
				"-s --silent [SECONDS] 'not play, just recording.'"
			),
		])
		.get_matches();

    if matches.occurrences_of("init") == 1 {
        return None;
    }
    let mut config = if let Some(config) = matches.value_of("config") {
        let file = File::open(config).unwrap_or_else(|_| panic!("not found: {}", config));
        serde_json::from_reader(file).unwrap_or_else(|_| panic!("json parse error: {}", config))
    } else {
        PlayConfig::default()
    };
    if let Some(filename) = matches.value_of("FILE") {
        config.shader_source = filename.to_string();
    }
    if let Some(r) = matches.values_of("resources") {
        config.resources = r.map(ToString::to_string).collect()
    }
    if let Some(output) = matches.value_of("output") {
        config.output = Some(output.to_string());
    }
    if let Some(seconds) = matches.value_of("silent") {
        let seconds: f32 = seconds.parse().expect("could not parse duration");
        config.silent = Some(seconds);
    }
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
            silent: None,
        })
        .unwrap(),
    )
    .unwrap();
}

fn silent<P: AsRef<Path>>(desc: ShaderStreamDescriptor<P>, filename: P, seconds: f32) {
    use hound::*;
    let buffer =
        sound_shader::write_buffer(desc, 44100, std::time::Duration::from_secs_f32(seconds));
    let spec = WavSpec {
        channels: 2,
        sample_rate: 44100,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    let mut writer = WavWriter::create(filename, spec).unwrap();
    buffer
        .into_iter()
        .for_each(|s| writer.write_sample(s).unwrap());
    writer.finalize().unwrap()
}

fn play<P: AsRef<Path>>(
    desc: ShaderStreamDescriptor<P>,
    record_buffer: Option<Arc<Mutex<Vec<f32>>>>,
    output: Option<P>,
) {
    let (stream, stream_config) = sound_shader::stream(desc);
    cpal::traits::StreamTrait::play(&stream).unwrap();

    let running = Arc::new(AtomicBool::new(true));
    let running0 = Arc::clone(&running);
    ctrlc::set_handler(move || running0.store(false, Ordering::SeqCst))
        .expect("Error setting Ctrl-C handler");
    println!("Hit CTRL-C to stop playing");
    while running.load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    drop(stream);
    if let Some(record_mutex) = record_buffer {
        use hound::*;
        let buffer = record_mutex.lock().unwrap();
        let filename = output.unwrap();
        let spec = WavSpec {
            channels: 2,
            sample_rate: stream_config.sample_rate.0,
            bits_per_sample: 32,
            sample_format: SampleFormat::Float,
        };
        let mut writer = WavWriter::create(&filename, spec).unwrap_or_else(|_| {
            panic!(
                "failed to create output file: {}",
                filename.as_ref().display()
            )
        });
        buffer.iter().for_each(|s| writer.write_sample(*s).unwrap());
        writer.finalize().unwrap();
    }
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
        .unwrap_or_else(|_| panic!("not found: {}", config.shader_source));
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
    match config.silent {
        None => play(desc, record_buffer, config.output),
        Some(seconds) => {
            let filename = config.output.expect("Output wav is not specified.");
            silent(desc, filename, seconds);
        }
    }
}
