use std::sync::OnceLock;

use parking_lot::Once;
use tree_sitter::{Language, Query};

pub static JS_REQUIRE_CONFIG: OnceLock<Query> = OnceLock::new();
pub static JS_ITEM_FROM_POS: OnceLock<Query> = OnceLock::new();

pub static PHP_REGISTRATION: OnceLock<Query> = OnceLock::new();
pub static PHP_CLASS: OnceLock<Query> = OnceLock::new();

pub static XML_TAG_AT_POS: OnceLock<Query> = OnceLock::new();
pub static XML_CURRENT_POSITION_PATH: OnceLock<Query> = OnceLock::new();

static BUILD: Once = Once::new();

pub fn js_require_config() -> &'static Query {
    get_query(&JS_REQUIRE_CONFIG)
}

pub fn php_registration() -> &'static Query {
    get_query(&PHP_REGISTRATION)
}

pub fn php_class() -> &'static Query {
    get_query(&PHP_CLASS)
}

pub fn xml_tag_at_pos() -> &'static Query {
    get_query(&XML_TAG_AT_POS)
}

pub fn xml_current_position_path() -> &'static Query {
    get_query(&XML_CURRENT_POSITION_PATH)
}

pub fn js_item_from_pos() -> &'static Query {
    get_query(&JS_ITEM_FROM_POS)
}

fn build() {
    BUILD.call_once(|| {
        build_js_item_from_pos();
        build_js_require_config();
        build_php_class();
        build_php_registration();
        build_xml_current_position_path();
        build_xml_tag_at_pos();
    });
}

fn build_js_item_from_pos() {
    make_query(
        &JS_ITEM_FROM_POS,
        r#"
        (
            (identifier) @def (#eq? @def define)
            (arguments (array (string) @str))
        )
        "#,
        "javascript",
    );
}

fn build_js_require_config() {
    let map_query = r#"
    (
        (identifier) @config
        (object (pair [(property_identifier) (string)] @mapkey
            (object (pair (object (pair
              [(property_identifier) (string)] @key + (string) @val
            ))))
        ))

        (#eq? @config config)
        (#match? @mapkey "[\"']?map[\"']?")
    )
    "#;

    let mixins_query = r#"
    (
        (identifier) @config
        (object (pair [(property_identifier) (string)] ; @configkey
            (object (pair [(property_identifier) (string)] @mixins
                (object (pair [(property_identifier) (string)] @key
                    (object (pair [(property_identifier) (string)] @val (true)))
                ))
            ))
        ))

        (#match? @config config)
        ; (#match? @configkey "[\"']?config[\"']?")
        (#match? @mixins "[\"']?mixins[\"']?")
    )
    "#;

    let path_query = r#"
    (
        (identifier) @config
        (object (pair [(property_identifier) (string)] @pathskey
            (((object (pair
                [(property_identifier) (string)] @key + (string) @val
            ))))
        ))

        (#eq? @config config)
        (#match? @pathskey "[\"']?paths[\"']?")
    )
    "#;

    let query_string = format!("{} {} {}", map_query, path_query, mixins_query);
    make_query(&JS_REQUIRE_CONFIG, &query_string, "javascript");
}

fn build_php_class() {
    make_query(
        &PHP_CLASS,
        r#"
        (namespace_definition (namespace_name) @namespace) ; pattern: 0
        (class_declaration (name) @class)                  ; pattern: 1
        (interface_declaration (name) @class)              ; pattern: 2
        ((method_declaration (visibility_modifier)
          @_vis (name) @name) (#eq? @_vis "public"))       ; pattern: 3
        (const_element (name) @const)                      ; pattern: 4
        "#,
        "php",
    );
}

fn build_php_registration() {
    make_query(
        &PHP_REGISTRATION,
        r#"
        (scoped_call_expression
           (name) @reg (#eq? @reg register)
           (arguments
               (string) @module_name
           )
        )
        "#,
        "php",
    );
}

fn build_xml_current_position_path() {
    make_query(
        &XML_CURRENT_POSITION_PATH,
        r#"
        (tag_name) @tag_name
        (attribute_value) @attr_val
        (text) @text
        (end_tag) @end_tag
        ((quoted_attribute_value) @q_attr_val (#eq? @q_attr_val "\"\""))
        ((quoted_attribute_value) @q_attr_val (#eq? @q_attr_val "\""))
        "#,
        "html",
    );
}

fn build_xml_tag_at_pos() {
    make_query(
        &XML_TAG_AT_POS,
        r#"
        (element
            (start_tag
                (tag_name) @tag_name
                (attribute
                    (attribute_name) @attr_name
                    (quoted_attribute_value (attribute_value) @attr_val)
                )?
            ) @tag
            (text)? @text
        )
        (element
            (self_closing_tag
                (tag_name) @tag_name
                (attribute
                    (attribute_name) @attr_name
                    (quoted_attribute_value (attribute_value) @attr_val)
                )
            ) @tag
        )
        "#,
        "html",
    );
}

fn make_query(static_query: &'static OnceLock<Query>, query: &str, lang: &str) {
    let query = Query::new(get_language(lang), query)
        .map_err(|e| eprintln!("Error creating query: {:?}", e))
        .expect("Error creating query");
    static_query.set(query).expect("Set up static query");
}

fn get_query(static_query: &'static OnceLock<Query>) -> &'static Query {
    build();
    static_query.get().expect("Query should be created.")
}

fn get_language(lang: &str) -> Language {
    tree_sitter_parsers::parse("", lang).language()
}
