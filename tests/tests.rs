use hound::WavReader;
use sound_shader::{AudioDevice, ShaderStreamDescriptor};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn default_audio_device() -> (cpal::Device, cpal::SupportedStreamConfig) {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("failed to find output device")
        .unwrap();
    let config = device
        .default_output_config()
        .map_err(|e| format!("{}", e))
        .unwrap();
    println!("Output Device: {:?}", config);
    assert_eq!(config.channels(), 2, "channels must be 2.");
    (device, config)
}

#[test]
fn simple_sine() {
    let (device, config) = default_audio_device();
    let sample_rate = config.sample_rate().0 as usize;
    let record = Arc::new(Mutex::new(Vec::new()));
    let desc = ShaderStreamDescriptor {
        audio_device: AudioDevice::Custum { device, config },
        shader_source: include_str!("simple-sine.comp"),
        record_buffer: Some(Arc::clone(&record)),
        ..Default::default()
    };
    sound_shader::play(desc, Duration::from_secs(10)).unwrap();

    let len = sample_rate * 10;
    let answer = (0..len).flat_map(|i| {
        let t = 2.0 * std::f32::consts::PI * 440.0 * i as f32 / sample_rate as f32;
        vec![f32::cos(t), f32::sin(t)]
    });
    let record = record.lock().unwrap();
    assert!(record.len() >= len * 2);
    answer.zip(&*record).enumerate().for_each(|(i, (a, b))| {
        assert!(
            f32::abs(a - b) < 0.01,
            "frame: {}\nchannel: {}\nanswer: {}\nrecorded: {}",
            i / 2,
            i % 2,
            a,
            b
        );
    })
}

#[test]
fn wav_input() {
    let (device, config) = default_audio_device();
    let sample_rate = config.sample_rate().0;
    let record = Arc::new(Mutex::new(Vec::new()));
    let desc = ShaderStreamDescriptor {
        audio_device: AudioDevice::Custum { device, config },
        shader_source: include_str!("../examples/mix.comp"),
        record_buffer: Some(Arc::clone(&record)),
        sound_storages: &["resources/vanilla-vocal.wav", "resources/vanilla-inst.wav"],
        ..Default::default()
    };
    sound_shader::play(desc, Duration::from_secs(10)).unwrap();

    let wav0 = WavReader::open("resources/vanilla-vocal.wav").unwrap();
    let wav1 = WavReader::open("resources/vanilla-inst.wav").unwrap();
    let record = record.lock().unwrap();
    wav0.into_samples::<i16>()
        .zip(wav1.into_samples::<i32>())
        .enumerate()
        .take(882000)
        .for_each(|(i, (a, b))| {
            let a = a.unwrap() as f32 / f32::powi(2.0, 15);
            let b = b.unwrap() as f32 / f32::powi(2.0, 23);
            let rem = i % 2;
            let quot = (i / 2) as f32 * sample_rate as f32 / 44100.0;
            let idx = quot as usize * 2 + rem;
            let c0 = record[usize::clamp(idx, 0, record.len() - 1)];
            let c1 = record[usize::clamp(idx + 2, 0, record.len() - 1)];
            let c2 = record[usize::clamp(idx + 4, 0, record.len() - 1)];
            assert!(
                f32::abs(a + b - c0) < 0.01
                    || f32::abs(a + b - c1) < 0.01
                    || f32::abs(a + b - c2) < 0.01,
                "frame: {}\nchannel: {}\nanswer: {}\nrecorded: {}, {}, {}",
                i / 2,
                i % 2,
                a + b,
                c0,
                c1,
                c2,
            );
        });
}

#[test]
fn decryption() {
    let record0 = Arc::new(Mutex::new(Vec::new()));
    let desc0 = ShaderStreamDescriptor {
        shader_source: "vec2 mainSound(int samp, float time) { return soundTexture0(time); }",
        sound_storages: &["resources/vanilla-vocal.wav"],
        record_buffer: Some(Arc::clone(&record0)),
        ..Default::default()
    };
    let record1 = Arc::new(Mutex::new(Vec::new()));
    let desc1 = ShaderStreamDescriptor {
        shader_source: include_str!("decryption.comp"),
        sound_storages: &["resources/vanilla-vocal.wav"],
        record_buffer: Some(Arc::clone(&record1)),
        ..Default::default()
    };
    sound_shader::play(desc0, Duration::from_secs(10)).unwrap();
    sound_shader::play(desc1, Duration::from_secs(10)).unwrap();

    record0
        .lock()
        .unwrap()
        .iter()
        .zip(record1.lock().unwrap().windows(3))
        .enumerate()
        .for_each(|(i, (a, b))| {
            assert!(
                f32::abs(a - b[0]) < 0.01 || f32::abs(a - b[1]) < 0.01 || f32::abs(a - b[2]) < 0.01,
                "frame: {}\noriginal: {}\ndecryptions: {:?}",
                i / 2,
                a,
                b
            );
        });
}
