use consumer::storage::Storage;

fn tmp_storage() -> (tempfile::TempDir, Storage) {
    let dir = tempfile::tempdir().unwrap();
    let s = Storage::new(dir.path()).unwrap();
    (dir, s)
}

#[test]
fn id_is_first_16_hex_chars() {
    let sha = "0123456789abcdef".to_string() + &"f".repeat(48);
    assert_eq!(Storage::id_from_sha(&sha), "0123456789abcdef");
}

#[test]
fn reservation_blocks_duplicate_until_released() {
    let (_dir, s) = tmp_storage();
    let sha = "a".repeat(64);
    assert!(s.try_reserve(&sha));
    assert!(!s.try_reserve(&sha));
    s.release(&sha);
    assert!(s.try_reserve(&sha));
}

#[test]
fn seed_reserved_from_catalog_hashes() {
    let (_dir, s) = tmp_storage();
    s.seed_reserved(vec!["a".repeat(64), "b".repeat(64)]);
    assert!(!s.try_reserve(&"a".repeat(64)));
    assert!(!s.try_reserve(&"b".repeat(64)));
    assert!(s.try_reserve(&"c".repeat(64)));
}

#[test]
fn video_rel_preserves_known_extension() {
    assert_eq!(Storage::video_rel("abc", "movie.MP4"), "uploads/abc.mp4");
    assert_eq!(Storage::video_rel("abc", "movie.mov"), "uploads/abc.mov");
    assert_eq!(Storage::video_rel("abc", "no_ext"), "uploads/abc.mp4");
    assert_eq!(Storage::video_rel("abc", "evil.exe"), "uploads/abc.mp4");
    assert_eq!(Storage::video_rel("abc", "../etc/passwd.mp4"), "uploads/abc.mp4");
}

#[test]
fn persist_moves_file_to_uploads_dir() {
    let (_dir, s) = tmp_storage();
    let staging = s.create_staging().unwrap();
    std::fs::write(staging.path(), b"hello").unwrap();
    let sha = "d".repeat(64);
    let path = s.persist(staging, &sha, "clip.mp4").unwrap();
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), format!("{}.mp4", &sha[..16]));
    assert_eq!(std::fs::read(&path).unwrap(), b"hello");
}