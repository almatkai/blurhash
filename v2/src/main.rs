#[tokio::main(worker_threads = 2)]
async fn main() {
    if tezbar_extension_sdk::serve(tezbar_blurhash::Adapter::default())
        .await
        .is_err()
    {
        eprintln!("BlurHash protocol terminated");
        std::process::exit(1);
    }
}
