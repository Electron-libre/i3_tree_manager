use std::error::Error;

use i3ipc::{reply::Node, I3Connection, I3EventListener};
use termion::event::Key;

use crate::event::{Event, Events};

#[allow(dead_code)]
mod event;
mod ui;

type NodeId = i64;

enum StateMode {
    Move(NodeId),
    None,
}

struct State {
    node_tree: Node,
    selected: NodeId,
    node_ids: Vec<NodeId>,
    mode: StateMode,
    message_port: I3Connection,
}

fn collect_ids(node: &Node) -> Vec<i64> {
    let mut ids = vec![node.id];
    ids.extend(node.nodes.iter().flat_map(|n| collect_ids(n)));
    ids
}

impl State {
    fn new() -> Self {
        let mut message_port = I3Connection::connect().unwrap();
        let node = message_port.get_tree().unwrap();
        Self {
            selected: node.id,
            node_ids: collect_ids(&node),
            node_tree: node,
            mode: StateMode::None,
            message_port,
        }
    }

    fn update_tree(&mut self) {
        let node = self.message_port.get_tree().unwrap();
        self.node_ids = collect_ids(&node);
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
            if current == 0 {
                return;
            };

            if let Some(selected) = self.node_ids.get(current - 1) {
                self.selected = *selected
            }
        };
    }

    fn move_mode(&mut self) {
        match self.mode {
            StateMode::None => self.mode = StateMode::Move(self.selected),
            StateMode::Move(_) => self.mode = StateMode::None,
        }
    }

    fn move_container(&mut self, direction: &str) {
        self.message_port
            .run_command(format!("[con_id=\"{}\"] move {}", self.selected, direction).as_str())
            .unwrap();
    }

    fn split_toggle(&mut self) {
        self.message_port
            .run_command(format!("[con_id=\"{}\"] split toggle", self.selected).as_str())
            .unwrap();
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let i3_event_listener = I3EventListener::connect().unwrap();
    let mut state = State::new();
    let events = Events::new(i3_event_listener);
    let mut renderer = ui::Renderer::new()?;

    loop {
        renderer.render(&state)?;

        match events.next()? {
            Event::Input(input) => match state.mode {
                StateMode::None => match input {
                    Key::Char('q') => {
                        break;
                    }
                    Key::Down => state.select_next(),
                    Key::Up => state.select_previous(),
                    Key::Char('m') => state.move_mode(),
                    Key::Char('s') => state.split_toggle(),
                    _ => {}
                },
                StateMode::Move(node_id) => match input {
                    Key::Char('q') => {
                        break;
                    }
                    Key::Esc => state.move_mode(),
                    Key::Down => state.move_container("down"),
                    Key::Up => state.move_container("up"),
                    Key::Left => state.move_container("left"),
                    Key::Right => state.move_container("right"),
                    Key::Char('s') => state.split_toggle(),
                    _ => {}
                },
            },
            Event::I3 => {
                state.update_tree();
            }
            _ => (),
        }
    }
    Ok(())
}
