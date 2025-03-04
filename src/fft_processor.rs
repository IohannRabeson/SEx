use std::sync::Arc;

use rustfft::{num_complex::Complex, Fft, FftPlanner};

pub struct FftProcessor<const FFT_SIZE: usize> {
    scratch_buffer: Box<[Complex<f32>]>,
    fft_input_buffer: Box<[Complex<f32>]>,
    temporary: Vec<f32>,
    window: Box<[f32]>,
    fft: Arc<dyn Fft<f32>>,
}

impl<const FFT_SIZE: usize> FftProcessor<FFT_SIZE> {
    pub fn new() -> Self {
        let mut fft_planer = FftPlanner::new();
        let fft = fft_planer.plan_fft_forward(FFT_SIZE);

        Self {
            scratch_buffer: Box::new([Complex::ZERO; FFT_SIZE]),
            fft_input_buffer: Box::new([Complex::ZERO; FFT_SIZE]),
            window: apodize::hanning_iter(FFT_SIZE)
                .map(|n| n as f32)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            temporary: Vec::with_capacity(FFT_SIZE),
            fft,
        }
    }

    pub fn reset(&mut self) {
        self.temporary.clear();
    }

    pub fn process(&mut self, buffer: &[f32]) -> Option<std::slice::Iter<'_, Complex<f32>>> {
        self.temporary.extend(buffer);

        if self.temporary.len() >= FFT_SIZE {
            for ((result, window), fft_input_buffer) in self
                .temporary
                .iter()
                .take(FFT_SIZE)
                .zip(self.window.iter())
                .zip(self.fft_input_buffer.iter_mut())
            {
                fft_input_buffer.re = result * window;
                fft_input_buffer.im = 0f32;
            }
            self.fft
                .process_with_scratch(&mut self.fft_input_buffer, &mut self.scratch_buffer);

            self.temporary.drain(0..FFT_SIZE);

            Some(self.fft_input_buffer.iter())
        } else {
            None
        }
    }

    pub const fn fft_size(&self) -> usize {
        FFT_SIZE
    }
}
