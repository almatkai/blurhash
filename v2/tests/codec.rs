use base64::Engine;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tezbar_blurhash::{codec, media};
fn oracle() -> Value {
    serde_json::from_str(include_str!("../fixtures/oracle.json")).unwrap()
}
#[test]
fn exact_locked_typescript_library_vectors_and_png_roundtrip() {
    let o = oracle();
    for v in o["vectors"].as_array().unwrap() {
        let pixels = base64::engine::general_purpose::STANDARD
            .decode(v["pixelsBase64"].as_str().unwrap())
            .unwrap();
        let hash = codec::encode(
            &pixels,
            v["w"].as_u64().unwrap() as u32,
            v["h"].as_u64().unwrap() as u32,
        )
        .unwrap();
        assert_eq!(hash, v["hash"], "encode {}", v["name"]);
        let w = v["decodeWidth"].as_u64().unwrap() as u32;
        let h = v["decodeHeight"].as_u64().unwrap() as u32;
        let preview = codec::decode(&hash, w, h).unwrap();
        assert_eq!(
            base64::engine::general_purpose::STANDARD.encode(&preview),
            v["decodedBase64"],
            "decode {}",
            v["name"]
        );
        let png = codec::png(&preview, w, h).unwrap();
        let mut reader = png::Decoder::new(std::io::Cursor::new(png))
            .read_info()
            .unwrap();
        let mut out = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut out).unwrap();
        assert_eq!((info.width, info.height), (w, h));
        assert_eq!(&out[..info.buffer_size()], &preview);
    }
    let hash = o["result"]["hash"].as_str().unwrap();
    let preview = codec::decode(hash, 400, 300).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(preview)),
        o["previewRgbaSha256"]
    );
}
#[test]
fn dimensions_rounding_uri_and_extension_contracts() {
    assert_eq!(media::sizes(8, 6).unwrap(), (8, 6, 300));
    assert_eq!(media::sizes(200, 1).unwrap(), (100, 1, 2)); // JS Math.round(0.5) = 1
    assert!(media::sizes(1000, 1).is_err());
    assert!(media::sizes(1, 1000).is_err());
    assert_eq!(media::dimensions(&serde_json::json!({"streams":[{"codec_type":"audio"},{"width":8,"height":6},{"width":9,"height":9}]})).unwrap(),(8,6));
    for v in [
        serde_json::json!({}),
        serde_json::json!({"streams":[]}),
        serde_json::json!({"streams":[{"width":20000,"height":1}]}),
    ] {
        assert!(media::dimensions(&v).is_err());
    }
    assert_eq!(
        media::decode_uri("/a%20b+%E2%82%AC.png").unwrap(),
        "/a b+€.png"
    );
    for s in ["%", "%GG", "%FF", "%ED%A0%80"] {
        assert!(media::decode_uri(s).is_err());
    }
    for ext in [
        "mp4", "mov", "avi", "mkv", "webm", "m4v", "flv", "wmv", "3gp", "ts", "jpg", "jpeg", "png",
        "gif", "bmp", "webp", "tiff", "tif", "heic", "heif", "avif", "ico",
    ] {
        assert!(media::supported(std::path::Path::new(&format!(
            "/file.{}",
            ext.to_uppercase()
        ))));
    }
    assert!(!media::supported(std::path::Path::new("/file.svg")));
    assert!(codec::encode(&[], 0, 1).is_err());
    assert!(codec::decode("bad", 1, 1).is_err());
}
#[test]
fn text_paths_decode_then_literal_and_no_extensionless_trust() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".work");
    std::fs::create_dir_all(&root).unwrap();
    let dir = tempfile::tempdir_in(root).unwrap();
    let plain = dir.path().join("encoded name.PNG");
    std::fs::write(&plain, b"fixture").unwrap();
    let uri = format!(
        "\u{feff} file://{} \u{00a0}",
        plain.to_str().unwrap().replace(' ', "%20")
    );
    assert_eq!(media::resolve_text(&uri).unwrap(), Some(plain));
    let literal = dir.path().join("literal%20name.png");
    std::fs::write(&literal, b"fixture").unwrap();
    assert_eq!(
        media::resolve_text(literal.to_str().unwrap()).unwrap(),
        Some(literal)
    );
    let noext = dir.path().join("extensionless");
    std::fs::write(&noext, b"fixture").unwrap();
    assert!(media::resolve_text(noext.to_str().unwrap())
        .unwrap()
        .is_none());
}
#[cfg(unix)]
#[test]
fn private_cleanup_checks_identity_and_does_not_sweep_foreign_names() {
    use tezbar_blurhash::owned::OwnedPng;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".work");
    std::fs::create_dir_all(&root).unwrap();
    let d = tempfile::tempdir_in(root).unwrap();
    let foreign = d.path().join("blurhash-raw-foreign");
    std::fs::write(&foreign, b"keep").unwrap();
    let owned = OwnedPng::create(d.path(), b"owned").unwrap();
    let p = owned.path().to_path_buf();
    drop(owned);
    assert!(!p.exists());
    assert!(foreign.exists());
    let owned = OwnedPng::create(d.path(), b"owned").unwrap();
    let p = owned.path().to_path_buf();
    std::fs::rename(&p, d.path().join("original")).unwrap();
    std::fs::write(&p, b"replacement").unwrap();
    drop(owned);
    assert_eq!(std::fs::read(p).unwrap(), b"replacement");
}
