use hound::*;
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
				let max = f32::powi(2.0, *bps as i32);
				samples.next().map(|val| val.unwrap() as f32 / max)
			}
			WrapperSamples::I16(samples, bps) => {
				let max = f32::powi(2.0, *bps as i32);
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
	spec: WavSpec,
}

impl WavTextureMaker {
	pub fn try_new<P: AsRef<Path>>(filename: P) -> Result<Self, String> {
		let wav = WavReader::open(filename).map_err(|e| format!("{}", e))?;
		let spec = wav.spec();
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
		})
	}

	pub fn spec(&self) -> WavSpec {
		self.spec
	}

	pub fn buffer_len(&self) -> usize { self.buffer.len() }

	pub fn reserve(&mut self, len: usize) {
		(0..len).for_each(|_| {
			let a = match self.samples.next() {
				Some(val) => val,
				None => 0.0,
			};
			self.buffer.push(a);
		})
	}

	pub fn next_buffer(&mut self, len: usize) -> Vec<f32> {
		if self.buffer.len() < len {
			self.reserve(len - self.buffer.len());
		}
		let vec = self.buffer.split_off(len);
		let res = self.buffer.clone();
		self.buffer = vec;
		res
	}
}
