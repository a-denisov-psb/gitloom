//! Keeps presses in the grab area of a pane divider away from the pane's
//! content (finding #48).
//!
//! iced's pane grid hands every event to the panes first and only then
//! checks whether the press hit a divider; the grab area reaches
//! [`crate::layout::DIVIDER_REACH`] pixels into the panes on both sides.
//! A scrollbar at the edge of a pane got the press as well, so dragging
//! the divider also dragged the scrollbar. This wrapper drops a left press
//! inside the grab area before it reaches the content; the pane grid still
//! gets it and starts the resize. Everything else passes through, and the
//! rest of a scrollbar next to a divider works as before.
//!
//! The wrapper reports its bounds under its id like a container does, so
//! tests can find the panes.

use iced::advanced::widget::{tree, Operation, Tree};
use iced::advanced::{layout, mouse, overlay, renderer, Clipboard, Layout, Shell, Widget};
use iced::{touch, Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector};

use crate::panes::DividerEdges;

struct DividerGuard<'a, Message> {
    content: Element<'a, Message>,
    edges: DividerEdges,
    id: iced::widget::Id,
}

/// `content` of a pane whose `edges` border a divider; `id` names the pane
/// for widget operations.
pub fn divider_guard<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    edges: DividerEdges,
    id: iced::widget::Id,
) -> Element<'a, Message> {
    Element::new(DividerGuard {
        content: content.into(),
        edges,
        id,
    })
}

/// Whether `point` lies in the part of a pane `bounds` large that belongs
/// to the grab area of a divider at one of its `edges`.
pub fn in_grab_area(bounds: Rectangle, edges: DividerEdges, point: Point) -> bool {
    let reach = crate::layout::DIVIDER_REACH;
    bounds.contains(point)
        && ((edges.left && point.x < bounds.x + reach)
            || (edges.right && point.x >= bounds.x + bounds.width - reach)
            || (edges.top && point.y < bounds.y + reach)
            || (edges.bottom && point.y >= bounds.y + bounds.height - reach))
}

impl<Message> DividerGuard<'_, Message> {
    /// Whether `event` is a press the content must not see.
    fn swallows(&self, event: &Event, bounds: Rectangle, cursor: mouse::Cursor) -> bool {
        let position = match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => cursor.position(),
            Event::Touch(touch::Event::FingerPressed { position, .. }) => Some(*position),
            _ => None,
        };
        position.is_some_and(|p| in_grab_area(bounds, self.edges, p))
    }
}

impl<Message> Widget<Message, Theme, Renderer> for DividerGuard<'_, Message> {
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
        operation.container(Some(&self.id), layout.bounds());
        operation.traverse(&mut |operation| {
            self.content
                .as_widget_mut()
                .operate(tree, layout, renderer, operation);
        });
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
        if self.swallows(event, layout.bounds(), cursor) {
            return;
        }
        self.content.as_widget_mut().update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
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
        // A scrollbar under the divider's grab area does not light up.
        let cursor = match cursor.position() {
            Some(p) if in_grab_area(layout.bounds(), self.edges, p) => mouse::Cursor::Unavailable,
            _ => cursor,
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
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grab_area_reaches_three_pixels_into_the_bordering_edges() {
        let bounds = Rectangle::new(Point::new(100.0, 50.0), Size::new(200.0, 300.0));
        let right = DividerEdges {
            right: true,
            ..DividerEdges::default()
        };
        assert!(in_grab_area(bounds, right, Point::new(299.5, 200.0)));
        assert!(in_grab_area(bounds, right, Point::new(297.0, 200.0)));
        assert!(!in_grab_area(bounds, right, Point::new(296.9, 200.0)));
        assert!(!in_grab_area(bounds, right, Point::new(300.0, 200.0)));
        assert!(!in_grab_area(bounds, right, Point::new(100.5, 200.0)));
        let left_bottom = DividerEdges {
            left: true,
            bottom: true,
            ..DividerEdges::default()
        };
        assert!(in_grab_area(bounds, left_bottom, Point::new(102.9, 60.0)));
        assert!(!in_grab_area(bounds, left_bottom, Point::new(103.0, 60.0)));
        assert!(in_grab_area(bounds, left_bottom, Point::new(200.0, 347.0)));
        assert!(!in_grab_area(bounds, left_bottom, Point::new(200.0, 346.9)));
        assert!(!in_grab_area(
            bounds,
            DividerEdges::default(),
            Point::new(100.0, 50.0)
        ));
    }
}
