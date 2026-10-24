//! Loads and validates the built-in `tools/definitions.toml`.

use std::collections::HashSet;

use anyhow::{Context, Result, bail};

use super::schema::{Definitions, SourceDef, Walk};

const DEFINITIONS: &str = include_str!("../../tools/definitions.toml");

pub fn load() -> Result<Definitions> {
    parse(DEFINITIONS).context("built-in tools/definitions.toml is invalid")
}

pub fn parse(text: &str) -> Result<Definitions> {
    let defs: Definitions = toml::from_str(text)?;
    validate(&defs)?;
    Ok(defs)
}

fn validate(defs: &Definitions) -> Result<()> {
    let mut ids = HashSet::new();
    for tool in &defs.tools {
        if !ids.insert(tool.id.as_str()) {
            bail!("duplicate tool id `{}`", tool.id);
        }
        if tool.sources.is_empty() {
            bail!("tool `{}` has no sources", tool.id);
        }
        for (i, src) in tool.sources.iter().enumerate() {
            validate_source(src)
                .with_context(|| format!("tool `{}`, source #{}", tool.id, i + 1))?;
            let uses_config_dir = src
                .path
                .as_deref()
                .is_some_and(|p| p.contains("{config_dir}"))
                || src
                    .glob
                    .as_deref()
                    .is_some_and(|g| g.contains("{config_dir}"));
            if uses_config_dir && tool.config_dir.is_none() {
                bail!(
                    "tool `{}` uses {{config_dir}} but declares no config_dir",
                    tool.id
                );
            }
        }
    }
    Ok(())
}

fn validate_source(src: &SourceDef) -> Result<()> {
    let has_path = src.path.is_some();
    let has_candidates = !src.candidates.is_empty();
    let has_glob = src.glob.is_some();
    match src.walk {
        Walk::None => {
            if !(has_path || has_glob) {
                bail!("walk = none needs `path` or `glob`");
            }
            if has_path && has_glob {
                bail!("`path` and `glob` are mutually exclusive");
            }
        }
        Walk::FirstMatch => {
            if !has_candidates || has_path || has_glob {
                bail!("walk = first_match needs `candidates` only");
            }
        }
        _ => {
            if has_path {
                bail!("`path` only makes sense with walk = none");
            }
            if !(has_candidates || has_glob) {
                bail!("walking sources need `candidates` or `glob`");
            }
        }
    }
    if src.one_per_dir && !has_candidates {
        bail!("one_per_dir needs `candidates`");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_definitions_are_valid() {
        let defs = load().expect("definitions parse");
        assert!(defs.tools.iter().any(|t| t.id == "claude-code"));
    }

    #[test]
    fn rejects_duplicate_ids() {
        let text = r#"
[[tool]]
id = "x"
name = "X"
homepage = "h"
[[tool.source]]
scope = "user"
path = "~/x"
[[tool]]
id = "x"
name = "X"
homepage = "h"
[[tool.source]]
scope = "user"
path = "~/x"
"#;
        assert!(parse(text).is_err());
    }
}
