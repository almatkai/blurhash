#![cfg(feature = "qa-harness")]
use base64::Engine;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tezbar_blurhash::{media::Helpers, Adapter};
use tezbar_extension_sdk::{Extension, ExtensionPaths, TestContext};
fn private() -> tempfile::TempDir {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(".work");
    std::fs::create_dir_all(&p).unwrap();
    tempfile::tempdir_in(p).unwrap()
}
fn tool() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_blurhash-qa-fixture"))
}
fn adapter() -> Adapter {
    Adapter::with_helpers(Helpers {
        ffmpeg: tool(),
        ffprobe: tool(),
    })
}
fn context(dir: &Path, grants: &[&str]) -> TestContext {
    TestContext::new(
        ExtensionPaths {
            package_dir: dir.into(),
            data_dir: dir.into(),
            temp_dir: dir.into(),
        },
        grants,
    )
}
const CAPS: &[&str] = &[
    "clipboard.read",
    "clipboard.write",
    "selection.read",
    "shell.open",
];
async fn handle(a: &Adapter, c: &TestContext, op: &str, p: Value) -> Value {
    c.run(a.handle(&c.context, "clipboard-blurhash", op, &p, &mut json!({})))
        .await
        .unwrap()
}
fn id(v: &Value, title: &str) -> String {
    v["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["title"] == title)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .into()
}
fn reply_input(c: &TestContext, path: &Path) {
    c.reply(Err("Clipboard contains no readable image".into()));
    c.reply(Ok(json!(path.to_str().unwrap())));
}
#[tokio::test]
async fn real_adapter_host_contracts_copy_ack_uncertainty_and_stale_guard() {
    let d = private();
    let p = d.path().join("image.PNG");
    std::fs::write(&p, b"normal").unwrap();
    let c = context(d.path(), CAPS);
    let a = adapter();
    reply_input(&c, &p);
    let v = handle(&a, &c, "extension:run-command", json!({"sessionId":"a"})).await;
    assert!(v["root"]["props"]["markdown"]
        .as_str()
        .unwrap()
        .starts_with("![preview](data:image/png;base64,"));
    assert_eq!(v["refreshMs"], Value::Null);
    assert_eq!(
        v["root"]["metadata"]["children"][0]["props"]["text"],
        "image.PNG"
    );
    let calls = c.calls();
    assert_eq!(calls[0]["method"], "host.clipboard.read");
    assert_eq!(calls[0]["params"], json!({"format":"image"}));
    assert_eq!(calls[1]["params"], json!({}));
    let old = id(&v, "Copy All");
    c.reply(Ok(json!(true))); // positive bool is NOT actual structured ack
    let uncertain = handle(
        &a,
        &c,
        "extension:invoke-action",
        json!({"sessionId":"a","actionId":old}),
    )
    .await;
    let copy = c.calls().last().unwrap()["params"]["text"].clone();
    assert!(copy.as_str().unwrap().starts_with("image.PNG 8x6 4:3 "));
    assert!(uncertain["message"]
        .as_str()
        .unwrap()
        .contains("not confirmed"));
    let count = c.calls().len();
    let refreshed = handle(
        &a,
        &c,
        "extension:refresh-session",
        json!({"sessionId":"a"}),
    )
    .await;
    assert_eq!(c.calls().len(), count);
    assert!(c
        .run(a.handle(
            &c.context,
            "clipboard-blurhash",
            "extension:invoke-action",
            &json!({"sessionId":"a","actionId":old}),
            &mut json!({})
        ))
        .await
        .is_err());
    c.reply(Ok(json!({"ok":true})));
    let copied = handle(
        &a,
        &c,
        "extension:invoke-action",
        json!({"sessionId":"a","actionId":id(&refreshed,"Retry Prepared Clipboard Write")}),
    )
    .await;
    assert_eq!(copied["message"], "Copied");
    assert_eq!(c.calls().last().unwrap()["params"]["text"], copy);
    assert_eq!(c.calls().len(), count + 1);
}
#[tokio::test]
async fn fallback_selected_failure_deliberate_retry_no_payload_path() {
    let d = private();
    let c = context(d.path(), CAPS);
    let a = adapter();
    c.reply(Err("No image".into()));
    c.reply(Ok(json!("")));
    c.reply(Ok(json!([])));
    let v = handle(&a, &c, "extension:run-command", json!({"sessionId":"a"})).await;
    assert_eq!(v["root"]["type"], "List");
    c.reply(Ok(json!([])));
    let failure=handle(&a,&c,"extension:invoke-action",json!({"sessionId":"a","actionId":id(&v,"Generate from Selected File"),"path":"/arbitrary/ignored.png"})).await;
    assert!(failure["message"]
        .as_str()
        .unwrap()
        .contains("Select a supported"));
    let p = d.path().join("selected.png");
    std::fs::write(&p, b"normal").unwrap();
    c.reply(Ok(json!([p])));
    let success = handle(
        &a,
        &c,
        "extension:invoke-action",
        json!({"sessionId":"a","actionId":id(&failure,"Generate from Selected File")}),
    )
    .await;
    assert_eq!(success["root"]["type"], "Detail");
    assert_eq!(
        c.calls()
            .iter()
            .filter(|v| v["method"] == "host.clipboard.read")
            .count(),
        2
    );
}
#[tokio::test]
async fn clipboard_pixels_private_png_bounded_lifetime_and_no_copied_path() {
    let d = private();
    let c = context(d.path(), CAPS);
    let a = adapter();
    let png = tezbar_blurhash::codec::png(&[0, 128, 255, 255], 1, 1).unwrap();
    c.reply(Ok(json!({"data":base64::engine::general_purpose::STANDARD.encode(png),"mimeType":"image/png","width":1,"height":1})));
    let v = handle(&a, &c, "extension:run-command", json!({"sessionId":"a"})).await;
    assert_eq!(
        v["root"]["metadata"]["children"][0]["props"]["text"],
        "clipboard.png"
    );
    assert_eq!(c.calls().len(), 1);
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 0);
}
#[tokio::test]
async fn dropped_copy_checkpoint_survives_sdk_state_rollback() {
    let d = private();
    let p = d.path().join("a.png");
    std::fs::write(&p, b"normal").unwrap();
    let c = context(d.path(), CAPS);
    let a = adapter();
    reply_input(&c, &p);
    let v = handle(&a, &c, "extension:run-command", json!({"sessionId":"a"})).await;
    let payload = json!({"sessionId":"a","actionId":id(&v,"Copy BlurHash")});
    let mut state = json!({});
    let mut future = a.handle(
        &c.context,
        "clipboard-blurhash",
        "extension:invoke-action",
        &payload,
        &mut state,
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(20), &mut future)
            .await
            .is_err()
    );
    drop(future);
    let count = c.calls().len();
    let v = handle(
        &a,
        &c,
        "extension:refresh-session",
        json!({"sessionId":"a"}),
    )
    .await;
    assert_eq!(c.calls().len(), count);
    assert!(id(&v, "Retry Prepared Clipboard Write").contains(':'));
}
#[tokio::test]
async fn host_capability_denial_and_session_bound() {
    let d = private();
    let c = context(d.path(), &[]);
    let a = adapter();
    for n in 0..8 {
        let v = handle(
            &a,
            &c,
            "extension:run-command",
            json!({"sessionId":format!("s{n}")}),
        )
        .await;
        assert!(v["message"].as_str().unwrap().contains("clipboard"));
    }
    assert!(c
        .run(a.handle(
            &c.context,
            "clipboard-blurhash",
            "extension:run-command",
            &json!({"sessionId":"overflow"}),
            &mut json!({})
        ))
        .await
        .is_err());
    assert!(c.calls().is_empty());
    handle(
        &a,
        &c,
        "extension:dispose-session",
        json!({"sessionId":"s0"}),
    )
    .await;
    handle(&a, &c, "extension:run-command", json!({"sessionId":"new"})).await;
}
#[tokio::test]
async fn helper_failure_and_output_limit_are_visible_and_recoverable() {
    let d = private();
    let c = context(d.path(), CAPS);
    let a = adapter();
    let p = d.path().join("a.png");
    std::fs::write(&p, b"bad").unwrap();
    reply_input(&c, &p);
    let v = handle(&a, &c, "extension:run-command", json!({"sessionId":"a"})).await;
    assert!(v["message"].as_str().unwrap().contains("helper rejected"));
    std::fs::write(&p, b"overflow").unwrap();
    reply_input(&c, &p);
    let v = handle(
        &a,
        &c,
        "extension:invoke-action",
        json!({"sessionId":"a","actionId":id(&v,"Retry from Clipboard")}),
    )
    .await;
    assert!(v["message"].as_str().unwrap().contains("exceeds limit"));
}
#[cfg(unix)]
#[tokio::test]
async fn helper_timeout_and_drop_kill_owned_descendants() {
    use std::time::Duration;
    let d = private();
    let c = context(d.path(), CAPS);
    let p = d.path().join("slow.png");
    std::fs::write(&p, b"slow").unwrap();
    let args = vec!["-i".into(), p.as_os_str().into()];
    let result =
        tezbar_blurhash::media::run(&c.context, &tool(), &args, 1024, Duration::from_secs(2)).await;
    assert!(result.unwrap_err().contains("timed out"));
    let pidfile = p.with_extension("pids");
    let pids = std::fs::read_to_string(&pidfile).unwrap();
    assert_eq!(pids.lines().count(), 2);
    for pid in pids.lines() {
        let pid = pid.parse::<i32>().unwrap();
        for _ in 0..50 {
            if unsafe { libc::kill(pid, 0) } != 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_ne!(
            unsafe { libc::kill(pid, 0) },
            0,
            "owned helper {pid} survived"
        );
    }
}

#[tokio::test]
async fn missing_dependencies_install_copy_and_instructions_exact_contracts() {
    for (platform, command, url) in [
        (
            "macOS",
            "brew install ffmpeg",
            "https://formulae.brew.sh/formula/ffmpeg",
        ),
        (
            "Windows",
            "winget install --id Gyan.FFmpeg",
            "https://www.gyan.dev/ffmpeg/builds/",
        ),
    ] {
        let d = private();
        let mut c = context(d.path(), CAPS);
        c.context.platform = platform.into();
        let a = Adapter::without_helpers();
        let v = handle(&a, &c, "extension:run-command", json!({"sessionId":"a"})).await;
        assert!(c.calls().is_empty());
        assert!(v["root"]["props"]["markdown"]
            .as_str()
            .unwrap()
            .contains(command));
        c.reply(Ok(json!({"ok":true})));
        let v = handle(
            &a,
            &c,
            "extension:invoke-action",
            json!({"sessionId":"a","actionId":id(&v,"Copy Install Command")}),
        )
        .await;
        assert_eq!(c.calls()[0]["params"], json!({"text":command}));
        c.reply(Ok(json!({"ok":true,"message":"Opened"})));
        let mut opened = handle(
            &a,
            &c,
            "extension:invoke-action",
            json!({"sessionId":"a","actionId":id(&v,"Open Instructions")}),
        )
        .await;
        assert_eq!(c.calls()[1]["method"], "host.shell.open");
        assert_eq!(c.calls()[1]["params"], json!({"target":url}));
        assert_eq!(opened["message"], "Instructions opened");
        for response in [
            Value::Null,
            json!(true),
            json!({"ok":true,"error":"denied"}),
            json!({"ok":true,"success":false}),
            json!({"ok":true,"message":false}),
        ] {
            c.reply(Ok(response));
            opened = handle(
                &a,
                &c,
                "extension:invoke-action",
                json!({"sessionId":"a","actionId":id(&opened,"Open Instructions")}),
            )
            .await;
            assert_eq!(
                opened["message"],
                "Open instructions was not confirmed; retry deliberately"
            );
            assert_eq!(c.calls().last().unwrap()["method"], "host.shell.open");
            assert_eq!(c.calls().last().unwrap()["params"], json!({"target":url}));
        }
    }
}

#[tokio::test]
async fn helper_stderr_and_sparse_input_bytes_caps_fail_without_truncation() {
    let d = private();
    let c = context(d.path(), CAPS);
    let tools = Helpers {
        ffmpeg: tool(),
        ffprobe: tool(),
    };
    let p = d.path().join("stderr.png");
    std::fs::write(&p, b"stderr-overflow").unwrap();
    assert!(tezbar_blurhash::media::generate(&c.context, &tools, &p)
        .await
        .unwrap_err()
        .contains("output exceeds limit"));
    let p = d.path().join("sparse.png");
    let file = std::fs::File::create(&p).unwrap();
    file.set_len(tezbar_blurhash::media::FILE_BYTES + 1)
        .unwrap();
    assert!(tezbar_blurhash::media::generate(&c.context, &tools, &p)
        .await
        .unwrap_err()
        .contains("512 MiB"));
}
