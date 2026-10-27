#[path = "support/registry_after.rs"]
mod after;
#[path = "support/registry_before.rs"]
mod before;
#[test]
fn actual_registry_verified_version_resources_without_moving_preferences_root() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".work");
    std::fs::create_dir_all(&root).unwrap();
    let d = tempfile::tempdir_in(root).unwrap();
    let canonical = d
        .path()
        .join("extensions/packages/com.tezbar.native.blurhash");
    let version = canonical.join("versions/2.0.0-fixture");
    std::fs::create_dir_all(version.join("resources")).unwrap();
    std::fs::write(
        version.join("extension.json"),
        include_bytes!("../extension.json"),
    )
    .unwrap();
    std::fs::write(
        version.join("resources/extension-icon.png"),
        include_bytes!("../resources/extension-icon.png"),
    )
    .unwrap();
    std::fs::write(
        canonical.join("active.json"),
        b"{\"directory\":\"2.0.0-fixture\"}",
    )
    .unwrap();
    let old = before::inspect(d.path());
    assert_eq!(old.0, canonical);
    assert!(old.1.is_none() && old.2.is_none());
    let new = after::inspect(d.path());
    assert_eq!(new.0, canonical);
    let expected = version
        .join("resources/extension-icon.png")
        .to_str()
        .unwrap()
        .to_owned();
    assert_eq!(new.1, Some(expected.clone()));
    assert_eq!(new.2, Some(expected));
}
