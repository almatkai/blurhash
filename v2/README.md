# BlurHash native Rust v2

Standalone schema-3, protocol-1 executable for **clipboard-blurhash**. Production logic, encoding, decoding and PNG preview generation are Rust. No app, Tauri, Bun or JavaScript runtime is required. V1 remains intact; the root README only adds this discovery link.

Build from this directory with Rust 1.85 or newer:

```sh
cargo build --release --locked --bin tezbar-blurhash
cargo test --all-targets --all-features --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
```

The independent SDK is `tezbar-extension-sdk = 0.1.1`, pinned by HTTPS git revision `e955911f0bf5b558f9d3eb5183df0adc95c22af2`; Cargo.lock is authoritative. A newer mutable upstream default branch is not used. Own source is `048ffb1ac00ed2e5a7a7e99b69248d769663c7f5`. Host contracts are frozen at `ea100cee3f69711de12485ba061950c5fc65258f`; no host checkout/build is required.

## Behavior and explicit adaptations

- Both ffmpeg and ffprobe are located before acquiring input. macOS checks `/opt/homebrew/bin`, `/usr/local/bin`, `/usr/bin`, then `/usr/bin/which`; Windows uses `SystemRoot/System32/where.exe` with `.exe` names. Returned paths must resolve to absolute executable regular files. Missing tools show **Copy Install Command**, **Open Instructions**, and an explicit retry. Commands are `brew install ffmpeg` or `winget install --id Gyan.FFmpeg`, with the original platform-specific documentation URLs. Nothing installs or downloads a helper automatically.
- The host exposes text and genuine clipboard pixels separately, without Raycast's `Clipboard.read().file` snapshot. The adapter first requests `{format:"image"}`; host arboard pixels arrive as bounded `{data,mimeType:"image/png",width,height}`. They become an exclusive private PNG whose displayed/copied filename is `clipboard.png`. This replaces V1's extensionless clipboard-file trust; it does not mistake copied image pixels for a path string. The image and text reads are not atomic, so a clipboard change between them is an unavoidable host API gap.
- If no image is available, read text via `{}`. JS whitespace trimming, case-sensitive `file://` stripping, strict percent/UTF-8 decoding, case-insensitive supported extensions, decoded path before original literal path, and malformed-percent errors match the source. Native input must additionally be absolute. Text paths require a supported suffix; V1's distinct trusted `clip.file` property cannot be reconstructed from the host's text API. No renderer payload can supply a filesystem path.
- If text does not resolve, `{kind:"files"}` obtains genuine validated Finder/Explorer paths; select the first existing regular supported file. Explicit **Use Selected File** reacquires selection; failures remain visible and deliberately retryable. The original image/video suffix sets are preserved, including `.ts`, `.ico`, `.heic`, `.heif`, `.avif`. Actual decoding depends on the installed FFmpeg build, codecs and media contents; the extension does not promise every file with such a suffix is decodable.
- ffprobe reads the first stream with positive integer dimensions. Hash dimensions use the source's `Math.round` rule on `min(100/w,100/h,1)`. A dimension rounding to zero remains an error. ffmpeg emits first-frame RGBA. The 4×3 encoder ignores alpha like `blurhash@2.0.5`, retains x-outer/y-inner summation, the source's signed AC maximum, quantization and base83. Portable `libm` f64 operations and exact JSON float parsing are used. See the finite parity evidence below; no universal floating-point equivalence is claimed.
- Preview decodes to width 400 and height `Math.round(h/w*400)`, then encodes PNG in memory. Original filename, dimensions, integer-GCD aspect ratio and hash appear in actual Detail metadata. Copy All is exactly `"${fileName} ${width}x${height} ${ratio} ${hash}"`; Copy BlurHash comes first. There is no `raycast-height` suffix on a base64 data URI.
- **Open Clipboard History** honestly explains how to return to Tezbar's registered Clipboard History command. Frozen host has no typed navigation service. Source uses `${RAYCAST_SCHEME ?? "raycast"}://extensions/raycast/clipboard-history/clipboard-history`; opening an unrelated installed Raycast application would not establish Tezbar navigation parity. No invented navigation RPC or guessed URI is used.
- V1's global `cleanupStaleTempFiles` sweep is deliberately omitted. Preview never creates files. Only the operation's exclusive private clipboard PNG can be deleted, and cleanup checks its original OS inode/file ID. Foreign `blurhash-raw-*` and `blurhash-png-*` names are never scanned. A crash may leave the operation's own file for host-private temp lifecycle cleanup; no startup directory sweep runs. Input media is never modified.

## Ownership, limits and side effects

At most 8 authoritative sessions and 1 media job per process; SDK/manifest concurrency is `reject-while-busy`. No background lease, polling or detached task is used. Every view sets `refreshMs:null`. Refresh, search and load-more only return checkpoints; they never read the clipboard, rerun helpers or repeat a write. Initialize does no acquisition. A new command launch is the explicit initial acquisition; process restarts do not persist or auto-replay operations.

SDK clones transactional state and commits only successful operations. A bounded SDK-owned per-session drop guard also releases authoritative registrations after cancelled/failed opens, which SDK removes without adapter disposal. A separately bounded authoritative checkpoint marks generation/copy pending **before** the await. A dropped operation advances action epochs and exposes deliberate retry. Old, queued, disposed and unavailable action IDs are rejected. Successful generation/failure remains retryable; cancellation does not permanently lock a session. Clipboard writes succeed only on the actual `{ok:true}` acknowledgement. An error, cancellation, timeout or nonconforming response leaves exact prepared bytes with an explicit uncertainty message and retry; a heartbeat, refresh or late acknowledgement cannot duplicate the copy. No helper regeneration or input reacquisition occurs on a prepared-copy retry.

| Resource | Limit |
| --- | --- |
| Input | absolute regular file, 512 MiB, filename ≤4096 UTF-8 bytes |
| Text/selection path | 32 KiB each, ≤128 selected paths |
| Source dimensions | positive integers, each ≤16384, ≤64,000,000 pixels |
| Hash | 1..100 per dimension, exact RGBA byte count |
| Preview | 400 × 1..1600, ≤640,000 pixels |
| Clipboard PNG | host limits: ≤11 MiB encoded, ≤64 MiB RGBA, each dimension ≤16384 |
| Locator | 3 s, 64 KiB stdout |
| Each ffprobe/ffmpeg | 15 s, 64 KiB stderr; probe stdout 1 MiB, raw stdout exactly bounded by RGBA size |
| Whole generation | 45 s (host command deadline 60 s) |
| FFmpeg allocation option | `-max_alloc 67108864` is a per-allocation bound, not a total OS memory sandbox |

Helpers use `tokio::process::Command`, absolute validated paths, structured args, null stdin, concurrent bounded stdout/stderr reads, no user filename in an option position, kill-on-drop and an owned POSIX group / Windows Job Object. Cancellation, dispose, EOF, shutdown, overflow and timeout drop the invocation and stop its tree. Windows helper launch requests no console and attaches its Job Object immediately after spawn; host containment supplies the parent job. That pre-assignment interval and native Windows behavior still require actual platform acceptance. Raw stderr, clipboard content and filenames are not logged. Helper output limits return errors; no silent truncation.

Only `file,pipe` protocols are permitted for both media helpers; network protocols are denied even when a local file disguises a playlist. This is not an OS filesystem sandbox: a decoder may read referenced local resources. Untrusted codec CPU/memory behavior remains dependent on external FFmpeg; deadlines/output/dimension bounds and host process containment limit exposure. Synchronous bounded codec phases yield between stages so cancellation is observed before checkpoint publication.

## Validation and host prerequisites

[VALIDATION.md](VALIDATION.md) records available platform evidence and gaps. [PROVENANCE.json](PROVENANCE.json) pins source objects and fetched evidence. [INDEX.json](INDEX.json) inventories all Git-visible repository files except itself; regenerate after final edits and parent staging with `python3 v2/scripts/inventory.py` from repository root.

`fixtures/source` preserves actual own TSX and exact npm lock; `fixtures/library` preserves the locked npm implementation, source map and TypeScript sources. The VM oracle denies uncaptured imports and fakes **all** fs/os/child_process/Raycast/network boundaries before source execution. Its cleanup and preview callbacks execute against an in-memory filesystem, with real producers tripwired. Its only real output writes are explicit evidence inside this clone. Do not run the original source against a real desktop or filesystem.

128 actual TypeScript/library RGBA fixtures cover solid/extreme colors, alpha, random images, dimensions and base83. Hashes and decoded bytes match exactly for that finite set, with independent PNG roundtrips and a source 400×300 preview digest. 774 actual TS utility cases cover half-byte boundaries and their neighbors, saturation and the sRGB branch threshold. Floating-point equivalence outside this measured set is not proven.

Real adapter unit tests and executable JSONL pipes exercise genuine SDK code, grants, acquisition/actions, strict acknowledgement, cancellation/uncertainty, stale actions, explicit retry, disposal, EOF, malformed frames, bounded sessions, helper failures and descendant cleanup. FFmpeg 9.0.1 tests use only clone-owned PNG/video/hostile-playlist fixtures. Native MSVC, actual clipboard/Explorer/Finder, real GUI layout and target-wide media support are not inferred from mocks or cross-checking.

Two narrow generic prerequisite patches are supplied in [integration](integration/README.md), with tests on exact host excerpts. They are **not applied** to the host. Metadata and unmodified data-URI image rendering are already supported.

## Packaging and CI

`python3 scripts/isolated-build.py` builds an isolated source copy with a fresh Cargo home, exact HTTPS SDK revision and lockfile. `scripts/package.py --binary <native-binary> --target aarch64-apple-darwin` creates a deterministic ZIP under ignored `output`, inventories every member except the manifest itself, verifies native headers and hashes, extracts into owned `.work` and runs safe probes/JSONL smoke. No target/cache/fixtures/.work enters a ZIP. Original 47,089-byte PNG is offline in every package: SHA256 `65de4d066d5fda7187729cf4a4e3c6229389aff749ae58ca7b34a487381e521f`.

`extension.json` is a source/development manifest with `artifacts:{}` and no fictional publication URL. Both actual frozen host runtime validation and its real JSON Schema accept this form. It is not an install-ready trusted catalog pin. Generated target summaries contain measured archive/executable hashes for parent review, without fabricated artifact URLs. Parent owns release/catalog integration.

`ci/native.yml` is an inactive, reviewable template for native macOS ARM64, Windows x64 MSVC and Rust 1.85. It asserts the runner architecture before packaging, uses dummy helpers for default tests, and does not install FFmpeg or activate a root workflow. CI text is not evidence that these platforms were run.

Original extension: Freaktion (`@freaktion-craft`), MIT. Own repository: almatkai, MIT. Wolt BlurHash: MIT. SDK: Apache-2.0. See [NOTICE](NOTICE) and resolved grants under [licenses](licenses).
