use iced::{
    mouse,
    widget::{
        canvas::{self, Frame, Path, Text},
        Canvas,
    },
    Element, Length, Point, Renderer, Theme,
};

use crate::{fft_processor::FftProcessor, ui};

const MIN_FREQ: f32 = 20.0;
const MAX_FREQ: f32 = 10000.0;

#[derive(Debug, Clone)]
pub enum Message {
    Buffer(Vec<f32>),
    SampleRateChanged(usize),
}

pub struct Tuner {
    display: String,
    sample_rate: usize,
    processor: FftProcessor<8192>,
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
        }
    }

    fn process_buffer(&mut self, buffer: Vec<f32>) {
        let fft_size = self.processor.fft_size();
        let bin_resolution = self.sample_rate as f32 / fft_size as f32;

        let average = average(&buffer);

        if average >= 0.001 {
            if let Some(results) = self.processor.process(&buffer) {
                let mut max_magnitude = 0f32;
                let mut max_bin_index = None;

                for (index, result) in results.take(fft_size / 2).enumerate() {
                    let frequency = bin_resolution * index as f32;

                    if (MIN_FREQ..=MAX_FREQ).contains(&frequency) {
                        let magnitude = (result.re * result.re + result.im * result.im).sqrt();

                        if magnitude > max_magnitude {
                            max_magnitude = magnitude;
                            max_bin_index = Some(index);
                        }
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
        Canvas::new(self)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        // Alternative display for debugging
        //container(text(&self.display)).center(Length::Fill).into()
    }
}

fn average(buffer: &[f32]) -> f32 {
    let sum = buffer.iter().sum::<f32>().abs();

    sum / buffer.len() as f32
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
            position: Point::new(0.0, (frame.height() - 20.0) / 2.0), // TODO: implement something in iced to get the measured text
            color: theme.palette().text,
            size: 20u32.into(),
            ..Default::default()
        });
        vec![frame.into_geometry()]
    }
}
