use iced::{
    alignment::Vertical,
    widget::{container, image, text::Wrapping, MouseArea, Row},
    Color, Element, Padding, Theme,
};

use crate::Message;

pub(crate) const ICON_SIZE: u32 = 20;
pub(crate) const AXIS_COLOR: Color = Color::from_rgb(0.45, 0.45, 0.45);

pub fn file_entry<'a>(
    text: impl ToString,
    select_message: Message,
    icon: Option<image::Handle>,
    selected: bool,
) -> Element<'a, Message> {
    const FONT_SIZE: u32 = 14;

    let mut row = Row::new();

    row = row.push_maybe(icon.map(|handle| {
        container(
            image(handle)
                .filter_method(image::FilterMethod::Nearest)
                .width(ICON_SIZE)
                .height(ICON_SIZE),
        )
        .padding(Padding::from([0, 4]))
    }));
    row = row.push(
        iced::widget::text(text.to_string())
            .size(FONT_SIZE)
            .wrapping(Wrapping::None),
    );
    row = row.align_y(Vertical::Center);

    let mut selectable_part = container(row);

    if selected {
        selectable_part = selectable_part.style(selected_style);
    }

    MouseArea::new(selectable_part)
        .on_press(select_message)
        .into()
}

fn selected_style(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(theme.palette().primary)),
        ..Default::default()
    }
}
