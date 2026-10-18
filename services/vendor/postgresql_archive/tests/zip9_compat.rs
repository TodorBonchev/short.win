//! Local, offline regression fixtures for the two ZIP 9 filename adaptations.

use postgresql_archive::configuration::zonky;
use postgresql_archive::extractor::{ExtractDirectories, zip_extract};
use regex_lite::Regex;

const PAYLOAD: &[u8] = b"zip9 filename compatibility fixture\n";

fn directories(path: &std::path::Path) -> ExtractDirectories {
    let mut dirs = ExtractDirectories::default();
    dirs.add_mapping(Regex::new(".*").unwrap(), path.to_path_buf());
    dirs
}

/// A non-UTF-8 CP437 byte makes ZIP 9 return an owned decoded name instead of an ASCII borrow.
fn cp437_name(input: &[u8], component_offset: usize) -> Vec<u8> {
    let mut bytes = input.to_vec();
    for (signature, name) in [(b"PK\x03\x04", 30), (b"PK\x01\x02", 46)] {
        let start = bytes.windows(4).position(|v| v == signature).unwrap();
        bytes[start + name + component_offset] = 0x82; // é in CP437
    }
    bytes
}

#[test]
fn generic_zip_extracts_borrowed_and_owned_decoded_names() {
    let valid = include_bytes!("fixtures/zip9-generic.zip").to_vec();
    let out = tempfile::tempdir().unwrap();
    let files = zip_extract(&valid, &directories(out.path())).unwrap();
    assert_eq!(files, [out.path().join("postgres")]);
    assert_eq!(std::fs::read(&files[0]).unwrap(), PAYLOAD);
    let owned = tempfile::tempdir().unwrap();
    let files = zip_extract(&cp437_name(&valid, 4), &directories(owned.path())).unwrap();
    assert_eq!(files, [owned.path().join("éostgres")]);
    assert_eq!(std::fs::read(&files[0]).unwrap(), PAYLOAD);
    assert!(zip_extract(&valid[..20].to_vec(), &directories(owned.path())).is_err());
}

#[test]
fn zonky_finds_txz_with_borrowed_and_owned_decoded_names() {
    let valid = include_bytes!("fixtures/zip9-zonky.jar").to_vec();
    let root = tempfile::tempdir().unwrap();
    let out = root.path().join("postgres");
    let files = zonky::extract(&valid, &directories(&out)).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(std::fs::read(out.join("bin/postgres")).unwrap(), PAYLOAD);
    let owned = tempfile::tempdir().unwrap();
    let out = owned.path().join("postgres");
    assert_eq!(
        zonky::extract(&cp437_name(&valid, 0), &directories(&out))
            .unwrap()
            .len(),
        1
    );
    assert_eq!(std::fs::read(out.join("bin/postgres")).unwrap(), PAYLOAD);
    let refused = tempfile::tempdir().unwrap();
    assert!(
        zonky::extract(
            &valid[..20].to_vec(),
            &directories(&refused.path().join("postgres"))
        )
        .is_err()
    );
}
