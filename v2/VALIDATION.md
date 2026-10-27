# Validation evidence and remaining acceptance

Executed on macOS ARM64 in the authorized BlurHash clone, 2026-10-25. Nothing was staged, committed, pushed, published, installed or subdelegated. No host/profile/SDK/sibling/ledger files were changed. The host was not checked out, stashed or built; its contract evidence is committed objects at `ea100cee3f69711de12485ba061950c5fc65258f`.

## Executed checks

| Check | Result |
| --- | --- |
| Rust stable `1.98.0 (88d9e12ae 2026-08-18)` | 24 tests passed, 0 failed; one explicitly separate real-FFmpeg test ignored by default |
| Rust MSRV `1.85.0 (4d91de4e4 2025-02-17)` | Same 24 tests passed, 0 failed |
| rustfmt | `cargo fmt --check` and `cargo +1.85.0 fmt --check` passed |
| Strict Clippy | stable and 1.85, `--all-targets --all-features --locked -- -D warnings`, passed |
| Windows GNU Rust target | `cargo check` and strict all-target/all-feature Clippy passed for installed `x86_64-pc-windows-gnu`; no MinGW linker was available, no executable/native Windows execution claim |
| Original V1 | Exact Git-object disposable copy, exact lock, `npm ci --ignore-scripts`, original smoke test 1/1 and `tsc --noEmit` passed; no baseline fixes |
| Actual own TSX oracle | Original source bytes, transpiled in VM; all imports/fs/os/child_process/API seams captured before execution; fake home/tmp only under clone; real producers tripwired; cleanup and preview file callbacks remained entirely fake |
| Actual locked library | `blurhash@2.0.5` from exact npm integrity; 128 finite RGBA cases, exact hashes and decoded pixel arrays, independent PNG roundtrips; source 400×300 preview digest matches |
| Numeric edge oracle | 774 values from actual TypeScript utilities: exact bit-preserving fixture float parsing, sRGB threshold, extrema, saturation and half-byte boundary neighbors; all checked bytes match |
| Actual SDK/Adapter unit tests | Typed image/text/selection shapes, install copy/URL actions, PNG ownership, strict positive clipboard ack, uncertainty, exact prepared retry, stale IDs, selected-file retry, capability denial, session cap, output caps and sparse oversized input |
| Actual executable JSONL pipes | hello/init/open/call/cancel/dispose/shutdown/EOF, malformed JSON/CR/truncation, search/refresh no replay, late reverse ack ignored, prepared-copy retry, 12 cancelled opens without leaked registration slots |
| Helper lifecycle | Own dummy helper trees stop on timeout, explicit cancel, dispose, shutdown and EOF; normal failure/cancelled selection remains deliberately retryable; stderr/stdout caps fail without truncation |
| Real media helpers | Separately invoked ignored test passed: FFmpeg **9.0.1**, ffprobe **9.0.1**, generated owned PNG and FFV1/MKV, first-frame dimensions/hash/copy text, hostile local playlist denied by protocol restriction; no desktop clipboard or real media used |
| Host runtime validator | Exact frozen `process_install::validate` + exact launcher validation functions compiled with narrow harness dependencies; source manifest positive and identity/schema/protocol/capability/path/concurrency/deadline negatives passed |
| Actual JSON Schema | Pinned schema-v3 via Ajv 2020, positive canonical identity/replacements/minimal grants/`artifacts:{}` and seven negative variants passed |
| Actual renderer | Frozen List/Detail/surface functions executed with React-element seams: List `actionIds`, image-only base64 URI, root.metadata labels, Detail primary-button before/after regression passed |
| Active-version icon prerequisite | Actual registry collect/icon functions before/after patch: missing icon becomes exact version resource; canonical mutable preference/discovery root remains unchanged; verified Package return and search roots are explicit seams |
| Independent build | Fresh private Cargo home and isolated source copy, HTTPS pinned SDK, no local path dependency or app build; current production sources/manifest/lock match the isolated copy |
| Native ARM64 ZIP | Native Mach-O header, complete file inventory, original offline icon, all notices/licenses, deterministic double packaging, actual extracted executable probes and JSONL hello/init/shutdown passed |
| V1 preservation | All 15 original files checked against Git objects; only README's appended discovery link is permitted; all other V1 bytes exact |

The default Rust test count includes one numeric utility test, nine Adapter/helper tests, four codec/path/ownership tests, one runtime-validator test, eight real process tests and one registry-resource test. The real media test is an additional explicit pass. Default tests use an owned test-only executable that links the production Adapter and genuine SDK, with explicitly injected dummy helpers. They never locate or invoke desktop helpers. The production binary's probes and extracted JSONL smoke do not open a media session, so they do not locate real helpers or touch clipboard/selection.

Commands are in README and scripts; local execution logs are retained under ignored `.work`. Durable provenance is in `PROVENANCE.json`, measurements in `PACKAGE_EVIDENCE.json`, and all Git-visible bytes/hashes in `INDEX.json`. Re-run the inventory after parent staging; this handoff intentionally leaves staging empty.

## Finite claims and explicit seams

The source oracle uses actual TSX and actual locked blurhash code. Its ffmpeg/ffprobe output, file existence/stat/read/write/readdir/unlink, clipboard, selection, shell-open, hooks and toast services are captured fakes. It tests original helper arguments, acquisition order, URI/error/extension behavior, actions and cleanup/preview callbacks without running a source-side helper or touching a real temp/user file. The production native real-FFmpeg test supplies separate decoder evidence. Neither is a real desktop acceptance test.

Pure Rust f64/libm and exact float parsing pass the finite codec and scalar corpus; this is not a proof for every possible pixel array/platform/transcendental rounding boundary. Decoder output depends on external helper version, formats, colorspace, scaling, stream selection and media metadata. Dimensions/size/aspect/duration bounds deliberately reject cases V1 could attempt. Original filename is preserved for normal local inputs; copied image pixels use stable `clipboard.png`, since the host has no original Raycast file snapshot. Private PNG format and separate read ordering are explicit adaptations.

The first scalar-boundary experiment exposed float representation/transcendental concerns; final bit-exact fixture parsing and portable libm tests resolved all measured failures. No quantization snapping, hand-written expected Rust hashes or universal parity claim was used.

Two generic host patches are supplied but not applied: Detail click wrapper and active-version resource path. Actual image-only Detail and metadata already work without a base64 query suffix. Tests use committed excerpts, fake React elements and a verified-package seam; they do not prove full host build, DOM layout or launcher acceptance. Clipboard History has an honest instruction adaptation because no typed navigation service exists in the frozen host.

## Not run / release gates remaining with parent

- Native Windows MSVC build/run, Windows Job Object/no-console acceptance, Explorer selection and real Windows clipboard.
- Live macOS clipboard/Finder/launcher icons/Detail keyboard and mouse GUI acceptance.
- Exhaustive FFmpeg codec/format coverage, adversarial media memory-sandbox proof, or universal bitwise math parity.
- Any extension installation, catalog/ledger update, release, PR, push or deployment. `ci/native.yml` is an inactive review template, not evidence.

Windows GNU checks validate Rust type checking, not MSVC linking or Windows behavior. Helper jobs attach immediately after spawn; the parent host job assists containment, and the native Windows pre-assignment interval remains untested. Native executables are not OS-sandboxed. Protocol `file,pipe` denies network fetches but does not sandbox referenced local files. Per-allocation FFmpeg and output/work/deadline caps do not establish a global OS RSS ceiling.

## XERJ coverage

Canonical root is this clone; prefix `ax-ec7150c05915`, state `/Users/almat/.local/share/xerj/workspaces/ax-ec7150c05915`. Initial generation 1 indexed 4/4 code files (10 files, 61 records overall; binary/junk/ignore exclusions remained). The final update **aborted** because the frozen dataset schema does not accept new PROVENANCE JSON fields. Final XERJ coverage is incomplete. No global restart, schema mutation or alternate-prefix bypass was attempted. Full current files, exhaustive `rg`, Git object comparisons and SHA256 inventories were used as authority.

Parent review tightened the instructions-open acknowledgement: null, booleans, contradictory error/success fields and a non-string message cannot report success. The actual Adapter regression covers both platform instruction URLs; helper/media behavior and immutable fixtures remain unchanged.
