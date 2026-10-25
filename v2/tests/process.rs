#![cfg(feature = "qa-harness")]
use serde_json::{json, Value};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
};
fn wire_id(label: &str) -> String {
    format!(
        "h:{:032x}",
        label.bytes().fold(0u128, |v, b| v
            .wrapping_mul(257)
            .wrapping_add(u128::from(b)))
    )
}
struct Wire {
    child: Child,
    input: Option<ChildStdin>,
    output: Lines<BufReader<ChildStdout>>,
    _dir: tempfile::TempDir,
}
impl Wire {
    async fn start(grants: Value) -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".work");
        std::fs::create_dir_all(&root).unwrap();
        let dir = tempfile::tempdir_in(root).unwrap();
        let tool = env!("CARGO_BIN_EXE_blurhash-qa-fixture");
        let mut child = Command::new(tool)
            .args(["--serve", tool])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().unwrap()).lines();
        let mut w = Self {
            child,
            input,
            output,
            _dir: dir,
        };
        assert_eq!(w.read().await["type"], "hello");
        let dir = w._dir.path().to_str().unwrap().to_owned();
        w.send(json!({"type":"request","id":"h:init","method":"initialize","params":{"protocol":1,"extensionId":"com.tezbar.native.blurhash","platform":"macOS","packageDir":dir,"dataDir":dir,"tempDir":dir,"grantedCapabilities":grants,"reason":"test"}})).await;
        assert_eq!(w.response("h:init").await["result"]["ready"], true);
        w
    }
    async fn send(&mut self, mut v: Value) {
        for field in ["id", "requestId"] {
            if let Some(id) = v[field].as_str().filter(|s| s.starts_with("h:")) {
                v[field] = json!(wire_id(id));
            }
        }
        let mut b = serde_json::to_vec(&v).unwrap();
        b.push(b'\n');
        self.input.as_mut().unwrap().write_all(&b).await.unwrap();
    }
    async fn read(&mut self) -> Value {
        let line = tokio::time::timeout(Duration::from_secs(6), self.output.next_line())
            .await
            .unwrap()
            .unwrap()
            .expect("unexpected EOF");
        serde_json::from_str(&line).unwrap()
    }
    async fn response(&mut self, id: &str) -> Value {
        let v = self.read().await;
        assert!(
            v["type"] == "response" && v["id"] == wire_id(id),
            "unexpected frame {v}"
        );
        v
    }
    async fn open(&mut self, id: &str, sid: &str) {
        self.send(json!({"type":"request","id":id,"method":"session.open","params":{"sessionId":sid,"commandName":"clipboard-blurhash","preferences":{},"argumentValues":{}}})).await;
    }
    async fn reply(&mut self, r: &Value, result: Value) {
        self.send(json!({"type":"response","id":r["id"],"result":result}))
            .await;
    }
    async fn error(&mut self, r: &Value) {
        self.send(json!({"type":"response","id":r["id"],"error":{"code":"no-image","message":"No image"}})).await;
    }
    async fn call(&mut self, id: &str, sid: &str, op: &str, payload: Value) {
        self.send(json!({"type":"request","id":id,"method":"session.call","params":{"sessionId":sid,"operation":op,"payload":payload}})).await;
    }
    async fn stop(mut self) {
        self.send(json!({"type":"request","id":"h:stop","method":"shutdown","params":{}}))
            .await;
        let _ = self.response("h:stop").await;
        assert!(
            tokio::time::timeout(Duration::from_secs(5), self.child.wait())
                .await
                .unwrap()
                .unwrap()
                .success()
        );
    }
}
fn actions(v: &Value, title: &str) -> String {
    v["result"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["title"] == title)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn caps() -> Value {
    json!([
        "clipboard.read",
        "clipboard.write",
        "selection.read",
        "shell.open"
    ])
}
#[tokio::test]
async fn real_jsonl_adapter_flow_late_ack_no_replay_and_dispose() {
    let mut w = Wire::start(caps()).await;
    let path = w._dir.path().join("image.png");
    std::fs::write(&path, b"normal").unwrap();
    w.open("h:open", "s").await;
    let image = w.read().await;
    assert_eq!(image["params"], json!({"format":"image"}));
    w.error(&image).await;
    let text = w.read().await;
    assert_eq!(text["params"], json!({}));
    w.reply(&text, json!(path)).await;
    let result = w.response("h:open").await;
    assert_eq!(result["result"]["root"]["type"], "Detail");
    let action = actions(&result, "Copy All");
    w.call(
        "h:copy",
        "s",
        "extension:invoke-action",
        json!({"actionId":action}),
    )
    .await;
    let write = w.read().await;
    assert_eq!(write["method"], "host.clipboard.write");
    let exact = write["params"]["text"].clone();
    w.send(json!({"type":"cancel","requestId":"h:copy","reason":"test"}))
        .await;
    // Reader cancellation may emit reverse cancel and host-request error in either order.
    let mut got_error = false;
    let mut got_reverse_cancel = false;
    for _ in 0..2 {
        let frame = w.read().await;
        if frame["type"] == "response" && frame["id"] == wire_id("h:copy") {
            assert!(frame.get("error").is_some());
            got_error = true;
        } else if frame["type"] == "cancel" && frame["requestId"] == write["id"] {
            got_reverse_cancel = true;
        } else {
            panic!("unexpected cancellation frame {frame}");
        }
    }
    assert!(got_error && got_reverse_cancel);
    w.reply(&write, json!({"ok":true})).await; // genuine late acknowledgement must never trigger replay
    w.call("h:refresh", "s", "extension:refresh-session", json!({}))
        .await;
    let refresh = w.response("h:refresh").await;
    w.call(
        "h:retry",
        "s",
        "extension:invoke-action",
        json!({"actionId":actions(&refresh,"Retry Prepared Clipboard Write")}),
    )
    .await;
    let retry = w.read().await;
    assert_eq!(retry["params"]["text"], exact);
    w.reply(&retry, json!({"ok":true})).await;
    let copied = w.response("h:retry").await;
    assert_eq!(copied["result"]["message"], "Copied");
    w.call(
        "h:stale",
        "s",
        "extension:invoke-action",
        json!({"actionId":action}),
    )
    .await;
    assert!(w.response("h:stale").await.get("error").is_some());
    w.send(json!({"type":"request","id":"h:dispose","method":"session.dispose","params":{"sessionId":"s"}})).await;
    let _ = w.response("h:dispose").await;
    w.call("h:gone", "s", "extension:refresh-session", json!({}))
        .await;
    assert!(w.response("h:gone").await.get("error").is_some());
    w.stop().await;
}
#[tokio::test]
async fn real_pipes_denied_capabilities_search_refresh_and_eof() {
    let mut w = Wire::start(json!([])).await;
    w.open("h:open", "s").await;
    let view = w.response("h:open").await;
    assert!(view["result"]["message"]
        .as_str()
        .unwrap()
        .contains("clipboard"));
    for (id, op) in [
        ("h:search", "extension:search-text-changed"),
        ("h:refresh", "extension:refresh-session"),
    ] {
        w.call(id, "s", op, json!({"text":"irrelevant"})).await;
        assert_eq!(w.response(id).await["result"]["root"]["type"], "List");
    }
    drop(w.input.take());
    assert!(tokio::time::timeout(Duration::from_secs(5), w.child.wait())
        .await
        .unwrap()
        .unwrap()
        .success());
}
#[tokio::test]
async fn malformed_framing_exits_and_pending_reverse_eof_exits() {
    let mut w = Wire::start(caps()).await;
    w.input
        .as_mut()
        .unwrap()
        .write_all(b"not-json\n")
        .await
        .unwrap();
    assert!(
        !tokio::time::timeout(Duration::from_secs(5), w.child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
    let mut w = Wire::start(caps()).await;
    w.open("h:open", "s").await;
    let reverse = w.read().await;
    assert_eq!(reverse["method"], "host.clipboard.read");
    drop(w.input.take());
    assert!(tokio::time::timeout(Duration::from_secs(5), w.child.wait())
        .await
        .unwrap()
        .unwrap()
        .success());
}
#[cfg(unix)]
#[tokio::test]
async fn real_pipes_eof_during_helper_stops_tree_and_private_lifetime() {
    let mut w = Wire::start(caps()).await;
    let path = w._dir.path().join("slow.png");
    std::fs::write(&path, b"slow").unwrap();
    w.open("h:open", "s").await;
    let r = w.read().await;
    w.error(&r).await;
    let r = w.read().await;
    w.reply(&r, json!(path)).await;
    let pids = path.with_extension("pids");
    for _ in 0..300 {
        if std::fs::read_to_string(&pids).is_ok_and(|s| s.lines().count() == 2) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let pids = std::fs::read_to_string(pids).unwrap();
    assert_eq!(pids.lines().count(), 2);
    drop(w.input.take());
    let _ = tokio::time::timeout(Duration::from_secs(5), w.child.wait())
        .await
        .unwrap()
        .unwrap();
    for pid in pids.lines() {
        let p = pid.parse::<i32>().unwrap();
        for _ in 0..100 {
            if unsafe { libc::kill(p, 0) } != 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_ne!(unsafe { libc::kill(p, 0) }, 0, "helper survived EOF");
    }
}
#[test]
fn actual_production_executable_probes() {
    for flag in ["--protocol-info", "--self-test"] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_tezbar-blurhash"))
            .arg(flag)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(serde_json::from_slice::<Value>(&output.stdout).is_ok());
    }
}

#[cfg(unix)]
#[tokio::test]
async fn explicit_cancel_dispose_and_shutdown_stop_owned_helper_trees() {
    for mode in ["cancel", "dispose", "shutdown"] {
        let mut w = Wire::start(caps()).await;
        let path = w._dir.path().join("slow.png");
        std::fs::write(&path, b"slow").unwrap();
        w.open("h:open", "s").await;
        let r = w.read().await;
        w.error(&r).await;
        let r = w.read().await;
        w.reply(&r, json!("")).await;
        let r = w.read().await;
        w.reply(&r, json!([])).await;
        let fallback = w.response("h:open").await;
        w.call(
            "h:work",
            "s",
            "extension:invoke-action",
            json!({"actionId":actions(&fallback,"Generate from Selected File")}),
        )
        .await;
        let r = w.read().await;
        assert_eq!(r["method"], "host.selection.read");
        w.reply(&r, json!([path])).await;
        let pidfile = path.with_extension("pids");
        for _ in 0..300 {
            if std::fs::read_to_string(&pidfile).is_ok_and(|s| s.lines().count() == 2) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let pids = std::fs::read_to_string(&pidfile).unwrap();
        assert_eq!(pids.lines().count(), 2);
        match mode {
            "cancel"=>w.send(json!({"type":"cancel","requestId":"h:work","reason":"deliberate cancel"})).await,
            "dispose"=>w.send(json!({"type":"request","id":"h:dispose","method":"session.dispose","params":{"sessionId":"s"}})).await,
            _=>w.send(json!({"type":"request","id":"h:stop","method":"shutdown","params":{}})).await,
        }
        // The cancelled open reports an error. Control request response may arrive before it.
        let count = if mode == "cancel" { 1 } else { 2 };
        let mut open_error = false;
        for _ in 0..count {
            let r = w.read().await;
            if r["id"] == wire_id("h:work") {
                assert!(r.get("error").is_some());
                open_error = true;
            }
        }
        assert!(open_error);
        for pid in pids.lines() {
            let pid = pid.parse::<i32>().unwrap();
            for _ in 0..100 {
                if unsafe { libc::kill(pid, 0) } != 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            assert_ne!(unsafe { libc::kill(pid, 0) }, 0, "{mode}: helper survived");
        }
        if mode == "cancel" {
            w.call("h:refresh", "s", "extension:refresh-session", json!({}))
                .await;
            let v = w.response("h:refresh").await;
            assert!(v["result"]["message"]
                .as_str()
                .unwrap()
                .contains("interrupted"));
            let normal = w._dir.path().join("normal.png");
            std::fs::write(&normal, b"normal").unwrap();
            w.call(
                "h:retry",
                "s",
                "extension:invoke-action",
                json!({"actionId":actions(&v,"Generate from Selected File")}),
            )
            .await;
            let r = w.read().await;
            assert_eq!(r["method"], "host.selection.read");
            w.reply(&r, json!([normal])).await;
            assert_eq!(
                w.response("h:retry").await["result"]["root"]["type"],
                "Detail"
            );
        }
        if mode == "shutdown" {
            assert!(tokio::time::timeout(Duration::from_secs(5), w.child.wait())
                .await
                .unwrap()
                .unwrap()
                .success());
        } else {
            w.stop().await;
        }
    }
}

#[tokio::test]
async fn cr_and_truncated_frames_rejected() {
    for bytes in [&b"{}\r\n"[..], &b"{\"type\":\"request\"}"[..]] {
        let mut w = Wire::start(caps()).await;
        w.input.as_mut().unwrap().write_all(bytes).await.unwrap();
        drop(w.input.take());
        assert!(
            !tokio::time::timeout(Duration::from_secs(5), w.child.wait())
                .await
                .unwrap()
                .unwrap()
                .success()
        );
    }
}

#[tokio::test]
async fn cancelled_opens_release_authoritative_session_slots_without_replay() {
    let mut w = Wire::start(caps()).await;
    for n in 0..12 {
        let request = format!("h:open{n}");
        let session = format!("cancelled{n}");
        w.open(&request, &session).await;
        let image = w.read().await;
        assert_eq!(image["method"], "host.clipboard.read");
        w.send(json!({"type":"cancel","requestId":request,"reason":"cancel initial acquisition"}))
            .await;
        let mut replied = false;
        for _ in 0..2 {
            let r = w.read().await;
            if r["type"] == "response" {
                assert!(r.get("error").is_some());
                replied = true;
            } else {
                assert_eq!(r["type"], "cancel");
            }
        }
        assert!(replied);
        w.reply(&image, json!({"mimeType":"image/png","data":"late"}))
            .await;
    }
    w.open("h:normal", "normal").await;
    let r = w.read().await;
    w.error(&r).await;
    let r = w.read().await;
    w.reply(&r, json!("")).await;
    let r = w.read().await;
    w.reply(&r, json!([])).await;
    assert_eq!(
        w.response("h:normal").await["result"]["root"]["type"],
        "List"
    );
    w.stop().await;
}
