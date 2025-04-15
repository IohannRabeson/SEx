use iced::{
    mouse,
    widget::{
        canvas::{self, Cache},
        Canvas,
    },
    Element, Length, Point, Rectangle, Renderer, Size, Theme,
};

use crate::ui;

#[derive(Debug, Clone)]
pub enum Message {
    /// RMS per channel
    Rms(Vec<f32>),
}

pub struct VuMeter {
    levels_per_channel: Vec<f32>,
    cache: Cache,
}

impl VuMeter {
    pub fn new() -> Self {
        Self {
            levels_per_channel: Vec::with_capacity(2),
            cache: Cache::new(),
        }
    }

    pub fn view(&self) -> Element<crate::Message> {
        Canvas::new(self)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Rms(rms_per_channel) => {
                if rms_per_channel.len() != self.levels_per_channel.len() {
                    self.levels_per_channel.resize(rms_per_channel.len(), 0f32);
                }

                for (rms, level) in rms_per_channel
                    .into_iter()
                    .zip(self.levels_per_channel.iter_mut())
                {
                    let db = 20.0 * rms.max(f32::EPSILON).log10();

                    *level = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
                }
            }
        }

        self.cache.clear();
    }
}

impl canvas::Program<crate::Message> for VuMeter {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            let width = frame.width() / self.levels_per_channel.len() as f32;

            for (i, level) in self.levels_per_channel.iter().enumerate() {
                let height = level * frame.height();
                let y = frame.height() - height;

                frame.fill_rectangle(
                    Point::new(i as f32 * width, y),
                    Size::new(width, height),
                    ui::main_color(theme),
                );
            }
        });

        vec![geometry]
    }
}

#[cfg(test)]
mod tests {
    use crate::{tests::simulator, SEx};

    #[test]
    fn test_vu_meter_mono() -> Result<(), iced_test::Error> {
        let (mut app, _) = SEx::new();

        let _ = app.update(crate::Message::VuMeter(crate::vu_meter::Message::Rms(vec![1.0])));
        let mut ui = simulator(&app);

        let snapshot = ui.snapshot(&iced::Theme::CatppuccinFrappe)?;

        assert!(snapshot.matches_hash("snapshots/test_vu_meter_mono")?);

        Ok(())
    }

    #[test]
    fn test_vu_meter_stereo() -> Result<(), iced_test::Error> {
        let (mut app, _) = SEx::new();

        let _ = app.update(crate::Message::VuMeter(crate::vu_meter::Message::Rms(vec![0.5, 0.9])));
        let mut ui = simulator(&app);

        let snapshot = ui.snapshot(&iced::Theme::CatppuccinFrappe)?;

        assert!(snapshot.matches_hash("snapshots/test_vu_meter_stereo")?);

        Ok(())
    }

    #[test]
    fn test_vu_meter_more_channels() -> Result<(), iced_test::Error> {
        let (mut app, _) = SEx::new();

        let _ = app.update(crate::Message::VuMeter(crate::vu_meter::Message::Rms(vec![0.5, 0.6, 0.7])));
        let mut ui = simulator(&app);

        let snapshot = ui.snapshot(&iced::Theme::CatppuccinFrappe)?;

        assert!(snapshot.matches_hash("snapshots/test_vu_meter_more_channels")?);

        Ok(())
    }
}