use std::path::{Path, PathBuf};

use lsp_types::Url;

#[allow(clippy::module_name_repetitions)]
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
    BasePhtml(String, String),
}

#[allow(clippy::module_name_repetitions)]
pub enum M2Area {
    Frontend,
    Adminhtml,
    Base,
    Unknown,
}

impl M2Area {
    pub fn path_candidates(&self) -> Vec<String> {
        match self {
            Self::Frontend => vec!["frontend".to_string(), "base".to_string()],
            Self::Adminhtml => vec!["adminhtml".to_string(), "base".to_string()],
            Self::Base => vec!["base".to_string()],
            Self::Unknown => vec![],
        }
    }
}

impl ToString for M2Area {
    fn to_string(&self) -> String {
        match self {
            Self::Frontend => "frontend".to_string(),
            Self::Adminhtml => "adminhtml".to_string(),
            Self::Base => "base".to_string(),
            Self::Unknown => "unknown".to_string(),
        }
    }
}

#[allow(clippy::module_name_repetitions)]
pub trait M2Uri {
    fn to_path_buf(&self) -> PathBuf;
}

#[allow(clippy::module_name_repetitions)]
pub trait M2Path {
    fn has_components(&self, parts: &[&str]) -> bool;
    fn relative_to<P: AsRef<Path>>(&self, base: P) -> PathBuf;
    fn append(&self, parts: &[&str]) -> Self;
    fn append_ext(&self, ext: &str) -> Self;
    fn is_xml(&self) -> bool;
    fn is_frontend(&self) -> bool;
    fn is_test(&self) -> bool;
    fn get_area(&self) -> M2Area;
    fn to_path_string(&self) -> String;
}

impl M2Path for PathBuf {
    fn append(&self, parts: &[&str]) -> Self {
        let mut path = self.clone();
        for part in parts {
            path = path.join(part);
        }
        path
    }

    fn append_ext(&self, ext: &str) -> Self {
        let mut path = self.clone();
        path.set_extension(ext);
        path
    }

    fn relative_to<P: AsRef<Path>>(&self, base: P) -> PathBuf {
        self.strip_prefix(base).unwrap_or(self).to_path_buf()
    }

    fn to_path_string(&self) -> String {
        self.to_str()
            .expect("PathBuf should convert to path String")
            .to_string()
    }

    fn get_area(&self) -> M2Area {
        if self.has_components(&["view", "base"]) || self.has_components(&["design", "base"]) {
            M2Area::Base
        } else if self.has_components(&["view", "frontend"])
            || self.has_components(&["design", "frontend"])
        {
            M2Area::Frontend
        } else if self.has_components(&["view", "adminhtml"])
            || self.has_components(&["design", "adminhtml"])
        {
            M2Area::Adminhtml
        } else {
            M2Area::Unknown
        }
    }

    fn has_components(&self, parts: &[&str]) -> bool {
        let mut start = false;
        let mut part_id = 0;
        for component in self.components() {
            let component = component
                .as_os_str()
                .to_str()
                .expect("Component should convert to &str");
            if start && parts[part_id] != component {
                return false;
            }
            if parts[part_id] == component {
                start = true;
                part_id += 1;
            }
            if start && parts.len() == part_id {
                return true;
            }
        }
        false
    }

    fn is_xml(&self) -> bool {
        self.extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default()
            .to_lowercase()
            == "xml"
    }

    fn is_frontend(&self) -> bool {
        self.has_components(&["view", "frontend"])
            || self.has_components(&["app", "design", "frontend"])
    }

    fn is_test(&self) -> bool {
        self.has_components(&["dev", "tests"])
    }
}

impl M2Uri for Url {
    fn to_path_buf(&self) -> PathBuf {
        self.to_file_path().expect("Url should convert to PathBuf")
    }
}

pub fn is_part_of_module_name(text: &str) -> bool {
    text.chars()
        .reduce(|a, b| {
            if b.is_alphanumeric() || b == '_' && (a != 'N') {
                'Y'
            } else {
                'N'
            }
        })
        .unwrap_or_default()
        == 'Y'
}

#[cfg(test)]
mod test {
    use crate::m2::M2Path;

    #[test]
    fn test_has_components_when_components_in_the_middle() {
        let path = std::path::PathBuf::from("app/code/Magento/Checkout/Block/Cart.php");
        assert!(path.has_components(&["Magento", "Checkout"]));
    }

    #[test]
    fn test_has_components_when_components_at_start() {
        let path = std::path::PathBuf::from("app/code/Magento/Checkout/Block/Cart.php");
        assert!(path.has_components(&["app", "code"]));
    }

    #[test]
    fn test_has_components_when_components_at_end() {
        let path = std::path::PathBuf::from("app/code/Magento/Checkout/Block/Cart.php");
        assert!(path.has_components(&["Block", "Cart.php"]));
    }

    #[test]
    fn test_has_components_when_components_are_not_in_order() {
        let path = std::path::PathBuf::from("app/code/Magento/Checkout/Block/Cart.php");
        assert!(!path.has_components(&["Checkout", "Cart.php"]));
    }

    #[test]
    fn test_if_extention_can_be_add_with_append() {
        let path = std::path::PathBuf::from("app/code/Magento/Checkout/Block/Cart");
        assert_eq!(
            path.append_ext("php").to_str().unwrap(),
            "app/code/Magento/Checkout/Block/Cart.php"
        );
    }
}
