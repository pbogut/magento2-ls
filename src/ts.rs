use lsp_types::{Position, Range};
use tree_sitter::Node;

pub fn get_range_from_node(node: Node) -> Range {
    Range {
        start: Position {
            line: node.start_position().row as u32,
            character: node.start_position().column as u32,
        },
        end: Position {
            line: node.end_position().row as u32,
            character: node.end_position().column as u32,
        },
    }
}

pub fn get_node_text_before_pos(node: Node, content: &str, pos: Position) -> String {
    let text = node
        .utf8_text(content.as_bytes())
        .unwrap_or("")
        .trim_matches('\\');

    let node_start_pos = node.start_position();
    let node_end_pos = node.end_position();

    let text = if node_end_pos.row == node_start_pos.row {
        text.to_string()
    } else {
        let take_lines = pos.line as usize - node_start_pos.row;
        text.split('\n')
            .take(take_lines + 1)
            .collect::<Vec<&str>>()
            .join("\n")
    };

    if pos.line as usize == node_start_pos.row {
        let end = pos.character as usize - node_start_pos.column;
        text.chars().take(end).collect::<String>()
    } else {
        let mut trimed = false;
        let mut lines = text
            .split('\n')
            .rev()
            .map(|line| {
                if trimed {
                    line.to_owned()
                } else {
                    trimed = true;
                    line.chars()
                        .take(pos.character as usize)
                        .collect::<String>()
                }
            })
            .collect::<Vec<String>>();

        lines.reverse();
        lines.join("\n")
    }
}

pub fn get_node_str<'a>(node: Node, content: &'a str) -> &'a str {
    node.utf8_text(content.as_bytes())
        .unwrap_or("")
        .trim_matches('\\')
}

pub fn node_at_position(node: Node, pos: Position) -> bool {
    let start = node.start_position();
    let end = node.end_position();
    if pos.line < start.row as u32 || pos.line > end.row as u32 {
        return false;
    }
    if pos.line == start.row as u32 && pos.character < start.column as u32 {
        return false;
    }
    if pos.line == end.row as u32 && pos.character > end.column as u32 {
        return false;
    }
    true
}

pub fn node_last_child(node: Node) -> Option<Node> {
    let children_count = node.child_count();
    node.child(children_count - 1)
}
