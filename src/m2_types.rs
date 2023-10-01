use std::path::{Path, PathBuf};

use lsp_types::Url;

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
    // BasePhtml(String, String),
}

pub enum M2Area {
    Frontend,
    Adminhtml,
    Base,
}

pub trait M2Path {
    fn has_components(&self, parts: &[&str]) -> bool;
    fn relative_to<P: AsRef<Path>>(&self, base: P) -> PathBuf;
    fn append(&self, parts: &[&str]) -> Self;
    fn is_xml(&self) -> bool;
    fn is_frontend(&self) -> bool;
    fn is_test(&self) -> bool;
    fn get_area(&self) -> Option<M2Area>;
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

    fn relative_to<P: AsRef<Path>>(&self, base: P) -> PathBuf {
        self.strip_prefix(base).unwrap_or(self).to_path_buf()
    }

    fn to_path_string(&self) -> String {
        self.to_str()
            .expect("PathBuf should convert to path String")
            .to_string()
    }

    fn get_area(&self) -> Option<M2Area> {
        if self.has_components(&["view", "base"]) || self.has_components(&["design", "base"]) {
            Some(M2Area::Base)
        } else if self.has_components(&["view", "frontend"])
            || self.has_components(&["design", "frontend"])
        {
            Some(M2Area::Frontend)
        } else if self.has_components(&["view", "adminhtml"])
            || self.has_components(&["design", "adminhtml"])
        {
            Some(M2Area::Adminhtml)
        } else {
            None
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

impl M2Path for Url {
    fn has_components(&self, parts: &[&str]) -> bool {
        self.to_file_path()
            .expect("Url should convert to PathBuf")
            .has_components(parts)
    }

    fn relative_to<P: AsRef<Path>>(&self, base: P) -> PathBuf {
        self.to_file_path()
            .expect("Url should convert to PathBuf")
            .relative_to(base)
    }

    fn to_path_string(&self) -> String {
        self.to_file_path()
            .expect("Url should convert to PathBuf")
            .to_path_string()
    }

    fn get_area(&self) -> Option<M2Area> {
        self.to_file_path()
            .expect("Url should convert to PathBuf")
            .get_area()
    }

    fn append(&self, parts: &[&str]) -> Self {
        let mut uri = self.clone();
        for part in parts {
            uri = uri.join(part).unwrap_or(uri);
        }
        uri
    }

    fn is_xml(&self) -> bool {
        self.to_file_path()
            .expect("Url should convert to PathBuf")
            .is_xml()
    }

    fn is_frontend(&self) -> bool {
        self.to_file_path()
            .expect("Url should convert to PathBuf")
            .is_frontend()
    }

    fn is_test(&self) -> bool {
        self.to_file_path()
            .expect("Url should convert to PathBuf")
            .is_test()
    }
}

#[cfg(test)]
mod test {
    use crate::m2_types::M2Path;

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
}
