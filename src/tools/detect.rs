//! Is this tool installed here? Cheap, evidence-based: a config directory,
//! a binary on PATH, a macOS app bundle, or a glob (e.g. an editor extension).

use std::path::{Path, PathBuf};

use crate::model::{Install, display_path};
use crate::scan::{Ctx, Env};

use super::resolve::{expand, glob_paths};
use super::schema::ToolDef;

pub fn detect(def: &ToolDef, ctx: &Ctx) -> Install {
    let Some(d) = &def.detect else {
        return Install::Unknown;
    };
    let mut looked_for = Vec::new();
    let shown = |p: &Path| display_path(p, &ctx.root, &ctx.home);

    for dir in &d.dirs {
        let p = expand(dir, def, ctx);
        if p.exists() {
            return Install::Installed {
                evidence: format!("found {}", shown(&p)),
            };
        }
        looked_for.push(shown(&p));
    }
    for bin in &d.bins {
        if let Some(p) = which(bin, &ctx.env) {
            return Install::Installed {
                evidence: format!("`{bin}` on PATH ({})", shown(&p)),
            };
        }
        looked_for.push(format!("`{bin}` on PATH"));
    }
    if ctx.platform == "macos" {
        for app in &d.apps {
            let candidates = [
                PathBuf::from("/Applications").join(app),
                ctx.home.join("Applications").join(app),
            ];
            if let Some(p) = candidates.iter().find(|p| p.exists()) {
                return Install::Installed {
                    evidence: format!("found {}", shown(p)),
                };
            }
            looked_for.push(app.clone());
        }
    }
    for g in &d.globs {
        let pat = expand(g, def, ctx);
        if let Some(p) = glob_paths(&pat.to_string_lossy(), true).into_iter().next() {
            return Install::Installed {
                evidence: format!("found {}", shown(&p)),
            };
        }
        looked_for.push(shown(&pat));
    }
    Install::NotInstalled { looked_for }
}

/// First executable named `bin` on the environment's PATH.
pub fn which(bin: &str, env: &Env) -> Option<PathBuf> {
    let path = env.get("PATH")?;
    path.split(':')
        .filter(|d| !d.is_empty())
        .map(|d| Path::new(d).join(bin))
        .find(|p| is_executable(p))
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}
