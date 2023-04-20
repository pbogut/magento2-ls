import Parser from 'tree-sitter';
import Php from 'tree-sitter-php';
import { glob } from 'glob';
import fs from 'fs'

const phpClasses: Map<string, PHPClass> = new Map();

const query_string = `
  (namespace_definition (namespace_name) @namespace)
  (class_declaration (name) @class)
  ((method_declaration (visibility_modifier) @_vis (name) @name) (#eq? @_vis "public"))
`;

const parser = new Parser();
parser.setLanguage(Php);
const query = new Parser.Query(Php, query_string);

export class PHPClass {
  fqn: string;
  cls: Parser.SyntaxNode;
  methods: Parser.SyntaxNode[];
  file: string;

  constructor(
    namespace: Parser.SyntaxNode,
    cls: Parser.SyntaxNode,
    methods: Parser.SyntaxNode[],
    file: string
  ) {
    this.fqn = namespace.text + '\\' + cls.text;
    this.cls = cls;
    this.methods = methods;
    this.file = file;
  }
}

const trimLength = 'registration.php'.length

export function getLocation(fqn: string, method: string | null = null) {
  let phpClass = phpClasses.get(fqn)

  if (phpClass == null) {
    return null;
  }

  let node = phpClass.cls
  if (method != null) {
    phpClass.methods.forEach((m) => {
      if (m.text == method) {
        node = m
      }
    })
  }

  return {
    file: phpClass.file,
    start: {
      line: node.startPosition.row,
      character: node.startPosition.column
    },
    end: {
      line: node.endPosition.row,
      character: node.endPosition.column
    }
  }
}

export function collectPhpClasses(dir: string) {
  dir = dir.endsWith('/') ? dir : dir + '/'
  glob(dir + '**/registration.php').then((modules) => {
    modules.forEach((module) => {
      glob(module.slice(0, -trimLength) + '**/*.php').then((files) => {
        files.forEach((file) => {
          if (file.endsWith('Test.php')) {
            return
          }
          if (fs.statSync(file).isFile()) {
            let tree = parser.parse(fs.readFileSync(file, 'utf8'));
            // console.log(tree.rootNode.toString())
            let matches = query.matches(tree.rootNode);
            if (matches.length > 1) {
              let ns = null
              let cls = null
              let methods: Parser.SyntaxNode[] = []

              matches.forEach((match) => {
                if (match.pattern == 0) {
                  ns = match.captures[0].node
                }
                if (match.pattern == 1) {
                  cls = match.captures[0].node
                }
                if (match.pattern == 2) {
                  methods.push(match.captures[1].node)
                }
              });

              if (!ns || !cls) {
                return
              }

              let php = new PHPClass(
                ns,
                cls,
                methods,
                file
              )
              phpClasses.set(php.fqn, php)
            }
          }
        })
      })
    })
  });
}
