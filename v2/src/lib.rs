pub mod codec;
pub mod media;
pub mod owned;
use base64::Engine;
use media::{Helpers, ResultData};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tezbar_extension_sdk::{
    view, BoxFuture, CommandConcurrency, CoreResult, Extension, ExtensionContext,
};
use tokio::sync::Semaphore;

const MAX_SESSIONS: usize = 8;
#[derive(Clone, Default)]
struct Session {
    epoch: u64,
    message: String,
    missing: bool,
    result: Option<ResultData>,
    prepared_copy: Option<String>,
    pending: bool,
}
#[derive(Clone)]
pub struct Adapter {
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    jobs: Arc<Semaphore>,
    helpers: Option<Option<Helpers>>,
}
impl Default for Adapter {
    fn default() -> Self {
        Self {
            sessions: Arc::default(),
            jobs: Arc::new(Semaphore::new(1)),
            helpers: None,
        }
    }
}
/// Dropped with the SDK's bounded session task, including failed/cancelled opens.
struct SessionOwner {
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    id: String,
}
impl Drop for SessionOwner {
    fn drop(&mut self) {
        self.sessions.lock().unwrap().remove(&self.id);
    }
}
/// SDK state is transactional; this checkpoint survives a dropped handler.
struct Checkpoint {
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    id: String,
    complete: bool,
}
impl Drop for Checkpoint {
    fn drop(&mut self) {
        if !self.complete {
            if let Some(s) = self.sessions.lock().unwrap().get_mut(&self.id) {
                s.pending = false;
                s.epoch += 1;
                s.message = if s.prepared_copy.is_some() {
                    "Clipboard write was not confirmed. Retry explicitly with the exact prepared bytes; it may already have succeeded."
                } else { "Generation interrupted. Use an explicit retry to acquire input again." }.into();
            }
        }
    }
}
fn action(epoch: u64, verb: &str, title: &str) -> Value {
    json!({"id":format!("{epoch}:{verb}"),"title":title,"kind":"action"})
}
fn acknowledged_open(value: &Value) -> bool {
    value.as_object().is_some_and(|object| {
        value["ok"] == true
            && object.keys().all(|key| key == "ok" || key == "message")
            && object.get("message").is_none_or(Value::is_string)
    })
}
fn install(platform: &str) -> (&'static str, &'static str) {
    if platform == "windows" || platform == "Windows" || platform == "win32" {
        (
            "winget install --id Gyan.FFmpeg",
            "https://www.gyan.dev/ffmpeg/builds/",
        )
    } else {
        (
            "brew install ffmpeg",
            "https://formulae.brew.sh/formula/ffmpeg",
        )
    }
}
fn render(s: &Session, platform: &str) -> Value {
    let e = s.epoch;
    let mut actions = Vec::new();
    let root = if s.pending {
        json!({"type":"Detail","props":{"markdown":"Working…"}})
    } else if s.prepared_copy.is_some() {
        actions.push(action(e, "retry-copy", "Retry Prepared Clipboard Write"));
        actions.push(action(e, "dismiss-copy", "Dismiss Unconfirmed Copy"));
        json!({"type":"Detail","props":{"markdown":s.message}})
    } else if s.missing {
        let (command, _) = install(platform);
        actions.push(action(e, "install-copy", "Copy Install Command"));
        actions.push(action(e, "instructions", "Open Instructions"));
        actions.push(action(e, "clipboard", "Retry from Clipboard"));
        json!({"type":"Detail","props":{"markdown":format!("# ffmpeg / ffprobe not found\n\nBoth native helpers are required. Install manually:\n\n```\n{command}\n```\n\n{}",s.message)}})
    } else if let Some(r) = &s.result {
        actions.push(action(e, "copy-hash", "Copy BlurHash"));
        actions.push(action(e, "copy-all", "Copy All"));
        let metadata = [
            ("File", r.file_name.clone()),
            ("Dimensions", format!("{} × {}", r.width, r.height)),
            ("Ratio", r.ratio.clone()),
            ("BlurHash", r.hash.clone()),
        ];
        json!({"type":"Detail","props":{"markdown":format!("![preview]({})",r.preview_data_uri)},"metadata":{"type":"Detail.Metadata","children":metadata.into_iter().enumerate().flat_map(|(i,(title,text))| { let label=json!({"type":"Detail.Metadata.Label","props":{"title":title,"text":text}}); if i==3 {vec![json!({"type":"Detail.Metadata.Separator"}),label]}else{vec![label]} }).collect::<Vec<_>>()}})
    } else {
        actions.push(action(e, "history", "Open Clipboard History"));
        actions.push(action(e, "selected", "Generate from Selected File"));
        actions.push(action(e, "clipboard", "Retry from Clipboard"));
        json!({"type":"List","props":{"searchBarPlaceholder":"BlurHash"},"children":[
            {"type":"List.EmptyView","props":{"title":"No image found in clipboard","description":s.message}},
            {"type":"List.Item","props":{"id":"history","title":"Open Clipboard History","subtitle":"Use Tezbar’s Clipboard History command; direct navigation is unavailable in this host API","actionIds":[format!("{e}:history")]}},
            {"type":"List.Item","props":{"id":"selected","title":"Use Selected File","subtitle":if s.message.is_empty(){"Read selected image or video"}else{&s.message},"actionIds":[format!("{e}:selected")]}},
            {"type":"List.Item","props":{"id":"clipboard","title":"Retry from Clipboard","actionIds":[format!("{e}:clipboard")]}}
        ]})
    };
    let mut v = view("BlurHash", root, actions);
    v["refreshMs"] = Value::Null;
    if !s.message.is_empty() {
        v["message"] = json!(s.message);
    }
    v
}
async fn selected(ctx: &ExtensionContext) -> Result<PathBuf, String> {
    let value = ctx
        .host("host.selection.read", json!({"kind":"files"}))
        .await
        .map_err(|_| "Could not read selected files")?;
    let paths = value
        .as_array()
        .filter(|p| p.len() <= 128)
        .ok_or("Invalid selected-file response")?;
    for path in paths {
        let value = path
            .as_str()
            .filter(|p| p.len() <= 32 * 1024)
            .ok_or("Invalid selected-file path")?;
        let p = PathBuf::from(value);
        if p.is_absolute() && p.is_file() && media::supported(&p) {
            return Ok(p);
        }
    }
    Err("Select a supported image or video file first.".into())
}
struct Acquired {
    path: PathBuf,
    _private: Option<owned::OwnedPng>,
}
async fn acquire(ctx: &ExtensionContext, selection_only: bool) -> Result<Acquired, String> {
    if !selection_only {
        // Image pixels correspond to V1's trusted clipboard file before its text path.
        if let Ok(image) = ctx
            .host("host.clipboard.read", json!({"format":"image"}))
            .await
        {
            if image["mimeType"] != "image/png" {
                return Err("Invalid clipboard image format".into());
            }
            let data = image["data"]
                .as_str()
                .filter(|s| s.len() <= 15 * 1024 * 1024)
                .ok_or("Invalid clipboard PNG response")?;
            let width = image["width"]
                .as_u64()
                .ok_or("Missing clipboard image width")?;
            let height = image["height"]
                .as_u64()
                .ok_or("Missing clipboard image height")?;
            if width == 0
                || height == 0
                || width > 16384
                || height > 16384
                || width * height * 4 > 64 * 1024 * 1024
            {
                return Err("Clipboard image exceeds host limits".into());
            }
            let png = base64::engine::general_purpose::STANDARD
                .decode(data)
                .map_err(|_| "Invalid clipboard PNG base64")?;
            if png.len() > 11 * 1024 * 1024 || !png.starts_with(b"\x89PNG\r\n\x1a\n") {
                return Err("Invalid or oversized clipboard PNG".into());
            }
            // Exclusive 0600 owned inode, scoped to this future. Never scans a temp directory.
            let file = owned::OwnedPng::create(&ctx.paths.temp_dir, &png)?;
            return Ok(Acquired {
                path: file.path().to_path_buf(),
                _private: Some(file),
            });
        }
        let text = ctx
            .host("host.clipboard.read", json!({}))
            .await
            .map_err(|_| "Could not read clipboard text")?;
        let text = text.as_str().ok_or("Invalid clipboard text response")?;
        if let Some(path) = media::resolve_text(text)? {
            return Ok(Acquired {
                path,
                _private: None,
            });
        }
    }
    Ok(Acquired {
        path: selected(ctx).await?,
        _private: None,
    })
}
impl Adapter {
    /// Explicit dependency seam for the executable test fixture; default production never accepts helper overrides from RPC/environment.
    pub fn with_helpers(helpers: Helpers) -> Self {
        Self {
            helpers: Some(Some(helpers)),
            ..Self::default()
        }
    }
    /// Explicit missing-dependency seam; not exposed through the wire protocol.
    pub fn without_helpers() -> Self {
        Self {
            helpers: Some(None),
            ..Self::default()
        }
    }
    fn snapshot(&self, id: &str) -> Result<Session, String> {
        self.sessions
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| "Session has been disposed".into())
    }
    fn change(&self, id: &str, f: impl FnOnce(&mut Session)) -> Result<(), String> {
        let mut all = self.sessions.lock().unwrap();
        let s = all.get_mut(id).ok_or("Session has been disposed")?;
        f(s);
        Ok(())
    }
    async fn generate(
        &self,
        ctx: &ExtensionContext,
        id: &str,
        selection_only: bool,
    ) -> Result<(), String> {
        self.change(id, |s| {
            s.pending = true;
            s.missing = false;
            s.result = None;
            s.message.clear();
        })?;
        let mut checkpoint = Checkpoint {
            sessions: self.sessions.clone(),
            id: id.into(),
            complete: false,
        };
        let result = tokio::time::timeout(std::time::Duration::from_secs(45), async {
            let _permit = self
                .jobs
                .clone()
                .try_acquire_owned()
                .map_err(|_| "Another media job is active; retry deliberately")?;
            let tools = match &self.helpers {
                Some(tools) => tools.clone(),
                None => media::helpers(ctx).await,
            };
            let Some(tools) = tools else {
                self.change(id, |s| s.missing = true)?;
                return Err("No tools were installed or downloaded.".to_string());
            };
            let input = acquire(ctx, selection_only).await?;
            let mut result = media::generate(ctx, &tools, &input.path).await?;
            if input._private.is_some() {
                result.file_name = "clipboard.png".into();
                result.copy_text = format!(
                    "clipboard.png {}x{} {} {}",
                    result.width, result.height, result.ratio, result.hash
                );
            }
            Ok(result)
        })
        .await
        .map_err(|_| "Generation exceeded the 45-second limit".to_string())
        .and_then(|value| value);
        self.change(id, |s| {
            s.pending = false;
            s.epoch += 1;
            match result {
                Ok(r) => {
                    s.result = Some(r);
                    s.message.clear();
                }
                Err(error) => s.message = error,
            }
        })?;
        checkpoint.complete = true;
        Ok(())
    }
    async fn copy(&self, ctx: &ExtensionContext, id: &str, text: String) -> Result<(), String> {
        self.change(id, |s| {
            s.prepared_copy = Some(text.clone());
            s.pending = true;
        })?;
        let mut checkpoint = Checkpoint {
            sessions: self.sessions.clone(),
            id: id.into(),
            complete: false,
        };
        let response = ctx.host("host.clipboard.write", json!({"text":text})).await;
        self.change(id, |s| {
            s.pending = false; s.epoch += 1;
            if response.as_ref().is_ok_and(|v| *v == json!({"ok":true})) {
                s.prepared_copy = None; s.message = "Copied".into();
            } else { s.message = "Clipboard write was not confirmed. Retry explicitly with the exact prepared bytes; it may already have succeeded.".into(); }
        })?;
        checkpoint.complete = true;
        Ok(())
    }
}
impl Extension for Adapter {
    fn id(&self) -> &'static str {
        "com.tezbar.native.blurhash"
    }
    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
    fn commands(&self) -> &'static [&'static str] {
        &["clipboard-blurhash"]
    }
    fn concurrency(&self, _: &str) -> CommandConcurrency {
        CommandConcurrency::RejectWhileBusy
    }
    fn handle<'a>(
        &'a self,
        ctx: &'a ExtensionContext,
        command: &'a str,
        operation: &'a str,
        payload: &'a Value,
        state: &'a mut Value,
    ) -> BoxFuture<'a, CoreResult> {
        Box::pin(async move {
            if command != "clipboard-blurhash" {
                return Err("Unknown command".into());
            }
            let id = payload["sessionId"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 256)
                .ok_or("Missing canonical session ID")?;
            if operation == "extension:dispose-session" {
                self.sessions.lock().unwrap().remove(id);
                return Ok(json!({}));
            }
            if operation == "extension:run-command" {
                {
                    let mut all = self.sessions.lock().unwrap();
                    if all.contains_key(id) {
                        return Err("Session already opened; no automatic replay".into());
                    }
                    if all.len() >= MAX_SESSIONS {
                        return Err("BlurHash session limit reached (8)".into());
                    }
                    all.insert(
                        id.into(),
                        Session {
                            epoch: 1,
                            ..Session::default()
                        },
                    );
                }
                let owner = SessionOwner {
                    sessions: self.sessions.clone(),
                    id: id.into(),
                };
                ctx.spawn(async move {
                    let _owner = owner;
                    std::future::pending::<()>().await;
                })?;
                self.generate(ctx, id, false).await?;
            } else if operation == "extension:invoke-action" {
                let s = self.snapshot(id)?;
                let a = payload["actionId"].as_str().ok_or("Missing action ID")?;
                let (epoch, verb) = a.split_once(':').ok_or("Invalid action ID")?;
                if epoch.parse::<u64>().ok() != Some(s.epoch) || s.pending {
                    return Err("Stale or pending action; refresh and retry deliberately".into());
                }
                // Validate against exactly the actions displayed in this authoritative checkpoint.
                if !render(&s, &ctx.platform)["actions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v["id"] == a)
                {
                    return Err("Action is not available in this view".into());
                }
                self.change(id, |s| s.epoch += 1)?;
                match verb {
                    "clipboard" => self.generate(ctx,id,false).await?,
                    "selected" => self.generate(ctx,id,true).await?,
                    "copy-hash" => self.copy(ctx,id,s.result.ok_or("Missing result")?.hash).await?,
                    "copy-all" => self.copy(ctx,id,s.result.ok_or("Missing result")?.copy_text).await?,
                    "retry-copy" => self.copy(ctx,id,s.prepared_copy.ok_or("No prepared copy")?).await?,
                    "install-copy" => self.copy(ctx,id,install(&ctx.platform).0.into()).await?,
                    "dismiss-copy" => self.change(id, |s| { s.prepared_copy = None; s.message = "Unconfirmed copy dismissed. No write was replayed.".into(); })?,
                    "instructions" => {
                        self.change(id,|s| s.message = "Instructions request pending; no automatic retry.".into())?;
                        let ack = ctx.host("host.shell.open",json!({"target":install(&ctx.platform).1})).await;
                        self.change(id,|s| s.message = if ack.as_ref().is_ok_and(acknowledged_open) { "Instructions opened" } else { "Open instructions was not confirmed; retry deliberately" }.into())?;
                    },
                    "history" => self.change(id,|s| s.message = "This host has no typed navigation service. Return to Tezbar and run Clipboard History, re-copy a file, then explicitly Retry from Clipboard.".into())?,
                    _ => return Err("Unknown action".into()),
                }
            } else if !matches!(
                operation,
                "extension:refresh-session"
                    | "extension:search-text-changed"
                    | "extension:load-more"
            ) {
                return Err("Unsupported operation".into());
            }
            let s = self.snapshot(id)?;
            *state = json!({"epoch":s.epoch});
            Ok(render(&s, &ctx.platform))
        })
    }
    fn shutdown<'a>(&'a self, _: &'a ExtensionContext) -> BoxFuture<'a, CoreResult> {
        Box::pin(async move {
            self.sessions.lock().unwrap().clear();
            Ok(json!({}))
        })
    }
}
