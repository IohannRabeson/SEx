use std::sync::Arc;

use iced::{mouse, widget::{canvas::{self, Frame, Path}, Canvas}, Element, Length, Point, Renderer, Size, Theme};
use rustfft::{num_complex::Complex, Fft, FftPlanner};

use crate::ui;

const FFT_SIZE: usize = 16384 / 4;
const MIN_FREQ: f32 = 20.0;
const MAX_FREQ: f32 = 10000.0;

pub struct Spectrum {
    scratch_buffer: Box<[Complex<f32>]>,
    fft_input_buffer: Box<[Complex<f32>]>,
    temporary: Vec<f32>,
    window: Box<[f32]>,
    fft: Arc<dyn Fft<f32>>,
    sample_rate: usize,
    display_buffer: Vec<f32>,
}

impl Spectrum {
    pub fn new() -> Self {
        let mut fft_planer = FftPlanner::new();
        let fft = fft_planer.plan_fft_forward(FFT_SIZE);
        Self { 
            scratch_buffer: Box::new([Complex::ZERO; FFT_SIZE]),
            fft_input_buffer: Box::new([Complex::ZERO; FFT_SIZE]),
            window: apodize::hanning_iter(FFT_SIZE).map(|n|n as f32).collect::<Vec<_>>().into_boxed_slice(),
            temporary: Vec::with_capacity(FFT_SIZE),
            fft,
            sample_rate: 0,
            display_buffer: Vec::with_capacity(FFT_SIZE),
        }
    }
    
    pub fn update(&mut self, message: Message) {
        match message {
            Message::Buffer(buffer) => {
                if buffer.is_empty() {
                    self.display_buffer.clear();
                    return
                }

                self.temporary.extend(buffer.into_iter());

                if self.temporary.len() >= FFT_SIZE {
                    for ((result, window), fft_input_buffer) in self.temporary.iter().take(FFT_SIZE).zip(self.window.iter()).zip(self.fft_input_buffer.iter_mut()) {
                        fft_input_buffer.re = result * window;
                        fft_input_buffer.im = 0f32;
                    }
                    self.fft.process_with_scratch(&mut self.fft_input_buffer, &mut self.scratch_buffer);
                    
                    let bin_resolution = self.sample_rate as f32 / self.fft_input_buffer.len() as f32;

                    self.display_buffer.clear();

                    for (index, result) in self.fft_input_buffer.iter().take(FFT_SIZE / 2).enumerate() {
                        let frequency = bin_resolution * index as f32;

                        if frequency >= MIN_FREQ && frequency <= MAX_FREQ {
                            self.display_buffer.push((result.re * result.re + result.im * result.im).sqrt());
                        }
                    }
                    self.temporary.drain(0..FFT_SIZE);
                }
            },
            Message::SampleRateChanged(sample_rate) => {
                self.sample_rate = sample_rate;
            },
        }
    }

    pub fn view(&self) -> Element<crate::Message> {
        Canvas::new(self)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Buffer(Vec<f32>),
    SampleRateChanged(usize),
}

impl canvas::Program<crate::Message> for Spectrum {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let mut frame = Frame::new(renderer, bounds.size());
        let bin_count = self.display_buffer.len() / 2;
        let bin_width = frame.width() / bin_count as f32;

        for (bin_index, bin_height) in self.display_buffer.iter().enumerate() {
            let bin_left = bin_index as f32 * bin_width;
            let bin_top = frame.height() - bin_height;

            frame.fill_rectangle(Point::new(bin_left, bin_top), Size::new(bin_width, *bin_height), theme.palette().primary);
        }

        let path = Path::line(Point::ORIGIN, Point::new(frame.width(), 0.0));
        let stroke = ui::separation_line_stroke(theme);

        frame.stroke(&path, stroke);

        vec![frame.into_geometry()]
    }
}