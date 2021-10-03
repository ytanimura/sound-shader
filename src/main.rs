use sound_shader::ShaderStreamDescriptor;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PlayConfig<P: AsRef<std::path::Path> = &'static str> {
    shader_source: P,
    inputs: Vec<P>,
    output: Option<P>,
}

fn main() { let args = std::env::args().collect::<Vec<_>>(); }
