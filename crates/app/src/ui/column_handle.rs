//! Draggable border between two graph table header cells (M11-02).
//!
//! A press starts a drag, every cursor movement until the release reports
//! the horizontal distance from the press (also outside the handle), a
//! double click asks for the optimal width. The cursor is ↔ over the
//! handle and during the drag. The handle draws nothing; it lies over the
//! right edge of its header cell, so the cells keep their widths.

use iced::advanced::widget::{tree, Tree};
use iced::advanced::{layout, mouse, renderer, Clipboard, Layout, Shell, Widget};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme};

/// Width of the grab area.
pub const HANDLE_WIDTH: f32 = 7.0;

pub struct ColumnHandle<'a, Message> {
    on_start: Message,
    on_drag: Box<dyn Fn(f32) -> Message + 'a>,
    on_end: Message,
    on_double_click: Message,
}

/// A handle; see the module documentation for the messages.
pub fn column_handle<'a, Message: Clone + 'a>(
    on_start: Message,
    on_drag: impl Fn(f32) -> Message + 'a,
    on_end: Message,
    on_double_click: Message,
) -> Element<'a, Message> {
    Element::new(ColumnHandle {
        on_start,
        on_drag: Box::new(on_drag),
        on_end,
        on_double_click,
    })
}

#[derive(Default)]
struct State {
    /// Cursor x at the press while dragging.
    pressed_x: Option<f32>,
    previous: Option<mouse::Click>,
}

impl<Message: Clone> Widget<Message, Theme, Renderer> for ColumnHandle<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(HANDLE_WIDTH), Length::Fill)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.resolve(HANDLE_WIDTH, Length::Fill, Size::ZERO))
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(position) = cursor.position_over(layout.bounds()) else {
                    return;
                };
                let click = mouse::Click::new(position, mouse::Button::Left, state.previous);
                state.previous = Some(click);
                shell.capture_event();
                if click.kind() == mouse::click::Kind::Double {
                    state.pressed_x = None;
                    shell.publish(self.on_double_click.clone());
                } else {
                    state.pressed_x = Some(position.x);
                    shell.publish(self.on_start.clone());
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) if state.pressed_x.is_some() => {
                let start = state.pressed_x.unwrap_or(position.x);
                shell.publish((self.on_drag)(position.x - start));
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if state.pressed_x.is_some() =>
            {
                state.pressed_x = None;
                shell.publish(self.on_end.clone());
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let dragging = tree.state.downcast_ref::<State>().pressed_x.is_some();
        if dragging || cursor.is_over(layout.bounds()) {
            mouse::Interaction::ResizingHorizontally
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }
}
