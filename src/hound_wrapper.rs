use hound::*;
use rustfft::{num_complex::Complex, FftPlanner};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::result::Result;

enum WrapperSamples {
	I16(WavIntoSamples<BufReader<File>, i16>, u16),
	I32(WavIntoSamples<BufReader<File>, i32>, u16),
	F32(WavIntoSamples<BufReader<File>, f32>),
}

impl Iterator for WrapperSamples {
	type Item = f32;
	fn next(&mut self) -> Option<f32> {
		match self {
			WrapperSamples::F32(samples) => samples.next().map(|val| val.unwrap()),
			WrapperSamples::I32(samples, bps) => {
				let max = f32::powi(2.0, *bps as i32 - 1);
				samples.next().map(|val| val.unwrap() as f32 / max)
			}
			WrapperSamples::I16(samples, bps) => {
				let max = f32::powi(2.0, *bps as i32 - 1);
				samples.next().map(|val| val.unwrap() as f32 / max)
			}
		}
	}
	fn size_hint(&self) -> (usize, Option<usize>) {
		match self {
			WrapperSamples::F32(samples) => samples.size_hint(),
			WrapperSamples::I16(samples, _) => samples.size_hint(),
			WrapperSamples::I32(samples, _) => samples.size_hint(),
		}
	}
}

impl ExactSizeIterator for WrapperSamples {}

pub struct WavTextureMaker {
	samples: WrapperSamples,
	buffer: Vec<f32>,
	fft_buffer: Vec<Complex<f32>>,
	spec: WavSpec,
}

impl WavTextureMaker {
	pub fn try_new<P: AsRef<Path>>(filename: P) -> Result<Self, String> {
		let wav = WavReader::open(filename).map_err(|e| format!("{}", e))?;
		let spec = wav.spec();
		println!("Audio Input: {:?}", spec);
		let samples = match spec.sample_format {
			SampleFormat::Float => WrapperSamples::F32(wav.into_samples()),
			SampleFormat::Int => {
				if spec.bits_per_sample > 16 {
					WrapperSamples::I32(wav.into_samples(), spec.bits_per_sample)
				} else {
					WrapperSamples::I16(wav.into_samples(), spec.bits_per_sample)
				}
			}
		};
		Ok(Self {
			samples,
			spec,
			buffer: Vec::new(),
			fft_buffer: Vec::new(),
		})
	}

	pub fn spec(&self) -> WavSpec {
		self.spec
	}

	pub fn buffer_len(&self) -> usize {
		self.buffer.len()
	}

	pub fn reserve(&mut self, len: usize) {
		let Self {
			buffer,
			samples,
			spec,
			fft_buffer,
		} = self;
		buffer.extend((0..len).map(|_| match samples.next() {
			Some(val) => val,
			None => 0.0,
		}));
		let unit_len = spec.sample_rate as usize / 10;
		let delta = buffer.len() / spec.channels as usize - fft_buffer.len();
		if delta > unit_len {
			let delta = delta - delta % unit_len;
			let mut planner = FftPlanner::new();
			let fft = planner.plan_fft_forward(unit_len);
			let mut new_buffer: Vec<_> = match spec.channels {
				1 => buffer[fft_buffer.len()..fft_buffer.len() + delta]
					.iter()
					.map(|x| Complex { re: *x, im: 0.0 })
					.collect(),
				2 => buffer[fft_buffer.len() * 2..(fft_buffer.len() + delta) * 2]
					.chunks(2)
					.map(|x| Complex { re: x[0], im: x[1] })
					.collect(),
				_ => panic!("unknown channels!"),
			};
			fft.process(&mut new_buffer);
			fft_buffer.extend(new_buffer);
		}
	}

	pub fn next_buffer(&mut self, len: usize) -> Vec<[f32; 4]> {
		if self.buffer.len() < len {
			self.reserve(len - self.buffer.len());
		}
		let vec = self.buffer.split_off(len * self.spec.channels as usize);
		let res0 = self.buffer.clone();
		self.buffer = vec;
		let vec = self.fft_buffer.split_off(len);
		let res1 = self.fft_buffer.clone();
		self.fft_buffer = vec;
		match self.spec.channels {
			1 => res0
				.iter()
				.zip(res1)
				.map(|(x, y)| [*x, 0.0, y.re, y.im])
				.collect(),
			2 => res0
				.chunks(2)
				.zip(res1)
				.map(|(x, y)| [x[0], x[1], y.re, y.im])
				.collect(),
			_ => panic!("unknown channels!"),
		}
	}
}
