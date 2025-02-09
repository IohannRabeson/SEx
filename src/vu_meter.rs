use iced::{
    mouse,
    widget::{
        canvas::{self, Cache},
        Canvas,
    },
    Element, Length, Point, Rectangle, Renderer, Size, Theme,
};

use crate::Message;

#[derive(Debug, Clone)]
pub enum VuMeterMessage {
    Rms(f32),
}

pub struct VuMeter {
    level: f32,
    cache: Cache,
}

impl VuMeter {
    pub fn new() -> Self {
        Self {
            level: 0f32,
            cache: Cache::new(),
        }
    }

    pub fn view(&self) -> Element<Message> {
        Canvas::new(self)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn update(&mut self, message: VuMeterMessage) {
        match message {
            VuMeterMessage::Rms(rms) => {
                let db = 20.0 * rms.max(f32::EPSILON).log10();

                self.level = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
            }
        }

        self.cache.clear();
    }
}

impl canvas::Program<Message> for VuMeter {
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
            let height = self.level * frame.height();
            let y = frame.height() - height;

            frame.fill_rectangle(
                Point::new(0.0, y),
                Size::new(frame.width(), height),
                theme.palette().primary,
            );
        });

        vec![geometry]
    }
}
