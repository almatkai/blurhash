//! Test-only executable. Calls the real Adapter/SDK, or acts as an owned dummy helper.
use std::{io::Write, path::PathBuf, time::Duration};
#[tokio::main(worker_threads = 2)]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "--serve") {
        let tool = PathBuf::from(args.get(2).expect("owned dummy helper path"));
        if let Err(error) = tezbar_extension_sdk::serve(tezbar_blurhash::Adapter::with_helpers(
            tezbar_blurhash::media::Helpers {
                ffmpeg: tool.clone(),
                ffprobe: tool,
            },
        ))
        .await
        {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).is_some_and(|a| a == "--descendant") {
        let mut log = std::fs::OpenOptions::new()
            .append(true)
            .open(&args[2])
            .unwrap();
        writeln!(log, "{}", std::process::id()).unwrap();
        tokio::time::sleep(Duration::from_secs(120)).await;
        return;
    }
    let input = args
        .iter()
        .position(|a| a == "-i")
        .and_then(|i| args.get(i + 1))
        .expect("bounded local input");
    assert!(std::path::Path::new(input).is_absolute());
    if args.iter().any(|a| a == "-show_streams" || a == "-s") {
        let protocol = args
            .iter()
            .position(|a| a == "-protocol_whitelist")
            .expect("protocols must be constrained");
        assert_eq!(args[protocol + 1], "file,pipe");
        assert!(!args.iter().any(|a| a == "-y"));
    }
    let bytes = std::fs::read(input).unwrap();
    if bytes == b"stderr-overflow" {
        std::io::stderr()
            .write_all(&vec![b'x'; 128 * 1024])
            .unwrap();
        return;
    }

    if bytes == b"slow" {
        let log = PathBuf::from(input).with_extension("pids");
        std::fs::write(&log, format!("{}\n", std::process::id())).unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--descendant")
            .arg(&log)
            .spawn()
            .unwrap();
        tokio::time::sleep(Duration::from_secs(120)).await;
        let _ = child.kill();
        let _ = child.wait();
        return;
    }
    if bytes == b"bad" {
        std::process::exit(2);
    }
    if bytes == b"overflow" {
        std::io::stdout()
            .write_all(&vec![b'x'; 2 * 1024 * 1024])
            .unwrap();
        return;
    }
    if args.iter().any(|a| a == "-show_streams") {
        println!("{{\"streams\":[{{\"codec_type\":\"audio\"}},{{\"width\":8,\"height\":6}}]}}");
        return;
    }
    let size = args
        .iter()
        .position(|a| a == "-s")
        .and_then(|i| args.get(i + 1))
        .expect("size");
    let (w, h) = size.split_once('x').unwrap();
    let n = w.parse::<usize>().unwrap() * h.parse::<usize>().unwrap() * 4;
    std::io::stdout()
        .write_all(
            &(0..n)
                .map(|i| ((i * 73 + 29) % 256) as u8)
                .collect::<Vec<_>>(),
        )
        .unwrap();
}
