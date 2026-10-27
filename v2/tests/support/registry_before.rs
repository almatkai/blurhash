#![allow(dead_code, clippy::redundant_closure)]
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
};
struct Paths {
    data_dir: PathBuf,
}
struct CoreContext {
    paths: Paths,
}
fn read_json(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
mod process_install {
    use super::*;
    pub struct Package {
        pub manifest: Value,
        pub dir: PathBuf,
    }
    // Seam: stand-in for successfully VERIFIED Package returned by real installer. Never installs.
    pub fn resolve(ctx: &CoreContext, id: &str) -> Result<Package, String> {
        let root = ctx.paths.data_dir.join("extensions/packages").join(id);
        let a = read_json(&root.join("active.json"))?;
        let dir = root.join("versions").join(a["directory"].as_str().unwrap());
        Ok(Package {
            manifest: read_json(&dir.join("extension.json"))?,
            dir,
        })
    }
}
mod registry {
    use super::*;
    pub(super) struct Installed {
        id: String,
        slug: String,
        path: PathBuf,
        manifest: Value,
        data_dir: PathBuf,
    }
    fn slug(id: &str) -> Result<String, String> {
        let name = id
            .trim()
            .strip_prefix("com.tezbar.native.")
            .or_else(|| id.trim().strip_prefix("raycast."))
            .unwrap_or(id.trim());
        if name.is_empty()
            || name.len() > 200
            || !name.as_bytes()[0].is_ascii_alphanumeric()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err("Invalid extension id".into());
        }
        Ok(name.into())
    }
    fn managed_roots(ctx: &CoreContext) -> Vec<PathBuf> {
        vec![
            ctx.paths.data_dir.join("extensions"),
            ctx.paths.data_dir.join("extensions/packages"),
            ctx.paths.data_dir.join("extension-registry/packages"),
        ]
    }
    fn collect(ctx: &CoreContext) -> Vec<Installed> {
        let mut found = Vec::new();
        let mut seen = HashSet::new();
        for root in roots(ctx) {
            let mut paths = if root.join("package.json").is_file() {
                vec![root]
            } else {
                std::fs::read_dir(root)
                    .into_iter()
                    .flatten()
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .collect()
            };
            paths.sort();
            for path in paths {
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                let Ok(slug) = slug(name) else { continue };
                let native = name.starts_with("com.tezbar.native.");
                let mut manifest = if native {
                    match super::process_install::resolve(ctx, name) {
                        Ok(package) => package.manifest,
                        Err(error) => {
                            // A damaged canonical directory is NEVER a Raycast package.
                            let mut value = read_json(&path.join("package.json"))
                                .ok()
                                .filter(Value::is_object)
                                .unwrap_or(json!({"id":name,"title":slug}));
                            value["healthError"] = json!(error);
                            value["commands"] = json!([]);
                            value
                        }
                    }
                } else {
                    let Ok(value) = read_json(&path.join("package.json")) else {
                        continue;
                    };
                    value
                };
                if native {
                    manifest["id"] = json!(name);
                }
                let id = if native {
                    name.to_string()
                } else {
                    format!("raycast.{slug}")
                };
                if !manifest.is_object() || !seen.insert(id.to_lowercase()) {
                    continue;
                }
                found.push(Installed {
                    id,
                    slug,
                    path,
                    manifest,
                    data_dir: ctx.paths.data_dir.clone(),
                });
            }
        }
        found
    }
    fn fallback<'a>(value: &'a Value, key: &str, default: &'a str) -> &'a str {
        value[key]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or(default)
    }
    fn icon_path(extension: &Installed) -> Option<String> {
        let name = fallback(&extension.manifest, "icon", "icon.png");
        if name.starts_with("https://") || name.starts_with("http://") {
            return Some(name.into());
        }
        let relative = Path::new(name.strip_prefix("./").unwrap_or(name));
        if relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return None;
        }
        let mut candidates = vec![
            extension.path.join(relative),
            extension.path.join("assets").join(relative),
        ];
        if extension.id.starts_with("com.tezbar.native.") {
            if let Some(packages) = extension.path.parent() {
                if let Some(extensions) = packages.parent() {
                    let legacy = extensions.join(&extension.slug);
                    candidates.push(legacy.join(relative));
                    candidates.push(legacy.join("assets").join(relative));
                }
                let legacy = packages.join(format!("raycast.{}", extension.slug));
                candidates.push(legacy.join(relative));
                candidates.push(legacy.join("assets").join(relative));
            }
        }
        candidates
            .into_iter()
            .find(|p| p.is_file())
            .map(|p| p.to_string_lossy().into())
    }
    fn command_icon_path(extension: &Installed, cmd: &Value) -> Option<String> {
        let name = cmd["icon"].as_str().or_else(|| cmd["iconPath"].as_str())?;
        if name.starts_with("https://") || name.starts_with("http://") {
            return Some(name.into());
        }
        let relative = Path::new(name.strip_prefix("./").unwrap_or(name));
        if relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return None;
        }
        let mut candidates = vec![
            extension.path.join(relative),
            extension.path.join("assets").join(relative),
        ];
        if extension.id.starts_with("com.tezbar.native.") {
            if let Some(packages) = extension.path.parent() {
                if let Some(extensions) = packages.parent() {
                    let legacy = extensions.join(&extension.slug);
                    candidates.push(legacy.join(relative));
                    candidates.push(legacy.join("assets").join(relative));
                }
                let legacy = packages.join(format!("raycast.{}", extension.slug));
                candidates.push(legacy.join(relative));
                candidates.push(legacy.join("assets").join(relative));
            }
        }
        candidates
            .into_iter()
            .find(|p| p.is_file())
            .map(|p| p.to_string_lossy().into())
    }
    fn roots(ctx: &CoreContext) -> Vec<PathBuf> {
        managed_roots(ctx)
    }
    pub fn inspect(root: &Path) -> (PathBuf, Option<String>, Option<String>) {
        let ctx = CoreContext {
            paths: Paths {
                data_dir: root.into(),
            },
        };
        let entries = collect(&ctx);
        let e = entries
            .iter()
            .find(|e| e.id == "com.tezbar.native.blurhash")
            .unwrap();
        (
            e.path.clone(),
            icon_path(e),
            command_icon_path(e, &json!({"icon":"resources/extension-icon.png"})),
        )
    }
}
pub fn inspect(root: &Path) -> (PathBuf, Option<String>, Option<String>) {
    registry::inspect(root)
}
