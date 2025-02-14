use iced::Task;
use rodio::ChannelCount;

use crate::{vu_meter::VuMeterMessage, Message};

pub struct Visualization {}

#[derive(Debug, Clone)]
pub enum VisualizationMessage {
    AudioBuffer(ChannelCount, Vec<f32>),
}

impl Visualization {
    pub fn new() -> Self {
        Self {}
    }

    pub fn update(&mut self, message: VisualizationMessage) -> Task<Message> {
        match message {
            VisualizationMessage::AudioBuffer(channels, samples) => {
                let rms = Self::compute_rms(channels, &samples);

                Task::done(Message::VuMeter(VuMeterMessage::Rms(rms)))
            }
        }
    }

    fn compute_rms(channels: u16, buffer: &[f32]) -> Vec<f32> {
        let channels = channels as usize;

        if buffer.is_empty() || channels == 0 {
            return Vec::new();
        }

        let mut rms_per_channels = vec![0f32; channels];

        for (i, sample) in buffer.iter().enumerate() {
            rms_per_channels[i % channels] += sample * sample;
        }

        let frames_count = (buffer.len() / channels) as f32;

        for i in 0..channels {
            rms_per_channels[i] /= frames_count;
            rms_per_channels[i] = rms_per_channels[i].sqrt();
        }

        rms_per_channels
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::Visualization;

    #[rstest]
    #[case(2, &[], &[])]
    #[case(1, &[0.5, 0.6, 0.7], &[0.60553007081949833307])]
    #[case(2, &[0.5, 0.6, 0.7, 0.8], &[0.60827625302982196889, 0.70710678118654752440])]
    fn test_compute_rms(#[case] channels: u16, #[case] buffer: &[f32], #[case] rms: &[f32]) {
        let result = Visualization::compute_rms(channels, buffer);

        assert_eq!(result.len(), rms.len());

        for (result, expected) in result.iter().zip(rms.iter()) {
            assert!((result - expected).abs() < 0.00000001);
        }
    }
}
