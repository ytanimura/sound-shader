fn main() {
    env_logger::init();
    let desc = sound_shader::ShaderStreamDescriptor {
        shader_source: include_str!("wav-play.comp"),
        sound_storages: &["examples/vocal.wav", "examples/inst.wav"],
        ..Default::default()
    };
    sound_shader::play(desc, std::time::Duration::from_secs(100)).unwrap();
}
