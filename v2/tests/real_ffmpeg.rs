use std::path::{Path, PathBuf};
use tezbar_blurhash::{
    codec,
    media::{self, Helpers},
};
use tezbar_extension_sdk::{ExtensionPaths, TestContext};
#[tokio::test]
#[ignore = "Explicit local FFmpeg test on disposable fixtures; never uses clipboard or locator"]
async fn genuine_ffmpeg_png_and_video_and_protocol_restriction() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".work");
    std::fs::create_dir_all(&root).unwrap();
    let dir = tempfile::tempdir_in(root).unwrap();
    let c = TestContext::new(
        ExtensionPaths {
            package_dir: dir.path().into(),
            data_dir: dir.path().into(),
            temp_dir: dir.path().into(),
        },
        &[],
    );
    let tools = Helpers {
        ffmpeg: PathBuf::from("/opt/homebrew/bin/ffmpeg"),
        ffprobe: PathBuf::from("/opt/homebrew/bin/ffprobe"),
    };
    assert!(
        tools.ffmpeg.is_file() && tools.ffprobe.is_file(),
        "explicit tools unavailable; do not auto-install"
    );
    let pixels = (0..8 * 6 * 4)
        .map(|i| ((i * 73 + 29) % 256) as u8)
        .collect::<Vec<_>>();
    let png = codec::png(&pixels, 8, 6).unwrap();
    let path = dir.path().join("fixture.png");
    std::fs::write(&path, &png).unwrap();
    let result = media::generate(&c.context, &tools, &path).await.unwrap();
    let source: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/oracle.json")).unwrap();
    assert_eq!(result.hash, source["result"]["hash"]);
    assert_eq!(
        result.copy_text,
        format!("fixture.png 8x6 4:3 {}", result.hash)
    );
    let video = dir.path().join("fixture.mkv");
    let args = vec![
        "-v".into(),
        "error".into(),
        "-nostdin".into(),
        "-protocol_whitelist".into(),
        "file,pipe".into(),
        "-i".into(),
        path.as_os_str().into(),
        "-frames:v".into(),
        "1".into(),
        "-c:v".into(),
        "ffv1".into(),
        video.as_os_str().into(),
    ];
    media::run(
        &c.context,
        &tools.ffmpeg,
        &args,
        1024,
        std::time::Duration::from_secs(10),
    )
    .await
    .unwrap();
    let result = media::generate(&c.context, &tools, &video).await.unwrap();
    assert_eq!((result.width, result.height), (8, 6));
    let playlist = dir.path().join("hostile.mp4");
    std::fs::write(&playlist,b"#EXTM3U\n#EXT-X-TARGETDURATION:10\n#EXTINF:10,\nhttps://example.invalid/never-fetch.ts\n#EXT-X-ENDLIST\n").unwrap();
    assert!(media::generate(&c.context, &tools, &playlist)
        .await
        .is_err());
    let args = vec!["-version".into()];
    let ffmpeg = media::run(
        &c.context,
        &tools.ffmpeg,
        &args,
        64 * 1024,
        std::time::Duration::from_secs(3),
    )
    .await
    .unwrap();
    let ffprobe = media::run(
        &c.context,
        &tools.ffprobe,
        &args,
        64 * 1024,
        std::time::Duration::from_secs(3),
    )
    .await
    .unwrap();
    println!(
        "{}\n{}",
        String::from_utf8_lossy(&ffmpeg).lines().next().unwrap(),
        String::from_utf8_lossy(&ffprobe).lines().next().unwrap()
    );
}
