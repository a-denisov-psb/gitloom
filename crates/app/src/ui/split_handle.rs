//! Grey horizontal bar between the file lists and the commit message of
//! the staging area at the bottom (finding #44).
//!
//! A press starts a drag, every cursor movement until the release reports
//! the vertical distance from the press (also outside the bar). The cursor
//! is ↕ over the bar and during the drag; the grip in its middle gets
//! brighter while hovered or dragged, like the pane splitters.

use iced::advanced::widget::{tree, Tree};
use iced::advanced::{layout, mouse, renderer, Clipboard, Layout, Shell, Widget};
use iced::{Background, Border, Element, Event, Length, Rectangle, Renderer, Size, Theme};

/// Height of the bar (its grab area).
pub const HEIGHT: f32 = 10.0;
/// Size of the grip drawn in the middle.
const GRIP: Size = Size::new(36.0, 4.0);

pub struct SplitHandle<'a, Message> {
    on_start: Message,
    on_drag: Box<dyn Fn(f32) -> Message + 'a>,
    on_end: Message,
}

/// A bar; see the module documentation for the messages.
pub fn split_handle<'a, Message: Clone + 'a>(
    on_start: Message,
    on_drag: impl Fn(f32) -> Message + 'a,
    on_end: Message,
) -> Element<'a, Message> {
    Element::new(SplitHandle {
        on_start,
        on_drag: Box::new(on_drag),
        on_end,
    })
}

#[derive(Default)]
struct State {
    /// Cursor y at the press while dragging.
    pressed_y: Option<f32>,
}

impl<Message: Clone> Widget<Message, Theme, Renderer> for SplitHandle<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(HEIGHT))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.resolve(Length::Fill, HEIGHT, Size::ZERO))
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
                shell.capture_event();
                state.pressed_y = Some(position.y);
                shell.publish(self.on_start.clone());
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if let Some(start) = state.pressed_y {
                    shell.publish((self.on_drag)(position.y - start));
                } else if cursor.is_over(layout.bounds()) {
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if state.pressed_y.is_some() =>
            {
                state.pressed_y = None;
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
        let dragging = tree.state.downcast_ref::<State>().pressed_y.is_some();
        if dragging || cursor.is_over(layout.bounds()) {
            mouse::Interaction::ResizingVertically
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        use iced::advanced::Renderer as _;
        let palette = crate::theme::palette(theme);
        let bounds = layout.bounds();
        let active =
            tree.state.downcast_ref::<State>().pressed_y.is_some() || cursor.is_over(bounds);
        let color = if active {
            palette.text_muted
        } else {
            palette.border
        };
        let grip = Rectangle {
            x: bounds.center_x() - GRIP.width / 2.0,
            y: bounds.center_y() - GRIP.height / 2.0,
            width: GRIP.width,
            height: GRIP.height,
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: grip,
                border: Border::default().rounded(GRIP.height / 2.0),
                ..renderer::Quad::default()
            },
            Background::Color(color),
        );
    }
}
