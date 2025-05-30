use std::sync::Arc;

use iced::widget::canvas;
use iced::{
    mouse,
    widget::canvas::{Frame, Path},
    Element, Length, Point, Renderer, Size, Theme,
};

use crate::{fft_processor::FftProcessor, ui};

/// FFT size, bigger FFT causes slower updates.
/// 2048 gives good results, there are enough bins, and it's not too slow.
/// The priority here is the visual result.
const FFT_SIZE: usize = 2048;
/// 1023.75037 is the value I get for the bin of frequency 9996.094 (which is the maximum frequency
/// displayed) if I play a generated sine at 9996.094Hz at 0dB.
/// So I'm rounding to 1024 to be sure its big enough.
const MAGNITUDE_ZERO_DB: f32 = 1024.0;
const MIN_FREQ: f32 = 20.0;
const MAX_FREQ: f32 = 22000.0;

pub struct Spectrum {
    processor: FftProcessor<2048>,
    sample_rate: usize,
    display_buffer: Vec<f32>,
}

impl Spectrum {
    pub fn new() -> Self {
        Self {
            processor: FftProcessor::new(),
            sample_rate: 0,
            display_buffer: Vec::with_capacity(FFT_SIZE),
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Buffer(buffer) => {
                if buffer.is_empty() {
                    self.display_buffer.clear();
                    return;
                }

                self.process_buffer(buffer);
            }
            Message::SampleRateChanged(sample_rate) => {
                self.sample_rate = sample_rate;
                self.processor.reset();
            }
        }
    }

    fn process_buffer(&mut self, buffer: Arc<Vec<f32>>) {
        let bin_resolution = self.sample_rate as f32 / self.processor.fft_size() as f32;

        if let Some(results) = self.processor.process(&buffer) {
            self.display_buffer.clear();

            for (index, result) in results.take(FFT_SIZE / 2).enumerate() {
                let frequency = bin_resolution * index as f32;

                if (MIN_FREQ..=MAX_FREQ).contains(&frequency) {
                    let magnitude = (result.re * result.re + result.im * result.im).sqrt();
                    let amplitude = magnitude / MAGNITUDE_ZERO_DB;
                    let db = 20.0 * (amplitude.max(f32::EPSILON)).log10();
                    let normalized = ((db + 60.0f32) / 60.0f32).clamp(0.0f32, 1.0f32);

                    self.display_buffer.push(normalized);
                }
            }
        }
    }

    pub fn view(&self) -> Element<crate::Message> {
        canvas(self).width(Length::Fill).height(Length::Fill).into()
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Buffer(Arc<Vec<f32>>),
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

        for (bin_index, amplitude) in self.display_buffer.iter().enumerate() {
            let bin_height = amplitude * frame.height();
            let bin_left = bin_index as f32 * bin_width;
            let bin_top = frame.height() - bin_height;

            frame.fill_rectangle(
                Point::new(bin_left, bin_top),
                Size::new(bin_width, bin_height),
                ui::main_color(theme),
            );
        }

        let path = Path::line(Point::ORIGIN, Point::new(frame.width(), 0.0));
        let stroke = ui::separation_line_stroke(theme);

        frame.stroke(&path, stroke);

        vec![frame.into_geometry()]
    }
}
