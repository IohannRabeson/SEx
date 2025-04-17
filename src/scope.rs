use crate::ui;
use iced::widget::canvas;
use iced::{
    mouse,
    widget::canvas::{Frame, Geometry, Path, Stroke},
    Element, Length, Point, Renderer, Theme,
};

pub struct Scope {
    buffer: Vec<f32>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Buffer(Vec<f32>),
}

impl Scope {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Buffer(mut buffer) => {
                self.buffer.clear();
                self.buffer.append(&mut buffer);
            }
        }
    }

    pub fn view(&self) -> Element<crate::Message> {
        canvas(self).width(Length::Fill).height(Length::Fill).into()
    }
}

impl canvas::Program<crate::Message> for Scope {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        let line_path = Path::new(|p| {
            p.move_to(Point::ORIGIN);
            p.line_to(Point::new(0.0, bounds.size().height));

            p.move_to(Point::new(bounds.width, 0.0));
            p.line_to(Point::new(bounds.width, bounds.size().height));

            p.move_to(Point::ORIGIN);
            p.line_to(Point::new(bounds.width, 0.0));
        });
        let stroke = ui::separation_line_stroke(theme);

        frame.stroke(&line_path, stroke);

        if !self.buffer.is_empty() {
            let sample_size = bounds.width / self.buffer.len() as f32;
            let path = Path::new(|p| {
                let hy = frame.height() / 2.0;

                for i in 1..self.buffer.len() {
                    let y0 = hy + (self.buffer[i - 1] * hy);
                    let y1 = hy + (self.buffer[i] * hy);
                    let x0 = (i - 1) as f32 * sample_size;
                    let x1 = x0 + sample_size;

                    p.move_to(Point::new(x0, y0));
                    p.line_to(Point::new(x1, y1));
                }
            });
            let stroke = Stroke::default().with_color(theme.palette().primary);

            frame.stroke(&path, stroke);
        }

        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::{generate_sine, simulator},
        SEx,
    };
    use iced_test::Error;

    #[test]
    fn test_scope() -> Result<(), Error> {
        let (mut app, _task) = SEx::new();

        let buffer = generate_sine(1000).collect();
        let _ = app.update(crate::Message::Scope(crate::scope::Message::Buffer(buffer)));

        let mut ui = simulator(&app);
        let snapshot = ui.snapshot(&iced::Theme::CatppuccinFrappe)?;

        assert!(snapshot.matches_hash("snapshots/test_scope")?);

        Ok(())
    }
}
