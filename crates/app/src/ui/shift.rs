//! Moves its content left by an offset and clips it to its own bounds
//! (M11-16). Unlike `pin`, the content is laid out without a width limit,
//! so a row of fixed-width cells wider than the viewport keeps its widths.

use iced::advanced::widget::{tree, Operation, Tree};
use iced::advanced::{layout, mouse, overlay, renderer, Clipboard, Layout, Shell, Widget};
use iced::{Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector};

struct Shift<'a, Message> {
    content: Element<'a, Message>,
    offset: f32,
}

/// `content` moved left by `offset` pixels, clipped to the available width.
pub fn shift<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    offset: f32,
) -> Element<'a, Message> {
    Element::new(Shift {
        content: content.into(),
        offset: offset.max(0.0),
    })
}

impl<Message> Widget<Message, Theme, Renderer> for Shift<'_, Message> {
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
        Size::new(Length::Fill, self.content.as_widget().size().height)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let max = limits.max();
        let unbounded = layout::Limits::new(Size::ZERO, Size::new(f32::INFINITY, max.height));
        let node = self
            .content
            .as_widget_mut()
            .layout(tree, renderer, &unbounded)
            .move_to(Point::new(-self.offset, 0.0));
        let size = limits.resolve(
            Length::Fill,
            Length::Shrink,
            Size::new(max.width, node.size().height),
        );
        layout::Node::with_children(size, vec![node])
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        if let Some(child) = layout.children().next() {
            self.content
                .as_widget_mut()
                .operate(tree, child, renderer, operation);
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
        let Some(child) = layout.children().next() else {
            return;
        };
        // Only the visible part reacts to the pointer.
        let cursor = match cursor.position() {
            Some(position) if !layout.bounds().contains(position) => mouse::Cursor::Unavailable,
            _ => cursor,
        };
        self.content.as_widget_mut().update(
            tree,
            event,
            child,
            cursor,
            renderer,
            clipboard,
            shell,
            &layout.bounds().intersection(viewport).unwrap_or_default(),
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
        let Some(child) = layout.children().next() else {
            return mouse::Interaction::default();
        };
        if !cursor.is_over(layout.bounds()) {
            return mouse::Interaction::default();
        }
        self.content
            .as_widget()
            .mouse_interaction(tree, child, cursor, viewport, renderer)
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
        use iced::advanced::Renderer as _;
        let bounds = layout.bounds();
        let Some(child) = layout.children().next() else {
            return;
        };
        let Some(clip) = bounds.intersection(viewport) else {
            return;
        };
        renderer.with_layer(clip, |renderer| {
            self.content
                .as_widget()
                .draw(tree, renderer, theme, style, child, cursor, &clip);
        });
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let child = layout.children().next()?;
        self.content
            .as_widget_mut()
            .overlay(tree, child, renderer, viewport, translation)
    }
}
