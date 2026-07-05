//! Registro de ferramentas: o conjunto padrão anunciado ao modelo.

use crate::bash::BashTool;
use crate::edit::EditTool;
use crate::grep::GrepTool;
use crate::read::ReadTool;
use crate::Tool;
use std::path::Path;

pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    /// Conjunto padrão da Fase 1: read, grep, edit, bash.
    pub fn default_set(root: &Path) -> Self {
        let root = root.to_path_buf();
        Self {
            tools: vec![
                Box::new(ReadTool { root: root.clone() }),
                Box::new(GrepTool { root: root.clone() }),
                Box::new(EditTool { root: root.clone() }),
                Box::new(BashTool { root }),
            ],
        }
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools
            .iter()
            .find(|t| t.name() == name)
            .map(|t| t.as_ref())
    }

    pub fn iter(&self) -> impl Iterator<Item = &dyn Tool> {
        self.tools.iter().map(|t| t.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conjunto_padrao_tem_as_quatro_ferramentas() {
        let dir = tempfile::tempdir().unwrap();
        let reg = ToolRegistry::default_set(dir.path());
        for name in ["read", "grep", "edit", "bash"] {
            assert!(reg.get(name).is_some(), "{name}");
        }
        assert!(reg.get("inexistente").is_none());
    }
}
