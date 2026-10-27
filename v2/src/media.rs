use crate::codec;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tezbar_extension_sdk::ExtensionContext;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};

pub const FILE_BYTES: u64 = 512 * 1024 * 1024;
pub const HELPER_TIMEOUT: Duration = Duration::from_secs(15);
#[derive(Clone, Debug)]
pub struct Helpers {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResultData {
    pub hash: String,
    pub width: u32,
    pub height: u32,
    pub ratio: String,
    pub file_name: String,
    pub copy_text: String,
    pub preview_data_uri: String,
}

/// A process group/job belongs to this invocation, including its descendants.
struct Tree {
    #[cfg(unix)]
    pid: i32,
    #[cfg(windows)]
    job: windows_sys::Win32::Foundation::HANDLE,
}
// A job handle is exclusively owned and only used for close on drop.
#[cfg(windows)]
unsafe impl Send for Tree {}
impl Tree {
    fn attach(child: &tokio::process::Child) -> Result<Self, String> {
        #[cfg(unix)]
        {
            Ok(Self {
                pid: child.id().ok_or("Helper exited during spawn")? as i32,
            })
        }
        #[cfg(windows)]
        {
            use windows_sys::Win32::{Foundation::CloseHandle, System::JobObjects::*};
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return Err("Cannot create helper job".into());
                }
                let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let process = child.raw_handle().ok_or("Helper handle missing")?;
                if SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &limits as *const _ as *const _,
                    std::mem::size_of_val(&limits) as u32,
                ) == 0
                    || AssignProcessToJobObject(job, process as _) == 0
                {
                    CloseHandle(job);
                    return Err("Cannot contain helper job".into());
                }
                Ok(Self { job })
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = child;
            Err("Unsupported helper platform".into())
        }
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::kill(-self.pid, libc::SIGKILL);
        }
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.job);
        }
    }
}
async fn bounded(reader: impl AsyncRead + Unpin, cap: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .take(cap as u64 + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| "Cannot read helper output")?;
    if bytes.len() > cap {
        return Err("Helper output exceeds limit".into());
    }
    Ok(bytes)
}
/// No shell, inherited input or output; both pipes are drained concurrently.
pub async fn run(
    ctx: &ExtensionContext,
    executable: &Path,
    args: &[OsString],
    cap: usize,
    deadline: Duration,
) -> Result<Vec<u8>, String> {
    if !executable.is_absolute() || !executable.is_file() {
        return Err("Helper is not an absolute regular file".into());
    }
    let mut command = Command::new(executable);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut child = command.spawn().map_err(|_| "Cannot start media helper")?;
    let _tree = Tree::attach(&child)?;
    let stdout = child.stdout.take().ok_or("Helper stdout missing")?;
    let stderr = child.stderr.take().ok_or("Helper stderr missing")?;
    let work = async {
        let (status, out, _) = tokio::try_join!(
            async {
                child
                    .wait()
                    .await
                    .map_err(|_| "Cannot wait for helper".to_string())
            },
            bounded(stdout, cap),
            bounded(stderr, 64 * 1024)
        )?;
        if !status.success() {
            return Err("Media helper rejected the input (decoder/format dependent)".into());
        }
        Ok(out)
    };
    tokio::select! {
        biased;
        _ = ctx.cancelled() => Err("Operation cancelled".into()),
        result = tokio::time::timeout(deadline, work) => result.map_err(|_| "Media helper timed out".to_string())?,
    }
}
fn executable(path: &Path) -> Option<PathBuf> {
    let canonical = path.canonicalize().ok()?;
    let meta = canonical.metadata().ok()?;
    if !meta.is_file() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }
    Some(canonical)
}
async fn locate(ctx: &ExtensionContext, name: &str) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    for directory in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
        if let Some(path) = executable(&Path::new(directory).join(name)) {
            return Some(path);
        }
    }
    #[cfg(windows)]
    let (locator, target) = {
        let root = std::env::var_os("SystemRoot")?;
        (
            PathBuf::from(root).join("System32/where.exe"),
            format!("{name}.exe"),
        )
    };
    #[cfg(not(windows))]
    let (locator, target) = (PathBuf::from("/usr/bin/which"), name.to_string());
    let bytes = run(
        ctx,
        &locator,
        &[target.into()],
        64 * 1024,
        Duration::from_secs(3),
    )
    .await
    .ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    text.lines()
        .find(|line| !line.is_empty())
        .and_then(|line| executable(Path::new(line)))
}
pub async fn helpers(ctx: &ExtensionContext) -> Option<Helpers> {
    // Evaluate BOTH; launch dependency validation precedes clipboard acquisition.
    let ffmpeg = locate(ctx, "ffmpeg").await;
    let ffprobe = locate(ctx, "ffprobe").await;
    Some(Helpers {
        ffmpeg: ffmpeg?,
        ffprobe: ffprobe?,
    })
}
pub fn supported(path: &Path) -> bool {
    path.extension().and_then(|x| x.to_str()).is_some_and(|x| {
        matches!(
            x.to_ascii_lowercase().as_str(),
            "mp4"
                | "mov"
                | "avi"
                | "mkv"
                | "webm"
                | "m4v"
                | "flv"
                | "wmv"
                | "3gp"
                | "ts"
                | "jpg"
                | "jpeg"
                | "png"
                | "gif"
                | "bmp"
                | "webp"
                | "tiff"
                | "tif"
                | "heic"
                | "heif"
                | "avif"
                | "ico"
        )
    })
}
/// JavaScript decodeURIComponent: malformed percent escapes/UTF-8 fail, plus stays plus.
pub fn decode_uri(value: &str) -> Result<String, String> {
    let mut out = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let a = bytes
                .get(i + 1)
                .and_then(|b| (*b as char).to_digit(16))
                .ok_or("Malformed percent escape in clipboard path")?;
            let b = bytes
                .get(i + 2)
                .and_then(|b| (*b as char).to_digit(16))
                .ok_or("Malformed percent escape in clipboard path")?;
            out.push((a * 16 + b) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| "Invalid UTF-8 in clipboard path".into())
}
fn js_whitespace(c: char) -> bool {
    matches!(c, '\u{0009}'..='\u{000d}'|'\u{0020}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}')
}
pub fn resolve_text(value: &str) -> Result<Option<PathBuf>, String> {
    if value.len() > 32 * 1024 {
        return Err("Clipboard path exceeds limit".into());
    }
    let trimmed = value.trim_matches(js_whitespace);
    let text = trimmed.strip_prefix("file://").unwrap_or(trimmed);
    let decoded = decode_uri(text)?;
    for candidate in [&decoded, text] {
        let p = Path::new(candidate);
        if p.is_absolute() && p.is_file() && supported(p) {
            return Ok(Some(p.to_path_buf()));
        }
    }
    Ok(None)
}
pub fn dimensions(value: &Value) -> Result<(u32, u32), String> {
    let streams = value["streams"]
        .as_array()
        .ok_or("Could not read dimensions")?;
    let s = streams
        .iter()
        .find(|s| s["width"].as_u64().unwrap_or(0) != 0 && s["height"].as_u64().unwrap_or(0) != 0)
        .ok_or("Could not read dimensions")?;
    let w = s["width"].as_u64().ok_or("Invalid width")?;
    let h = s["height"].as_u64().ok_or("Invalid height")?;
    if w > 16384 || h > 16384 || w * h > 64_000_000 {
        return Err("Media dimensions exceed limit".into());
    }
    Ok((w as u32, h as u32))
}
pub fn sizes(w: u32, h: u32) -> Result<(u32, u32, u32), String> {
    if w == 0 || h == 0 {
        return Err("Zero media dimensions".into());
    }
    let scale = (100. / f64::from(w)).min(100. / f64::from(h)).min(1.);
    let sw = (f64::from(w) * scale + 0.5).floor() as u32;
    let sh = (f64::from(h) * scale + 0.5).floor() as u32;
    let ph = (f64::from(h) / f64::from(w) * 400. + 0.5).floor() as u32;
    if sw == 0 || sh == 0 {
        return Err("Hash resize rounded a dimension to zero".into());
    }
    if ph == 0 || ph > 1600 {
        return Err("Preview aspect ratio exceeds limit (height 1..1600)".into());
    }
    Ok((sw, sh, ph))
}
pub fn ratio(mut w: u32, mut h: u32) -> u32 {
    while h != 0 {
        let next = w % h;
        w = h;
        h = next;
    }
    w
}
pub async fn generate(
    ctx: &ExtensionContext,
    tools: &Helpers,
    path: &Path,
) -> Result<ResultData, String> {
    let display_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Input filename is not UTF-8")?
        .to_owned();
    let path = path
        .canonicalize()
        .map_err(|_| "Input file is unavailable")?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000);
    }
    let file = options.open(&path).map_err(|_| "Cannot read input file")?;
    let meta = file.metadata().map_err(|_| "Cannot inspect input file")?;
    if !meta.is_file() || meta.len() > FILE_BYTES {
        return Err("Input must be a regular file of at most 512 MiB".into());
    }
    let mut args: Vec<OsString> = [
        "-v",
        "quiet",
        "-max_alloc",
        "67108864",
        "-protocol_whitelist",
        "file,pipe",
        "-print_format",
        "json",
        "-show_streams",
        "-i",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    args.push(path.as_os_str().into());
    let bytes = run(ctx, &tools.ffprobe, &args, 1024 * 1024, HELPER_TIMEOUT).await?;
    let (width, height) =
        dimensions(&serde_json::from_slice(&bytes).map_err(|_| "Invalid ffprobe JSON")?)?;
    let (sw, sh, ph) = sizes(width, height)?;
    let mut args: Vec<OsString> = [
        "-v",
        "error",
        "-nostdin",
        "-max_alloc",
        "67108864",
        "-threads",
        "1",
        "-filter_threads",
        "1",
        "-protocol_whitelist",
        "file,pipe",
        "-i",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    args.push(path.as_os_str().into());
    args.extend([
        "-vframes".into(),
        "1".into(),
        "-s".into(),
        format!("{sw}x{sh}").into(),
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        "rgba".into(),
        "pipe:1".into(),
    ]);
    let pixels = run(
        ctx,
        &tools.ffmpeg,
        &args,
        (sw * sh * 4) as usize,
        HELPER_TIMEOUT,
    )
    .await?;
    let hash = codec::encode(&pixels, sw, sh)?;
    tokio::select! { biased; _ = ctx.cancelled() => return Err("Operation cancelled".into()), _ = tokio::task::yield_now() => {} }
    let preview = codec::decode(&hash, 400, ph)?;
    tokio::select! { biased; _ = ctx.cancelled() => return Err("Operation cancelled".into()), _ = tokio::task::yield_now() => {} }
    let png = codec::png(&preview, 400, ph)?;
    let file_name = display_name;
    if file_name.len() > 4096 {
        return Err("Input filename exceeds limit".into());
    }
    let d = ratio(width, height);
    let ratio = format!("{}:{}", width / d, height / d);
    let copy_text = format!("{file_name} {width}x{height} {ratio} {hash}");
    Ok(ResultData {
        hash,
        width,
        height,
        ratio,
        file_name,
        copy_text,
        preview_data_uri: format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        ),
    })
}
