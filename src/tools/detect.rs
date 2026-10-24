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

/// First executable named `bin` on PATH, then in the usual install locations
/// a non-login shell or GUI launcher may not have on PATH (npm / nvm / volta
/// / pnpm / bun / Homebrew / cargo).
pub fn which(bin: &str, env: &Env) -> Option<PathBuf> {
    let names: Vec<String> = if cfg!(windows) {
        vec![bin.to_string(), format!("{bin}.exe"), format!("{bin}.cmd")]
    } else {
        vec![bin.to_string()]
    };
    search_dirs(env)
        .into_iter()
        .flat_map(|d| names.iter().map(move |n| d.join(n)))
        .find(|p| is_executable(p))
}

/// PATH entries followed by common install directories that exist.
pub fn search_dirs(env: &Env) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = env
        .get("PATH")
        .map(|p| {
            std::env::split_paths(&p)
                .filter(|d| !d.as_os_str().is_empty())
                .collect()
        })
        .unwrap_or_default();

    let mut extra: Vec<PathBuf> = Vec::new();
    for (var, sub) in [
        ("NVM_BIN", ""),
        ("PNPM_HOME", ""),
        ("VOLTA_HOME", "bin"),
        ("BUN_INSTALL", "bin"),
        ("N_PREFIX", "bin"),
        ("NPM_CONFIG_PREFIX", "bin"),
    ] {
        if let Some(v) = env.get(var).filter(|v| !v.is_empty()) {
            extra.push(PathBuf::from(v).join(sub));
        }
    }
    if let Some(home) = env.get("HOME").map(PathBuf::from) {
        for rel in [
            ".local/bin",
            ".npm-global/bin",
            ".volta/bin",
            ".bun/bin",
            "Library/pnpm",
            ".local/share/pnpm",
            ".cargo/bin",
            "go/bin",
            "bin",
        ] {
            extra.push(home.join(rel));
        }
        let nvm = home.join(".nvm/versions/node/*/bin");
        extra.extend(glob_paths(&nvm.to_string_lossy(), true));
    }
    extra.extend(
        ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"]
            .iter()
            .map(PathBuf::from),
    );
    for d in extra {
        if d.is_dir() && !dirs.contains(&d) {
            dirs.push(d);
        }
    }
    dirs
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn which_falls_back_to_common_install_dirs() {
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join(".npm-global/bin");
        fs::create_dir_all(&bin).unwrap();
        let exe = bin.join("gemini");
        fs::write(&exe, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();

        let env = Env::from_pairs(&[
            ("HOME", home.path().to_str().unwrap()),
            ("PATH", "/nonexistent"),
        ]);
        assert_eq!(which("gemini", &env), Some(exe));
        assert_eq!(which("definitely-not-here", &env), None);
    }
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
