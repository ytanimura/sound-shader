fn play_wav(secs: u64) {
	env_logger::init();
	let desc = sound_shader::ShaderStreamDescriptor {
		shader_source: include_str!("wav-play.comp"),
		sound_storages: vec!["examples/vocal.wav", "examples/inst.wav"],
		..Default::default()
	};
	sound_shader::play(desc, std::time::Duration::from_secs(secs))
}

fn main() { play_wav(120) }

#[test]
fn test_play_wav() { play_wav(5) }
