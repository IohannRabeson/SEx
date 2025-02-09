use iced::Task;

use crate::{vu_meter::VuMeterMessage, Message};

pub struct Visualization {}

#[derive(Debug, Clone)]
pub enum VisualizationMessage {
    AudioBuffer(Vec<f32>),
}

impl Visualization {
    pub fn new() -> Self {
        Self {}
    }

    pub fn update(&mut self, message: VisualizationMessage) -> Task<Message> {
        match message {
            VisualizationMessage::AudioBuffer(samples) => {
                let rms = Self::compute_rms(&samples);

                return Task::done(Message::VuMeter(VuMeterMessage::Rms(rms)));
            }
        }
    }

    fn compute_rms(buffer: &[f32]) -> f32 {
        if buffer.is_empty() {
            return 0f32;
        }

        (buffer.iter().map(|sample| sample * sample).sum::<f32>() / buffer.len() as f32).sqrt()
    }
}
