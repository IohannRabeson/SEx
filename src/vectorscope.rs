use iced::{mouse, widget::{canvas::{self, Fill, Frame, Path, Stroke}, Canvas}, Element, Point, Renderer, Theme};

pub struct Vectorscope {
    points: Vec<(f32, f32)>
}

#[derive(Debug, Clone)]
pub enum Message {
    Points(Vec<(f32, f32)>),
}

impl Vectorscope {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Points(points) => {
                self.points = points;
            },
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
        let scale = bounds.width.min(bounds.height) / 2.0; // Scale factor
        let fill = Fill::from(theme.palette().primary);

        for &(x, y) in &self.points {
            let pos = Point::new(center_x + x * scale, center_y - y * scale);
            let path = Path::circle(pos, 1.0);
            frame.fill(&path, fill);
        }
        
        vec![frame.into_geometry()]
    }
}