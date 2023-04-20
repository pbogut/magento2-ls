import { Position, Callable } from './types.js';
import Parser from 'tree-sitter';
import Xml from 'tree-sitter-html';
import fs from 'fs';

const query_string = `
(attribute_value) @attr
(text) @text

(self_closing_tag (tag_name)
  (attribute (attribute_name ) @_attr2 (#eq? @_attr2 "class")
    (quoted_attribute_value (attribute_value) @class))
  ) @callable
(self_closing_tag (tag_name)
  (attribute (attribute_name) @_attr (#eq? @_attr "method")
    (quoted_attribute_value (attribute_value) @method))
  ) @callable
(self_closing_tag (tag_name) @_name
  (attribute (attribute_name ) @_attr2 (#eq? @_attr2 "instance")
    (quoted_attribute_value (attribute_value) @class))
  ) @callable
(start_tag (tag_name)
  (attribute (attribute_name ) @_attr2 (#eq? @_attr2 "class")
    (quoted_attribute_value (attribute_value) @class))
  ) @callable
(start_tag (tag_name)
  (attribute (attribute_name) @_attr (#eq? @_attr "method")
    (quoted_attribute_value (attribute_value) @method))
  ) @callable
(start_tag (tag_name) @_name
  (attribute (attribute_name ) @_attr2 (#eq? @_attr2 "instance")
    (quoted_attribute_value (attribute_value) @class))
  ) @callable
`;

const parser = new Parser();
parser.setLanguage(Xml);
const query = new Parser.Query(Xml, query_string);

const getCallable = (node: Parser.SyntaxNode): Callable | null => {
  let class_name = null;
  let method_name = null;
  node.namedChildren.forEach((child) => {
    if (child.type == 'attribute') {
      if (['class', 'instance'].includes(child.namedChildren[0].text)) {
        class_name = child.namedChildren[1].namedChildren[0].text;
      }
      if (child.namedChildren[0].text == 'method') {
        method_name = child.namedChildren[1].namedChildren[0].text;
      }
    }
  });
  if (class_name == null) {
    return null;
  }
  return { class_name, method_name };
};

export const getAtPosition = (file: string, position: Position): Callable | string | null => {
  file = file.slice(7); // get rid of file://
  let tree = parser.parse(fs.readFileSync(file, 'utf8'));

  let matches = query.matches(tree.rootNode);

  let node_text: string | null = null;
  let callable: null | Callable = null;

  matches.forEach((match) => {
    let node = match.captures[0].node;
    if (
      node.startPosition.row <= position.line
      && node.endPosition.row >= position.line
      && node.startPosition.column <= position.character
      && node.endPosition.column - 1 >= position.character
    ) {
      if (node.type == 'attribute_value' || node.type == 'text') {
        node_text = node.text;
      }
      if (node.type == 'self_closing_tag' || node.type == 'start_tag') {
        callable = getCallable(node);
      }
    }
  });

  if (!node_text) {
    return null;
  }

  node_text = node_text as string;

  if (node_text.startsWith('\\')) {
    node_text = node_text.slice(1);
  }

  if (callable != null) {
    callable = callable as Callable;

    if (callable.method_name == node_text || callable.class_name == node_text) {
      return callable;
    }
  }

  return node_text;
}
