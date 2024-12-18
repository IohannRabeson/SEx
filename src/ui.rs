use iced::{
    alignment::Vertical,
    widget::{container, image, MouseArea, Row},
    Element, Padding, Theme,
};

use crate::Message;

pub fn file_entry<'a>(
    text: impl ToString,
    select_message: Message,
    icon: Option<image::Handle>,
    selected: bool,
) -> Element<'a, Message> {
    let mut row = Row::new();

    const FONT_SIZE: u16 = 14;
    const ICON_SIZE: u16 = 16;

    row = row.push_maybe(icon.map(|handle| {
        container(
            image(handle)
                .filter_method(image::FilterMethod::Nearest)
                .width(ICON_SIZE)
                .height(ICON_SIZE),
        )
        .padding(Padding::from([0, 4]))
    }));
    row = row.push(iced::widget::text(text.to_string()).size(FONT_SIZE));
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
