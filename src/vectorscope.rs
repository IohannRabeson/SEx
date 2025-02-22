use iced::{
    mouse,
    widget::{
        canvas::{self, Fill, Frame, Path},
        Canvas,
    },
    Element, Point, Renderer, Theme,
};

use crate::ui;

pub struct Vectorscope {
    points: Vec<(f32, f32)>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Points(Vec<(f32, f32)>),
}

impl Vectorscope {
    pub fn new() -> Self {
        Self { points: Vec::new() }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Points(points) => {
                self.points = points;
            }
        }
    }

    pub fn view(&self) -> Element<crate::Message> {
        Canvas::new(self).into()
    }
}

impl canvas::Program<crate::Message> for Vectorscope {
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

        let center_x = bounds.width / 2.0;
        let center_y = bounds.height / 2.0;

        // Draw separating lines.
        let path = Path::new(|p| {
            p.move_to(Point::ORIGIN);
            p.line_to(Point::new(0.0, bounds.size().height));

            p.move_to(Point::new(bounds.width, 0.0));
            p.line_to(Point::new(bounds.width, bounds.size().height));
        });

        let stroke = ui::separation_line_stroke(&theme);

        frame.stroke(&path, stroke);

        // Draw scope.
        let scale = bounds.width.min(bounds.height) / 2.0;
        let fill = Fill::from(ui::main_color(theme));

        // Cumulating all the circles into a unique path leads to performance issue.
        for &(x, y) in &self.points {
            let pos = Point::new(center_x + x * scale, center_y - y * scale);
            let path = Path::circle(pos, 1.0);
            frame.fill(&path, fill);
        }

        vec![frame.into_geometry()]
    }
}
