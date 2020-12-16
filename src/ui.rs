use std::vec::IntoIter;
use std::{error::Error, io, io::Stdout, slice};

use i3ipc::reply::Node;
use termion::{input::MouseTerminal, raw::IntoRawMode, raw::RawTerminal, screen::AlternateScreen};
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Paragraph, Row, Table};
use tui::{
    backend::TermionBackend,
    layout::{Constraint, Corner, Direction, Layout},
    text::Text,
    widgets::{Block, Borders, List, ListItem},
    Terminal,
};

use crate::{State, StateMode};

#[derive(Clone)]
struct UiNode {
    con_id: i64,
    name: String,
    indentation: String,
    node_type: String,
    layout: String,
    focused: bool,
    urgent: bool,
}

impl UiNode {
    fn from(node: Node, indentation: String) -> Self {
        Self {
            con_id: node.id,
            name: node.name.unwrap_or_default(),
            layout: format!("{:?}", node.layout),
            node_type: format!("{:?}", node.nodetype),
            focused: node.focused,
            urgent: node.urgent,
            indentation,
        }
    }
}

static BRANCH_INDENT: &str = "│  ";
static LEAF_INDENT: &str = "   ";
static BRANCH_GLYPH: &str = "├──";
static LEAF_GLYPH: &str = "└──";
static ROOT_GLYPH: &str = "";
static EMPTY_INDENT: &str = "";

struct Context {
    ancestors_indent: String,
    level: TreeLevel,
    selected_id: Option<i64>,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            ancestors_indent: EMPTY_INDENT.to_string(),
            level: TreeLevel::Root,
            selected_id: None,
        }
    }
}

enum TreeLevel {
    Root,
    Branch,
    Leaf,
}

impl Context {
    fn descendant_indent(&self) -> String {
        let fill = match self.level {
            TreeLevel::Root => EMPTY_INDENT,
            TreeLevel::Branch => BRANCH_INDENT,
            TreeLevel::Leaf => LEAF_INDENT,
        };
        format!("{}{}", self.ancestors_indent, fill)
    }

    fn glyph(&self) -> String {
        match self.level {
            TreeLevel::Root => ROOT_GLYPH,
            TreeLevel::Branch => BRANCH_GLYPH,
            TreeLevel::Leaf => LEAF_GLYPH,
        }
        .to_string()
    }

    fn full_entry(&self) -> String {
        format!("{}{}", self.ancestors_indent, self.glyph())
    }

    fn to_leaf(&self) -> Self {
        Self {
            ancestors_indent: self.descendant_indent(),
            level: TreeLevel::Leaf,
            selected_id: self.selected_id,
        }
    }

    fn to_branch(&self) -> Self {
        Self {
            ancestors_indent: self.descendant_indent(),
            level: TreeLevel::Branch,
            selected_id: self.selected_id,
        }
    }
}

/// Recursively build a list of items with string representation of tree
fn node_into_ui_list<'a>(node: &Node, context: Context) -> Vec<ListItem<'a>> {
    let mut root = ListItem::new(UiNode::from(node.clone(), context.full_entry()));
    if node.urgent {
        root = root.style(Style::default().bg(Color::LightMagenta));
    }
    if node.focused {
        root = root.style(Style::default().bg(Color::LightGreen));
    }
    if Some(node.id) == context.selected_id {
        root = root.style(Style::default().add_modifier(Modifier::REVERSED));
    }

    let mut tree_list = vec![root];
    let mut branches = node.nodes.clone();
    let leaf = branches.pop();

    if let Some(ref last) = leaf {
        branches
            .iter()
            .fold(&mut tree_list, |lst, node| {
                lst.append(&mut node_into_ui_list(node, context.to_branch()));
                lst
            })
            .append(&mut node_into_ui_list(last, context.to_leaf()))
    }
    tree_list
}

impl From<UiNode> for Text<'_> {
    fn from(ui_node: UiNode) -> Self {
        Self::from(format!(
            "{}[{}] {{{}}} - {}",
            ui_node.indentation, ui_node.node_type, ui_node.layout, ui_node.name
        ))
    }
}

fn build_tree_widget(tree_items: Vec<ListItem>) -> List {
    List::new(tree_items)
        .block(Block::default().borders(Borders::ALL).title("I3 Tree"))
        .start_corner(Corner::TopLeft)
}

fn build_menu_span<'a>(mode: &'a str, actions: Vec<(&'a str, &'a str)>) -> Spans<'a> {
    let mode = Span::styled(
        format!("{} ┃", mode),
        Style::default().add_modifier(Modifier::REVERSED),
    );

    let actions = actions
        .into_iter()
        .fold(vec![mode], |mut acc, (key, action)| {
            let key = Span::styled(
                format!(" {} ∷ ", key),
                Style::default().add_modifier(Modifier::BOLD),
            );
            let action = Span::raw(format!("{} ┃", action));
            acc.push(key);
            acc.push(action);
            acc
        });

    Spans::from(actions)
}

fn build_menu_widget(state: &State) -> Paragraph {
    let block = Block::default().title("Commands").borders(Borders::ALL);

    let menu_span = match state.mode {
        StateMode::Move(_) => {
            let actions = vec![
                ("ESC", "exit mode"),
                ("UP", "move up"),
                ("DOWN", "move down"),
                ("LEFT", "move left"),
                ("RIGHT", "move right"),
            ];

            build_menu_span("Move", actions)
        }
        StateMode::None => {
            let actions = vec![("m", "move mode"), ("s", "toggle split"), ("q", "quit")];
            build_menu_span("Select", actions)
        }
    };
    Paragraph::new(menu_span).block(block)
}

type IOBoundTerminal =
    Terminal<TermionBackend<AlternateScreen<MouseTerminal<RawTerminal<Stdout>>>>>;

pub(crate) struct Renderer(IOBoundTerminal);

impl Renderer {
    pub(crate) fn new() -> io::Result<Self> {
        let stdout = io::stdout().into_raw_mode()?;
        let stdout = MouseTerminal::from(stdout);
        let stdout = AlternateScreen::from(stdout);
        let backend = TermionBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self(terminal))
    }

    pub(crate) fn render(&mut self, state: &State) -> Result<(), Box<dyn Error>> {
        self.0.draw(|frame| {
            let tree_items = node_into_ui_list(
                &state.node_tree,
                Context {
                    selected_id: Some(state.selected),
                    ..Context::default()
                },
            );
            let tree_widget = build_tree_widget(tree_items);
            let menu_widget = build_menu_widget(state);
            // Layout
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
                .split(frame.size());

            frame.render_widget(menu_widget, split[0]);
            frame.render_widget(tree_widget, split[1]);
        })?;
        Ok(())
    }
}
