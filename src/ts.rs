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

pub fn get_node_text(node: Node, content: &str) -> String {
    node.utf8_text(content.as_bytes()).unwrap_or("").to_string()
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
