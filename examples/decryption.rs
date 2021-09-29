fn main() {
	env_logger::init();
	let desc = sound_shader::ShaderStreamDescriptor {
		shader_source: include_str!("decryption.comp"),
		sound_storages: vec!["examples/vocal.wav"],
		..Default::default()
	};
	sound_shader::play(desc, std::time::Duration::from_secs(5))
}