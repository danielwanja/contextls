//! Git lookups by shelling out — one `rev-parse`, one `ls-files`, one batched
//! `check-ignore` per scan. Degrades to `Unknown` when git is missing.

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::model::GitStatus;

#[derive(Debug, Default)]
pub struct Git {
    pub toplevel: Option<PathBuf>,
    tracked: HashSet<PathBuf>,
    enabled: bool,
}

impl Git {
    pub fn discover(root: &Path, enabled: bool) -> Git {
        if !enabled {
            return Git::default();
        }
        let Some(top) = run(root, &["rev-parse", "--show-toplevel"], None) else {
            return Git {
                enabled: true,
                ..Git::default()
            };
        };
        let top = PathBuf::from(String::from_utf8_lossy(&top).trim_end());
        let top = top.canonicalize().unwrap_or(top);
        let tracked = run(&top, &["ls-files", "-z"], None)
            .map(|out| {
                out.split(|b| *b == 0)
                    .filter(|s| !s.is_empty())
                    .map(|s| top.join(String::from_utf8_lossy(s).as_ref()))
                    .collect()
            })
            .unwrap_or_default();
        Git {
            toplevel: Some(top),
            tracked,
            enabled: true,
        }
    }

    pub fn classify(&self, paths: &[PathBuf]) -> HashMap<PathBuf, GitStatus> {
        let mut out = HashMap::new();
        if !self.enabled {
            return paths
                .iter()
                .map(|p| (p.clone(), GitStatus::Unknown))
                .collect();
        }
        let Some(top) = &self.toplevel else {
            return paths
                .iter()
                .map(|p| (p.clone(), GitStatus::NotRepo))
                .collect();
        };
        let mut unknown = Vec::new();
        for p in paths {
            let canon = p.canonicalize().unwrap_or_else(|_| p.clone());
            if !canon.starts_with(top) {
                out.insert(p.clone(), GitStatus::NotRepo);
            } else if self.tracked.contains(&canon) {
                out.insert(p.clone(), GitStatus::Tracked);
            } else {
                out.insert(p.clone(), GitStatus::Untracked);
                unknown.push((p.clone(), canon));
            }
        }
        if unknown.is_empty() {
            return out;
        }
        let mut stdin = Vec::new();
        for (_, canon) in &unknown {
            let rel = canon.strip_prefix(top).unwrap_or(canon);
            stdin.extend_from_slice(rel.to_string_lossy().as_bytes());
            stdin.push(0);
        }
        if let Some(res) = run(top, &["check-ignore", "-z", "--stdin"], Some(&stdin)) {
            let ignored: HashSet<PathBuf> = res
                .split(|b| *b == 0)
                .filter(|s| !s.is_empty())
                .map(|s| top.join(String::from_utf8_lossy(s).as_ref()))
                .collect();
            for (orig, canon) in unknown {
                if ignored.contains(&canon) {
                    out.insert(orig, GitStatus::Ignored);
                }
            }
        }
        out
    }
}

/// Run git in `cwd`; `None` on spawn failure. `check-ignore` exits 1 when
/// nothing is ignored, so any exit code with output is accepted.
fn run(cwd: &Path, args: &[&str], stdin: Option<&[u8]>) -> Option<Vec<u8>> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(cwd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
    let mut child = cmd.spawn().ok()?;
    if let Some(data) = stdin {
        let mut pipe = child.stdin.take()?;
        pipe.write_all(data).ok()?;
    }
    let out = child.wait_with_output().ok()?;
    if out.status.success() || (args[0] == "check-ignore" && out.status.code() == Some(1)) {
        Some(out.stdout)
    } else {
        None
    }
}
