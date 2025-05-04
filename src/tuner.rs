use std::sync::Arc;

use iced::{
    mouse,
    widget::{
        canvas,
        canvas::{Frame, Path, Text},
    },
    Element, Length, Point, Renderer, Theme,
};
use pitch_detection::detector::{yin::YINDetector, PitchDetector};

use crate::ui;

#[derive(Debug, Clone)]
pub enum Message {
    Buffer(Arc<Vec<f32>>),
    SampleRateChanged(usize),
    SampleSelectionChanged,
}

pub struct Tuner {
    display: String,
    sample_rate: usize,
    pitch_detector: YINDetector<f32>,
    buffer: Vec<f32>,
}

const WINDOW: usize = 1024 * 8;
const WINDOW_PADDING: usize = WINDOW / 2;

impl Tuner {
    pub fn new() -> Self {
        Self {
            display: String::new(),
            sample_rate: 0,
            pitch_detector: YINDetector::new(WINDOW, WINDOW_PADDING),
            buffer: Vec::with_capacity(WINDOW),
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Buffer(new_buffer) => {
                if new_buffer.is_empty() {
                    self.display.clear();
                    return;
                }

                self.process_buffer(new_buffer);
            }
            Message::SampleRateChanged(sample_rate) => {
                self.sample_rate = sample_rate;
            }
            Message::SampleSelectionChanged => {
                self.buffer.clear();
            }
        }
    }

    fn process_buffer(&mut self, new_buffer: Arc<Vec<f32>>) {
        // self.buffer does not grow because WINDOW is bigger than new_buffer.len().
        assert!(WINDOW >= new_buffer.len());

        self.buffer.extend(new_buffer.iter());

        if self.buffer.len() < WINDOW {
            return;
        }

        let pitch =
            self.pitch_detector
                .get_pitch(&self.buffer[0..WINDOW], self.sample_rate, 10.0, 0.1);

        self.buffer.drain(0..WINDOW);

        self.display = pitch
            .map(|pitch| display_frequency_as_midi_note(pitch.frequency))
            .unwrap_or_default()
            .to_owned();
    }

    pub fn view(&self) -> Element<crate::Message> {
        canvas(self).width(Length::Fill).height(Length::Fill).into()
    }
}

fn display_frequency_as_midi_note(frequency: f32) -> &'static str {
    midi_to_note(frequency_to_midi(frequency))
}

fn frequency_to_midi(frequency: f32) -> usize {
    (12.0 * (frequency / 440.0).log2() + 69.0).round() as usize
}

fn midi_to_note(midi: usize) -> &'static str {
    const NOTES: &[&str] = &[
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];

    NOTES[midi % NOTES.len()]
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
        const TEXT_PADDING: f32 = 23.0;

        let mut frame = Frame::new(renderer, bounds.size());

        let line_path = Path::new(|p| {
            p.move_to(Point::new(bounds.width, 0.0));
            p.line_to(Point::new(bounds.width, bounds.size().height));

            p.move_to(Point::ORIGIN);
            p.line_to(Point::new(bounds.width, 0.0));
        });
        let stroke = ui::separation_line_stroke(theme);

        frame.stroke(&line_path, stroke);
        
        let min_size = bounds.width.min(bounds.height);

        if min_size > TEXT_PADDING {

       
        frame.fill_text(Text {
            content: self.display.clone(),
            position: Point::new(bounds.width / 2.0, bounds.height / 2.0),
            color: ui::main_color(theme),
            size: (bounds.width.min(bounds.height) - TEXT_PADDING).into(),
            align_x: iced::alignment::Horizontal::Center,
            align_y: iced::alignment::Vertical::Center,
            ..Default::default()
        });

    }
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
