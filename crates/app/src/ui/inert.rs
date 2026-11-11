//! Wrapper that keeps its content visible but out of reach (M11-13): while
//! active, no events reach the content and widget operations (focus, Tab
//! order, scrolling) skip it, so focus stays inside a modal shown above.
//! The wrapper is always in the tree, so the content keeps its state.

use iced::advanced::widget::{tree, Operation, Tree};
use iced::advanced::{layout, mouse, overlay, renderer, Clipboard, Layout, Shell, Widget};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector};

struct Inert<'a, Message> {
    content: Element<'a, Message>,
    active: bool,
}

/// `content`, unreachable while `active`.
pub fn inert<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    active: bool,
) -> Element<'a, Message> {
    Element::new(Inert {
        content: content.into(),
        active,
    })
}

impl<Message> Widget<Message, Theme, Renderer> for Inert<'_, Message> {
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
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
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        if !self.active {
            self.content
                .as_widget_mut()
                .operate(tree, layout, renderer, operation);
        }
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
        // Redraw requests still reach it (animations, scroll state).
        let redraw = matches!(
            event,
            Event::Window(iced::window::Event::RedrawRequested(_))
        );
        if !self.active || redraw {
            self.content.as_widget_mut().update(
                tree, event, layout, cursor, renderer, clipboard, shell, viewport,
            );
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
        if self.active {
            return mouse::Interaction::default();
        }
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
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
        let cursor = if self.active {
            mouse::Cursor::Unavailable
        } else {
            cursor
        };
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        if self.active {
            return None;
        }
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}
