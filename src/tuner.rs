use std::sync::Arc;

use iced::{
    mouse,
    widget::{
        canvas,
        canvas::{Frame, Path, Text},
    },
    Element, Length, Point, Renderer, Theme,
};

use crate::{fft_processor::FftProcessor, ui};

const MIN_FREQ: f32 = 20.0;
const MAX_FREQ: f32 = 10000.0;

#[derive(Debug, Clone)]
pub enum Message {
    Buffer(Arc<Vec<f32>>),
    SampleRateChanged(usize),
    SampleSelectionChanged,
}

pub struct Tuner {
    display: String,
    sample_rate: usize,
    processor: FftProcessor<16384>,
}

impl Tuner {
    pub fn new() -> Self {
        Self {
            display: String::new(),
            sample_rate: 0,
            processor: FftProcessor::new(),
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Buffer(buffer) => {
                if buffer.is_empty() {
                    self.display.clear();
                    return;
                }

                self.process_buffer(buffer);
            }
            Message::SampleRateChanged(sample_rate) => {
                self.sample_rate = sample_rate;
            }
            Message::SampleSelectionChanged => {
                self.processor.reset();
            }
        }
    }

    fn process_buffer(&mut self, buffer: Arc<Vec<f32>>) {
        let fft_size = self.processor.fft_size();
        let bin_resolution = self.sample_rate as f32 / fft_size as f32;

        let average = compute_signal_power(&buffer);

        if average >= 1e-6 {
            if let Some(results) = self.processor.process(&buffer) {
                let half_fft_size = fft_size / 2;
                let mut magnitude_spec = Vec::with_capacity(half_fft_size);

                for (index, result) in results.take(half_fft_size).enumerate() {
                    let frequency = bin_resolution * index as f32;

                    if (MIN_FREQ..=MAX_FREQ).contains(&frequency) {
                        magnitude_spec.push((result.re * result.re + result.im * result.im).sqrt());
                    } else {
                        magnitude_spec.push(0f32);
                    }
                }

                // Harmonic Product Spectrum (https://www.chciken.com/digital/signal/processing/2020/05/13/guitar-tuner.html#dft)
                const NUM_HPS: usize = 5;

                let mag_spec_ipol = magnitude_spec.clone();

                for i in 0..NUM_HPS {
                    let new_len = (magnitude_spec.len() as f64 / (i + 1) as f64).ceil() as usize;
                    let tmp_hps_spec: Vec<f32> = magnitude_spec[..new_len]
                        .iter()
                        .zip(mag_spec_ipol.iter().step_by(i + 1))
                        .map(|(a, b)| a * b)
                        .collect();

                    if tmp_hps_spec.iter().all(|&x| x == 0.0) {
                        break;
                    }

                    magnitude_spec = tmp_hps_spec;
                }

                let mut max_bin_index = None;
                let mut max_mag = 0f32;

                for (index, magnitude) in magnitude_spec.iter().enumerate() {
                    if magnitude > &max_mag {
                        max_mag = *magnitude;
                        max_bin_index = Some(index);
                    }
                }

                self.display = max_bin_index
                    .map(|max_bin_index| {
                        let frequency = max_bin_index as f32 * bin_resolution;
                        let midi = frequency_to_midi(frequency);
                        let note = midi_to_note(midi);

                        note.to_string()
                    })
                    .unwrap_or_default()
            }
        }
    }

    pub fn view(&self) -> Element<crate::Message> {
        canvas(self).width(Length::Fill).height(Length::Fill).into()
    }
}

fn compute_signal_power(samples: &[f32]) -> f32 {
    let sum_of_squares: f32 = samples.iter().map(|&x| x * x).sum();

    sum_of_squares / samples.len() as f32
}

fn frequency_to_midi(fm: f32) -> usize {
    (12.0 * (fm / 440.0).log2() + 69.0).round() as usize
}

fn midi_to_note(midi: usize) -> &'static str {
    const NOTES: &[&str] = &[
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];

    NOTES[midi % 12]
}

impl canvas::Program<crate::Message> for Tuner {
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

        let line_path = Path::new(|p| {
            p.move_to(Point::new(bounds.width, 0.0));
            p.line_to(Point::new(bounds.width, bounds.size().height));

            p.move_to(Point::ORIGIN);
            p.line_to(Point::new(bounds.width, 0.0));
        });
        let stroke = ui::separation_line_stroke(theme);

        frame.stroke(&line_path, stroke);
        frame.fill_text(Text {
            content: self.display.clone(),
            position: Point::new(bounds.width / 2.0, bounds.height / 2.0),
            color: theme.palette().text,
            size: 20u32.into(),
            align_x: iced::alignment::Horizontal::Center,
            align_y: iced::alignment::Vertical::Center,
            ..Default::default()
        });
        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::frequency_to_midi;

    #[rstest]
    #[case(0, "C")]
    #[case(69, "A")]
    fn test_midi_to_note(#[case] input: usize, #[case] expected: &str) {
        use super::midi_to_note;

        let result = midi_to_note(input);

        assert_eq!(result, expected);
    }

    #[rstest]
    #[case(440f32, 69)]
    #[case(261.6f32, 60)]
    fn test_frequency_to_midi(#[case] input: f32, #[case] expected: usize) {
        let result = frequency_to_midi(input);

        assert_eq!(result, expected)
    }
}
