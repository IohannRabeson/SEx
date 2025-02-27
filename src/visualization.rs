use std::sync::Arc;

use iced::Task;
use itertools::Itertools;
use rodio::ChannelCount;

use crate::{scope, spectrum, tuner, vectorscope, vu_meter};

pub struct Visualization {}

#[derive(Debug, Clone)]
pub enum Message {
    AudioBuffer(ChannelCount, Vec<f32>),
    SampleRateChanged(usize),
}

impl Visualization {
    pub fn new() -> Self {
        Self {}
    }

    pub fn update(&mut self, message: Message) -> Task<crate::Message> {
        match message {
            Message::AudioBuffer(channels, samples) => {
                let rms = Self::compute_rms(channels, &samples);
                let points = Self::vectorscope(channels, &samples);
                let mono = Arc::new(Self::mono(channels, &samples));

                Task::batch([
                    Task::done(crate::Message::VuMeter(vu_meter::Message::Rms(rms))),
                    Task::done(crate::Message::Vectorscope(vectorscope::Message::Points(
                        points,
                    ))),
                    Task::done(crate::Message::Scope(scope::Message::Buffer(mono.clone()))),
                    Task::done(crate::Message::Spectrum(spectrum::Message::Buffer(
                        mono.clone(),
                    ))),
                    Task::done(crate::Message::Tuner(tuner::Message::Buffer(mono))),
                ])
            }
            Message::SampleRateChanged(sample_rate) => Task::batch([
                Task::done(crate::Message::Spectrum(
                    spectrum::Message::SampleRateChanged(sample_rate),
                )),
                Task::done(crate::Message::Tuner(tuner::Message::SampleRateChanged(
                    sample_rate,
                ))),
            ]),
        }
    }

    fn compute_rms(channels: ChannelCount, buffer: &[f32]) -> Vec<f32> {
        let channels = channels as usize;

        if buffer.is_empty() || channels == 0 {
            return Vec::new();
        }

        let mut rms_per_channels = vec![0f32; channels];

        for (i, sample) in buffer.iter().enumerate() {
            rms_per_channels[i % channels] += sample * sample;
        }

        let frames_count = (buffer.len() / channels) as f32;

        for rms in rms_per_channels.iter_mut().take(channels) {
            *rms /= frames_count;
            *rms = rms.sqrt();
        }

        rms_per_channels
    }

    fn vectorscope(channels: ChannelCount, samples: &[f32]) -> Vec<(f32, f32)> {
        if channels == 0 || samples.is_empty() {
            return Vec::new();
        }

        let channels = channels as usize;
        let mut result = Vec::with_capacity(samples.len() / channels);

        match channels {
            1 => {
                for sample in samples {
                    result.push((*sample, *sample));
                }
            }
            2 => {
                for i in (0..samples.len()).step_by(2) {
                    let left = samples[i];
                    let right = samples[i + 1];

                    result.push((left, right));
                }
            }
            _ => (),
        }

        result
    }

    fn mono(channels: u16, samples: &[f32]) -> Vec<f32> {
        if samples.is_empty() {
            return Vec::new();
        }
        if channels == 1 {
            return samples.to_vec();
        }

        let channels = channels as usize;
        let frame_count = samples.len() / channels;
        let mut result = vec![0f32; frame_count];

        for (i, chunk) in (&samples.iter().chunks(channels)).into_iter().enumerate() {
            let average = chunk.sum::<f32>() / channels as f32;

            result[i] = average;
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::Visualization;

    #[rstest]
    #[case(2, &[], &[])]
    #[case(1, &[0.5, 0.6, 0.7], &[0.605_530_1])]
    #[case(2, &[0.5, 0.6, 0.7, 0.8], &[0.608_276_25, 0.707_106_77])]
    fn test_compute_rms(#[case] channels: u16, #[case] buffer: &[f32], #[case] rms: &[f32]) {
        let result = Visualization::compute_rms(channels, buffer);

        assert_eq!(result.len(), rms.len());

        for (result, expected) in result.iter().zip(rms.iter()) {
            assert!((result - expected).abs() < 0.00000001);
        }
    }
}
