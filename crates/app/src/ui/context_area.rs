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

/// Translation from content to window coordinates, as iced passes it to
/// `overlay()` at the start of every update. Inside a scrolled `scrollable`
/// the cursor is in content coordinates; adding this gives the window
/// position for menus.
#[derive(Default)]
struct Translation(Vector);

impl<Message> Widget<Message, Theme, Renderer> for ContextArea<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Translation>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Translation::default())
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
                    let translation = tree.state.downcast_ref::<Translation>().0;
                    shell.publish((self.on_press)(position + translation));
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
        tree.state.downcast_mut::<Translation>().0 = translation;
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

/// Wraps a branch entry so it can be dragged onto another one (M4-11).
/// Never captures events, so clicks, double clicks and context menus of
/// the content keep working:
/// - a left press followed by 4 px of movement emits `on_drag_start`;
/// - while the left button is down, entering or leaving the entry emits
///   `on_enter` / `on_leave` (drop target highlight);
/// - releasing the left button over the entry emits `on_drop(position)`.
pub fn drag_area<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    events: DragEvents<Message>,
    on_drop: impl Fn(Point) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(DragArea {
        content: content.into(),
        events,
        on_drop: Box::new(on_drop),
    })
}

/// Messages of a [`drag_area`].
pub struct DragEvents<Message> {
    pub start: Message,
    pub enter: Message,
    pub leave: Message,
}

struct DragArea<'a, Message> {
    content: Element<'a, Message>,
    events: DragEvents<Message>,
    on_drop: Box<dyn Fn(Point) -> Message + 'a>,
}

#[derive(Default)]
struct DragState {
    /// Where the left button went down on this entry.
    pressed_at: Option<Point>,
    started: bool,
    /// The left button is down (pressed anywhere).
    button_down: bool,
    hovered: bool,
    /// See [`Translation`].
    translation: Vector,
}

/// Movement in pixels before a press becomes a drag.
const DRAG_THRESHOLD: f32 = 4.0;

impl<Message: Clone> Widget<Message, Theme, Renderer> for DragArea<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<DragState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(DragState::default())
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
        // Seen before the content can capture it; never captured here.
        let state = tree.state.downcast_mut::<DragState>();
        let bounds = layout.bounds();
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                state.button_down = true;
                state.pressed_at = cursor.position_over(bounds);
                state.started = false;
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if let (Some(pressed), false) = (state.pressed_at, state.started) {
                    if pressed.distance(*position) > DRAG_THRESHOLD {
                        state.started = true;
                        shell.publish(self.events.start.clone());
                    }
                }
                let over = cursor.is_over(bounds);
                if state.button_down && over != state.hovered {
                    shell.publish(if over {
                        self.events.enter.clone()
                    } else {
                        self.events.leave.clone()
                    });
                }
                state.hovered = over;
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.button_down = false;
                state.pressed_at = None;
                state.started = false;
                if let Some(position) = cursor.position_over(bounds) {
                    shell.publish((self.on_drop)(position + state.translation));
                }
            }
            _ => {}
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
        tree.state.downcast_mut::<DragState>().translation = translation;
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

/// Wraps the whole window: a left button release anywhere emits
/// `on_release` after the content handled it (ends a branch drag that was
/// not dropped on a target).
pub fn release_area<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    on_release: Message,
) -> Element<'a, Message> {
    Element::new(ReleaseArea {
        content: content.into(),
        on_release,
    })
}

struct ReleaseArea<'a, Message> {
    content: Element<'a, Message>,
    on_release: Message,
}

impl<Message: Clone> Widget<Message, Theme, Renderer> for ReleaseArea<'_, Message> {
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
        if let Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) = event {
            shell.publish(self.on_release.clone());
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
