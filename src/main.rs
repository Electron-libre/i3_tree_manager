use std::error::Error;

use i3ipc::{reply::Node, I3Connection, I3EventListener};
use termion::event::Key;

use crate::event::{Event, Events};

#[allow(dead_code)]
mod event;
mod ui;

struct State {
    node_tree: Node,
    selected: i64,
    node_ids: Vec<i64>,
}

fn collect_ids(node: &Node) -> Vec<i64> {
    let mut ids = vec![node.id];
    ids.extend(node.nodes.iter().flat_map(|n| collect_ids(n)));
    ids
}

impl State {
    fn new(node: Node) -> Self {
        Self {
            selected: node.id,
            node_ids: collect_ids(&node),
            node_tree: node,
        }
    }

    fn update_tree(&mut self, node: Node) {
        self.node_tree = node;
    }

    fn select_next(&mut self) {
        let mut cursor = self.node_ids.iter();
        cursor.position(|id| id == &self.selected);
        if let Some(selected) = cursor.next() {
            self.selected = *selected
        }
    }

    fn select_previous(&mut self) {
        let mut cursor = self.node_ids.iter();
        if let Some(current) = cursor.rposition(|id| id == &self.selected) {
            if let Some(selected) = self.node_ids.get(current - 1) {
                self.selected = *selected
            }
        };
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut message_port = I3Connection::connect().unwrap();
    let i3_event_listener = I3EventListener::connect().unwrap();
    let mut state = State::new(message_port.get_tree()?);
    let events = Events::new(i3_event_listener);
    let mut renderer = ui::Renderer::new()?;

    loop {
        renderer.render(&state)?;

        match events.next()? {
            Event::Input(input) => match input {
                Key::Char('q') => {
                    break;
                }
                Key::Down => state.select_next(),
                Key::Up => state.select_previous(),
                _ => {}
            },
            Event::I3 => {
                state.update_tree(message_port.get_tree()?);
            }
            _ => (),
        }
    }
    Ok(())
}
