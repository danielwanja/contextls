//! Small helpers for reading context files: YAML frontmatter and list-ish values.

use anyhow::{Result, anyhow};

/// Parse a leading `---` YAML block into JSON. `Ok(None)` when there is none.
pub fn frontmatter(content: &str) -> Result<Option<serde_json::Value>> {
    let mut lines = content.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return Ok(None);
    }
    let mut yaml = String::new();
    let mut closed = false;
    for line in lines {
        if matches!(line.trim_end(), "---" | "...") {
            closed = true;
            break;
        }
        yaml.push_str(line);
        yaml.push('\n');
    }
    if !closed {
        return Err(anyhow!("opening `---` without a closing one"));
    }
    if yaml.trim().is_empty() {
        return Ok(Some(serde_json::Value::Object(Default::default())));
    }
    match serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&yaml) {
        Ok(value) => Ok(Some(serde_json::to_value(value)?)),
        // Cursor writes `globs: *.ts,*.tsx` unquoted, which is not valid YAML
        // (`*` starts an alias) but is what the tool itself accepts.
        Err(e) => loose(&yaml).ok_or_else(|| anyhow!("{e}")).map(Some),
    }
}

/// Minimal `key: value` parser for frontmatter that strict YAML rejects.
/// Values stay strings except `true`/`false` and `[a, b]` lists.
fn loose(yaml: &str) -> Option<serde_json::Value> {
    let mut map = serde_json::Map::new();
    for line in yaml.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line.split_once(':')?;
        let key = key.trim();
        if key.is_empty() || key.contains(char::is_whitespace) {
            return None;
        }
        let value = value.trim().trim_matches(['"', '\'']);
        let json = match value {
            "true" => serde_json::Value::Bool(true),
            "false" => serde_json::Value::Bool(false),
            v if v.starts_with('[') && v.ends_with(']') => serde_json::Value::Array(
                v[1..v.len() - 1]
                    .split(',')
                    .map(|s| s.trim().trim_matches(['"', '\'']))
                    .filter(|s| !s.is_empty())
                    .map(|s| serde_json::Value::String(s.to_string()))
                    .collect(),
            ),
            v => serde_json::Value::String(v.to_string()),
        };
        map.insert(key.to_string(), json);
    }
    Some(serde_json::Value::Object(map))
}

/// A frontmatter value that is either a string (possibly comma-separated) or an array of strings.
pub fn string_list(v: &serde_json::Value) -> Vec<String> {
    match v {
        serde_json::Value::String(s) => s
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect(),
        serde_json::Value::Array(a) => a
            .iter()
            .filter_map(|x| x.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter() {
        let fm = frontmatter("---\nglobs: \"*.ts, *.tsx\"\nalwaysApply: false\n---\n# hi\n")
            .unwrap()
            .unwrap();
        assert_eq!(string_list(&fm["globs"]), vec!["*.ts", "*.tsx"]);
        assert_eq!(fm["alwaysApply"], false);
    }

    #[test]
    fn cursor_style_unquoted_globs() {
        let fm = frontmatter(
            "---\ndescription: Rules for TS\nglobs: *.ts,*.tsx\nalwaysApply: false\n---\n",
        )
        .unwrap()
        .unwrap();
        assert_eq!(string_list(&fm["globs"]), vec!["*.ts", "*.tsx"]);
        assert_eq!(fm["alwaysApply"], false);
        assert_eq!(fm["description"], "Rules for TS");
    }

    #[test]
    fn no_frontmatter() {
        assert!(frontmatter("# just markdown\n---\n").unwrap().is_none());
    }

    #[test]
    fn unterminated_is_an_error() {
        assert!(frontmatter("---\nfoo: 1\n").is_err());
    }
}
