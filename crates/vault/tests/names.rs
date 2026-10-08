//! Logical names live only in the encrypted index (N1 to N4, N6, N7).

mod common;

use std::fs;

use common::{Fixture, PASSWORD, TestResult, pw};
use genslate_vault::format::index::IndexSlotHeader;
use genslate_vault::{Conflict, LockPolicy, VaultError, VaultPath};

fn invalid(s: &str) -> bool {
    matches!(VaultPath::parse(s), Err(VaultError::InvalidPath(_)))
}

#[test]
fn unicode_and_long_names_roundtrip_via_index() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    let long_ascii = format!("{}.txt", "a".repeat(251));
    let long_emoji = "\u{1F600}".repeat(127); // 254 UTF-16 units, 508 UTF-8 bytes
    let names = [
        "caf\u{e9}.txt".to_owned(),              // NFC
        "cafe\u{301}.txt".to_owned(),            // NFD: a different name, stored as given
        "a\u{308}\u{301}\u{323}.txt".to_owned(), // stacked combining marks
        "\u{65e5}\u{672c}\u{8a9e}\u{306e}\u{30d5}\u{30a1}\u{30a4}\u{30eb}.txt".to_owned(), // CJK
        "\u{5e9}\u{5dc}\u{5d5}\u{5dd} \u{645}\u{631}\u{62d}\u{628}\u{627}.txt".to_owned(), // RTL
        "emoji \u{1F600}\u{1F389}\u{1F468}\u{200D}\u{1F469}.txt".to_owned(), // surrogate pairs, ZWJ
        "\u{ff26}\u{ff55}\u{ff4c}\u{ff4c}\u{ff57}\u{ff49}\u{ff44}\u{ff54}\u{ff48}".to_owned(), // fullwidth
        long_ascii.clone(),
        long_emoji.clone(),
    ];
    assert_eq!(long_ascii.encode_utf16().count(), 255);
    assert_eq!(long_emoji.encode_utf16().count(), 254);

    // Import under a short on-disk name, then rename to the logical name (the index is the
    // only place the name lives, and some of these exceed Linux's 255-byte file names).
    for (i, name) in names.iter().enumerate() {
        let short = format!("f{i}.bin");
        fixture.import_bytes(&vault, "", &short, name.as_bytes())?;
        vault.rename(&VaultPath::parse(&short)?, &VaultPath::parse(name)?)?;
    }
    // A deep folder chain: depth 16 is accepted, 17 is not.
    let mut deep = VaultPath::root();
    for level in 1..=16 {
        deep = deep.join(&format!("level-{level:02}"))?;
        vault.create_dir(&deep)?;
    }
    assert!(deep.join("level-17").is_err());

    vault.lock(LockPolicy::SyncThenWipe)?;
    drop(vault);
    let vault = fixture.open()?;
    vault.unlock(&pw(PASSWORD))?;

    let mut expected: Vec<String> = names.to_vec();
    expected.push("level-01".to_owned());
    expected.sort();
    assert_eq!(
        common::names(&vault, "")?,
        expected,
        "byte-exact names after re-reading the index"
    );
    for name in &names {
        assert_eq!(
            fixture.export_bytes(&vault, name)?,
            name.as_bytes(),
            "{name}"
        );
    }
    assert_eq!(vault.list(&deep)?.len(), 0);
    assert_eq!(deep.as_str().split('/').count(), 16);

    // No logical name appears in plaintext anywhere in the vault folder.
    let on_disk = common::snapshot(fixture.root())?;
    for (path, bytes) in &on_disk {
        let file_name = path.to_string_lossy();
        for name in names.iter().map(String::as_str).chain(["level-16"]) {
            assert!(
                !file_name.contains(name),
                "name {name} visible as a file name"
            );
            assert!(
                !common::contains_bytes(bytes, name.as_bytes()),
                "name {name} in {}",
                path.display()
            );
        }
    }
    Ok(())
}

#[test]
fn name_length_limits() {
    assert!(VaultPath::parse(&"a".repeat(255)).is_ok());
    assert!(invalid(&"a".repeat(256)));
    assert!(invalid(&"\u{1F600}".repeat(128)), "256 UTF-16 units");
    // Total path: 1024 UTF-16 units accepted, 1025 rejected.
    let parts = |last: usize| {
        [
            "p".repeat(255),
            "q".repeat(255),
            "r".repeat(255),
            "s".repeat(250),
            "t".repeat(last),
        ]
        .join("/")
    };
    assert_eq!(parts(5).encode_utf16().count(), 1024);
    assert!(VaultPath::parse(&parts(5)).is_ok());
    assert!(invalid(&parts(6)));
    let sixteen = (1..=16)
        .map(|i| format!("d{i}"))
        .collect::<Vec<_>>()
        .join("/");
    assert!(VaultPath::parse(&sixteen).is_ok());
    assert!(invalid(&format!("{sixteen}/d17")));
}

#[test]
fn illegal_names_rejected() -> TestResult {
    for bad in [
        "a/",
        "/a",
        "a//b",
        ".",
        "..",
        "a/./b",
        "a/../b",
        "trailing.",
        "trailing ",
        "CON",
        "con.txt",
        "NUL",
        "nul.tar.gz",
        "COM1",
        "LPT9.log",
        "COM\u{b9}",
        "lpt\u{b3}.txt",
        "Aux",
        "PRN.x",
        "a\u{0}b",
        "tab\there",
        "bell\u{7}",
        "new\nline",
    ] {
        assert!(invalid(bad), "{bad:?} must be rejected");
    }
    for c in ['\\', ':', '*', '?', '"', '<', '>', '|'] {
        assert!(invalid(&format!("a{c}b")), "{c:?} must be rejected");
    }
    for good in [
        "CONSOLE",
        "con-tent.txt",
        "COM10",
        "LPT0",
        ".hidden",
        "a.b.c",
        "x y",
        "nul_file",
    ] {
        assert!(VaultPath::parse(good).is_ok(), "{good:?} must be accepted");
    }
    let root = VaultPath::root();
    assert_eq!(root.as_str(), "");
    assert!(root.parent().is_none());
    assert!(root.file_name().is_none());
    assert!(matches!(root.join("a/b"), Err(VaultError::InvalidPath(_))));
    let p = VaultPath::parse("Taxes/2025.pdf")?;
    assert_eq!(p.file_name(), Some("2025.pdf"));
    assert_eq!(p.parent(), Some(VaultPath::parse("Taxes")?));
    assert_eq!(VaultPath::parse("Taxes")?.parent(), Some(VaultPath::root()));
    Ok(())
}

#[test]
fn case_insensitive_collisions_conflict() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    fixture.import_bytes(&vault, "", "A.txt", b"upper")?;
    let src = fixture.scratch()?.join("a.TXT");
    fs::write(&src, b"lower")?;
    let root = VaultPath::root();

    assert!(matches!(
        vault.import(&src, &root, Conflict::Fail, None),
        Err(VaultError::Exists(_))
    ));
    assert!(matches!(
        vault.create_dir(&VaultPath::parse("a.txt")?),
        Err(VaultError::Exists(_))
    ));
    let kept = vault.import(&src, &root, Conflict::KeepBoth, None)?;
    assert_eq!(kept.path.as_str(), "a (2).TXT");
    let again = vault.import(&src, &root, Conflict::KeepBoth, None)?;
    assert_eq!(again.path.as_str(), "a (3).TXT");
    let replaced = vault.import(&src, &root, Conflict::Replace, None)?;
    assert_eq!(
        replaced.path.as_str(),
        "A.txt",
        "Replace keeps the existing entry's name"
    );
    assert_eq!(fixture.export_bytes(&vault, "A.txt")?, b"lower");

    // A case-only rename of the same entry is allowed; onto another entry it is not.
    vault.rename(&VaultPath::parse("A.txt")?, &VaultPath::parse("a.txt")?)?;
    assert!(matches!(
        vault.rename(&VaultPath::parse("a.txt")?, &VaultPath::parse("A (2).txt")?),
        Err(VaultError::Exists(_))
    ));
    Ok(())
}

#[test]
fn rename_dir_moves_descendants_in_one_commit() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    vault.create_dir(&VaultPath::parse("Docs")?)?;
    vault.create_dir(&VaultPath::parse("Docs/Sub")?)?;
    fixture.import_bytes(&vault, "Docs", "a.txt", b"a")?;
    fixture.import_bytes(&vault, "Docs/Sub", "b.txt", b"b")?;
    let generation = || -> Result<u64, Box<dyn std::error::Error>> {
        let a = IndexSlotHeader::decode(&fs::read(fixture.root().join("index.a.gvi"))?)?.generation;
        let b = IndexSlotHeader::decode(&fs::read(fixture.root().join("index.b.gvi"))?)?.generation;
        Ok(a.max(b))
    };
    let before = generation()?;
    vault.rename(&VaultPath::parse("Docs")?, &VaultPath::parse("Papers")?)?;
    assert_eq!(generation()?, before + 1, "one index commit");
    assert_eq!(common::names(&vault, "")?, vec!["Papers".to_owned()]);
    assert_eq!(
        common::names(&vault, "Papers")?,
        vec!["Sub".to_owned(), "a.txt".to_owned()]
    );
    assert_eq!(fixture.export_bytes(&vault, "Papers/Sub/b.txt")?, b"b");

    let into_itself = vault.rename(
        &VaultPath::parse("Papers")?,
        &VaultPath::parse("Papers/Sub/X")?,
    );
    assert!(matches!(into_itself, Err(VaultError::InvalidPath(_))));
    assert!(matches!(
        vault.rename(&VaultPath::parse("Nope")?, &VaultPath::parse("Other")?),
        Err(VaultError::NotFound(_))
    ));
    assert!(matches!(
        vault.rename(
            &VaultPath::parse("Papers/a.txt")?,
            &VaultPath::parse("Missing/a.txt")?
        ),
        Err(VaultError::NotFound(_))
    ));
    Ok(())
}
