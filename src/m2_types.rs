use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum M2Item {
    Component(String),
    ModComponent(String, String, PathBuf),
    RelComponent(String, PathBuf),
    Class(String),
    Method(String, String),
    Const(String, String),
    FrontPhtml(String, String),
    AdminPhtml(String, String),
}
