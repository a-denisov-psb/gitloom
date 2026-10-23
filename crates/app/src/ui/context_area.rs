//! Wrapper widget that reports right (or left) clicks with the absolute cursor position.
//!
//! `mouse_area::on_right_press` carries no position and `on_move` would emit a
//! message for every mouse movement; context menus only need the position at
//! the moment of the right click.

use iced::advanced::widget::{tree, Operation, Tree};
use iced::advanced::{layout, mouse, overlay, renderer, Clipboard, Layout, Shell, Widget};
use iced::{Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector};

pub struct ContextArea<'a, Message> {
    content: Element<'a, Message>,
    on_press: Box<dyn Fn(Point) -> Message + 'a>,
    button: mouse::Button,
}

/// Wraps `content`; a right click on it emits `on_right_press(cursor position)`.
pub fn context_area<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    on_right_press: impl Fn(Point) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(ContextArea {
        content: content.into(),
        on_press: Box::new(on_right_press),
        button: mouse::Button::Right,
    })
}

/// Like [`context_area`] for the left button, e.g. to open a dropdown menu
/// at the cursor.
pub fn click_area<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    on_press: impl Fn(Point) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(ContextArea {
        content: content.into(),
        on_press: Box::new(on_press),
        button: mouse::Button::Left,
    })
}

impl<Message> Widget<Message, Theme, Renderer> for ContextArea<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        if shell.is_event_captured() {
            return;
        }
        if let Event::Mouse(mouse::Event::ButtonPressed(button)) = event {
            if *button == self.button && cursor.is_over(layout.bounds()) {
                if let Some(position) = cursor.position() {
                    shell.publish((self.on_press)(position));
                    shell.capture_event();
                }
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

/// Wraps `content` (e.g. a list button) and emits `on_double_click` on a
/// double click, while single clicks still reach the content. Unlike
/// `mouse_area::on_double_click`, presses are not captured.
pub fn double_click_area<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    on_double_click: Message,
) -> Element<'a, Message> {
    Element::new(DoubleClickArea {
        content: content.into(),
        on_double_click,
    })
}

struct DoubleClickArea<'a, Message> {
    content: Element<'a, Message>,
    on_double_click: Message,
}

#[derive(Default)]
struct ClickState {
    previous: Option<mouse::Click>,
}

impl<Message: Clone> Widget<Message, Theme, Renderer> for DoubleClickArea<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<ClickState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(ClickState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        // Seen before the content (a button) captures the press.
        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
            if let Some(position) = cursor.position_over(layout.bounds()) {
                let state = tree.state.downcast_mut::<ClickState>();
                let click = mouse::Click::new(position, mouse::Button::Left, state.previous);
                state.previous = Some(click);
                if click.kind() == mouse::click::Kind::Double {
                    shell.publish(self.on_double_click.clone());
                }
            }
        }
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
