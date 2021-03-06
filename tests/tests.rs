use hound::WavReader;
use sound_shader::{AudioDevice, ShaderStreamDescriptor};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[test]
fn simple_sine() {
    let record = Arc::new(Mutex::new(Vec::new()));
    let desc = ShaderStreamDescriptor {
        audio_device: AudioDevice::Default,
        shader_source: include_str!("simple-sine.comp"),
        record_buffer: Some(Arc::clone(&record)),
        ..Default::default()
    };
    let config = sound_shader::play(desc, Duration::from_secs(10)).unwrap();

    let sample_rate = config.sample_rate.0 as usize;
    let len = sample_rate * 10;
    let answer = (0..len).flat_map(|i| {
        let t = 2.0 * std::f32::consts::PI * 440.0 * i as f32 / sample_rate as f32;
        vec![f32::cos(t), f32::sin(t)]
    });
    let record = record.lock().unwrap();
    assert!(
        record.len() + sample_rate / 10 >= len * 2,
        "invalid record length:\n record length: {}\nrequired length: {}",
        record.len(),
        len * 2
    );
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
fn silent_and_record() {
    let record = Arc::new(Mutex::new(Vec::new()));
    let duration = Duration::from_secs_f32(6.28);
    let desc = ShaderStreamDescriptor {
        audio_device: AudioDevice::Default,
        shader_source: include_str!("simple-sine.comp"),
        record_buffer: Some(Arc::clone(&record)),
        ..Default::default()
    };
    let config = sound_shader::play(desc, duration).unwrap();
    let desc = ShaderStreamDescriptor {
        audio_device: AudioDevice::Default,
        shader_source: include_str!("simple-sine.comp"),
        ..Default::default()
    };
    let sample_rate = config.sample_rate.0 as usize;
    let buffer = sound_shader::write_buffer(desc, sample_rate as u32, duration);
    let record = record.lock().unwrap();
    assert!(
        buffer.len() < record.len() + sample_rate / 10,
        "buffer length mismatch\nrecord: {}\nsilent: {}",
        record.len(),
        buffer.len()
    );
    assert!(
        record.len() < buffer.len() + sample_rate / 10,
        "buffer length mismatch\nrecord: {}\nsilent: {}",
        record.len(),
        buffer.len()
    );
    buffer
        .iter()
        .zip(&*record)
        .for_each(|(a, b)| assert!(f32::abs(a - b) < 0.01));
}

#[test]
fn wav_input() {
    let record = Arc::new(Mutex::new(Vec::new()));
    let desc = ShaderStreamDescriptor {
        audio_device: AudioDevice::Default,
        shader_source: include_str!("../examples/mix.comp"),
        record_buffer: Some(Arc::clone(&record)),
        sound_storages: &["resources/vanilla-vocal.wav", "resources/vanilla-inst.wav"],
        ..Default::default()
    };
    let config = sound_shader::play(desc, Duration::from_secs(10)).unwrap();

    let sample_rate = config.sample_rate.0;
    let wav0: Vec<f32> = WavReader::open("resources/vanilla-vocal.wav")
        .unwrap()
        .into_samples::<i16>()
        .take(88500)
        .map(|a| a.unwrap() as f32 / f32::powi(2.0, 15))
        .collect();
    let wav1: Vec<f32> = WavReader::open("resources/vanilla-inst.wav")
        .unwrap()
        .into_samples::<i32>()
        .take(88500)
        .map(|a| a.unwrap() as f32 / f32::powi(2.0, 31))
        .collect();
    record
        .lock()
        .unwrap()
        .iter()
        .take(88200)
        .enumerate()
        .for_each(|(i, a)| {
            let t = (i / 2) as f32 / sample_rate as f32;
            let idx = (t * 44100.0) as usize;
            let p = f32::fract(t * 44100.0);
            let audio0 = wav0[idx] * (1.0 - p) + wav0[idx + 1] * p;
            let audio1 = wav1[idx] * (1.0 - p) + wav1[idx + 1] * p;
            assert!(
                f32::abs(audio0 + audio1 - a) < 0.01,
                "frame: {}\nchannel: {}\nanswer: {}\nrecorded: {}",
                i / 2,
                i % 2,
                audio0 + audio1,
                a,
            );
        });
}

#[test]
fn decryption() {
    let record0 = Arc::new(Mutex::new(Vec::new()));
    let desc0 = ShaderStreamDescriptor {
        shader_source: include_str!("texel.comp"),
        sound_storages: &["resources/this-little-girl.wav"],
        record_buffer: Some(Arc::clone(&record0)),
        ..Default::default()
    };
    let record1 = Arc::new(Mutex::new(Vec::new()));
    let desc1 = ShaderStreamDescriptor {
        shader_source: include_str!("../examples/decryption.comp"),
        sound_storages: &["resources/this-little-girl.wav"],
        record_buffer: Some(Arc::clone(&record1)),
        ..Default::default()
    };
    sound_shader::play(desc0, Duration::from_secs(10)).unwrap();
    sound_shader::play(desc1, Duration::from_secs(10)).unwrap();

    let record0 = record0.lock().unwrap();
    let record1 = record1.lock().unwrap();
    record0
        .iter()
        .zip(&*record1)
        .enumerate()
        .for_each(|(i, (a, b))| {
            assert!(
                f32::abs(a - b) < 0.01,
                "frame: {}\noriginal: {}\ndecryptions: {:?}",
                i / 2,
                a,
                b
            );
        });
}
