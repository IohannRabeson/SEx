use iced::{
    alignment::Vertical,
    widget::{canvas::Stroke, container, svg, text::Wrapping, MouseArea, Row},
    Color, Element, Padding, Theme,
};

use crate::{ui, Message};

pub(crate) const ICON_SIZE: u32 = 18;

pub fn file_entry<'a>(
    text: impl ToString,
    select_message: Message,
    icon: Option<svg::Handle>,
    selected: bool,
) -> Element<'a, Message> {
    const FONT_SIZE: u32 = 14;

    let mut row = Row::new();

    row = row.push_maybe(icon.map(|handle| {
        container(
            svg(handle)
                .style(|theme: &Theme, _status| svg::Style {
                    color: Some(ui::main_color(theme)),
                })
                .width(ICON_SIZE)
                .height(ICON_SIZE),
        )
        .padding(Padding {
            top: 0.,
            right: 4.,
            bottom: 0.,
            left: 0.,
        })
    }));
    row = row.push(
        iced::widget::text(text.to_string())
            .size(FONT_SIZE)
            .wrapping(Wrapping::None),
    );
    row = row.align_y(Vertical::Center);

    let mut selectable_part = container(row).padding(Padding {
        top: 0.,
        right: 4.,
        bottom: 0.,
        left: 4.,
    });

    if selected {
        selectable_part = selectable_part.style(selected_style);
    }

    MouseArea::new(selectable_part)
        .on_press(select_message)
        .into()
}

fn selected_style(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(
            theme.extended_palette().secondary.weak.color,
        )),
        ..Default::default()
    }
}

pub fn separation_line_stroke(theme: &Theme) -> Stroke<'_> {
    Stroke::default()
        .with_color(theme.extended_palette().background.strong.color)
        // Choosing a width of 1 causes a bug on Windows only where horizontal lines are not displayed.
        .with_width(2.0)
}

pub fn main_color(theme: &Theme) -> Color {
    theme.extended_palette().primary.weak.color
}
