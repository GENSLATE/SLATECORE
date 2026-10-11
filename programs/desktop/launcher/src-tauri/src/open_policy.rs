//! Which vault files the launcher will hand to Windows to open.
//!
//! `vault_open` decrypts a file into the session folder and asks Explorer to open it, which is a
//! double click: for a program, script, shortcut or installer that is *running it*. A file that
//! came out of the vault has lost its Mark-of-the-Web (the import drops it), so Windows will not
//! warn first. The launcher therefore refuses these types (owner-protective default; recorded in
//! the Task 12 fix round). The file can still be exported to a folder and handled from there,
//! where the person is in charge.
//!
//! Macro-enabled Office files (`docm`, `xlsm`, `pptm`) are deliberately *not* blocked: their
//! program asks before it runs a macro. What the vault round trip does cost them is Protected
//! View, which Office applies only to files that carry the Mark-of-the-Web, and an import drops
//! it. The UI warning about opening files from the vault should say so.
//!
//! The check is on the *name*, the way Windows reads it: case does not matter, trailing dots and
//! spaces are dropped (`evil.exe.` is `evil.exe`), only the last extension counts
//! (`report.pdf.exe` is an `exe`) and a name that is only an extension (`.exe`) still has it.

// cspell:ignore appinstaller dotm fullwidth jnlp pyzw searchconnector vhdx appref appxbundle diagcab msix msixbundle rsplit settingcontent themepack xbap

/// Extensions (lower case, no dot) of files that run code, install it or point at it when they
/// are opened. Broader than "executable": scripts, shortcuts, registry and policy files.
const BLOCKED: &[&str] = &[
    // Programs and libraries.
    "exe",
    "com",
    "scr",
    "pif",
    "dll",
    "sys",
    "ocx",
    "cpl",
    "msc",
    "jar",
    "gadget",
    // Installers and packages.
    "msi",
    "msp",
    "mst",
    "appx",
    "msix",
    "appxbundle",
    "msixbundle",
    "application",
    "xbap",
    // Scripts and script hosts.
    "bat",
    "cmd",
    "ps1",
    "ps1xml",
    "ps2",
    "ps2xml",
    "psc1",
    "psc2",
    "psd1",
    "psm1",
    "vbs",
    "vbe",
    "js",
    "jse",
    "wsf",
    "wsh",
    "wsc",
    "hta",
    "sct",
    // Shortcuts, links and shell objects.
    "lnk",
    "url",
    "appref-ms",
    "scf",
    "shb",
    "shs",
    "website",
    "settingcontent-ms",
    "library-ms",
    "search-ms",
    "diagcab",
    // Registry and setup information.
    "reg",
    "inf",
    "theme",
    "themepack",
    // Help files that run script, interpreter scripts, update packages and add-ins.
    "chm",
    "py",
    "pyw",
    "pyz",
    "pyzw",
    "msu",
    "cab",
    "appinstaller",
    "jnlp",
    "vsix",
    "searchconnector-ms",
    // Office and Access add-ins and compiled databases (code in the file, no macro prompt).
    "xll",
    "xlam",
    "ppam",
    "mde",
    "accde",
    "ade",
    "adp",
    // Disk images Explorer mounts as a drive.
    "iso",
    "img",
    "vhd",
    "vhdx",
];

/// The file name of `path`, a vault path (`Docs/a.txt`) or a Windows path, without the folders.
fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Whether the launcher refuses to open the file at `path` (a vault path or a file name).
pub fn is_blocked_type(path: &str) -> bool {
    let name = file_name(path);
    // A stream (`name:stream`) is not a file Explorer opens by name; refuse it outright rather
    // than reason about which side of the colon decides.
    if name.contains(':') {
        return true;
    }
    let name = name.trim_end_matches(['.', ' ']);
    // Everything after the last dot, even when the dot is the first character (`.exe`).
    let Some((_, extension)) = name.rsplit_once('.') else {
        return false;
    };
    // Anything outside ASCII (a Cyrillic `e` in `exe`, a right-to-left override) is not a type
    // this list can vouch for.
    if !extension.is_ascii() {
        return true;
    }
    BLOCKED
        .iter()
        .any(|blocked| blocked.eq_ignore_ascii_case(extension))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_scripts_shortcuts_and_installers_are_blocked() {
        for name in [
            "setup.exe",
            "run.bat",
            "run.cmd",
            "s.ps1",
            "s.vbs",
            "s.js",
            "s.hta",
            "s.msi",
            "s.scr",
            "s.cpl",
            "s.lnk",
            "s.url",
            "s.appref-ms",
            "s.com",
            "s.pif",
            "s.jar",
            "s.wsf",
            "s.reg",
            "s.dll",
            "s.jse",
            "s.vbe",
            "s.wsh",
            "s.msp",
            "s.psm1",
            "s.settingcontent-ms",
            "s.inf",
            "s.msix",
            "s.gadget",
        ] {
            assert!(is_blocked_type(name), "{name}");
        }
    }

    #[test]
    fn case_does_not_matter() {
        for name in [
            "A.EXE",
            "a.Exe",
            "SETUP.Bat",
            "x.PS1",
            "x.LnK",
            "x.APPREF-MS",
        ] {
            assert!(is_blocked_type(name), "{name}");
        }
    }

    #[test]
    fn trailing_dots_and_spaces_do_not_hide_the_type() {
        for name in [
            "evil.exe.",
            "evil.exe ",
            "evil.exe. .",
            "evil.exe...",
            "evil.exe  ",
            "evil.EXE. ",
        ] {
            assert!(is_blocked_type(name), "{name:?}");
        }
    }

    #[test]
    fn only_the_last_extension_counts_and_a_bare_extension_still_counts() {
        assert!(is_blocked_type("report.pdf.exe"));
        assert!(is_blocked_type("invoice.docx.lnk"));
        assert!(is_blocked_type(".exe"));
        assert!(!is_blocked_type("setup.exe.txt"));
        assert!(!is_blocked_type("run.bat.bak"));
    }

    #[test]
    fn the_folders_in_front_of_the_name_are_ignored() {
        assert!(is_blocked_type("Documents/tools/setup.exe"));
        assert!(is_blocked_type(r"Documents\tools\setup.exe"));
        assert!(!is_blocked_type("setup.exe/readme.txt"));
        assert!(!is_blocked_type(r"setup.exe\readme.txt"));
        assert!(!is_blocked_type("folder.exe/"));
    }

    #[test]
    fn a_stream_suffix_is_blocked_as_well() {
        assert!(is_blocked_type("note.txt:evil.exe"));
        assert!(is_blocked_type("setup.exe:Zone.Identifier"));
    }

    #[test]
    fn the_second_round_of_types_is_blocked() {
        for ext in [
            "chm",
            "py",
            "pyw",
            "pyz",
            "pyzw",
            "msu",
            "xll",
            "xlam",
            "ppam",
            "mde",
            "accde",
            "ade",
            "adp",
            "cab",
            "appinstaller",
            "jnlp",
            "vsix",
            "searchconnector-ms",
            "iso",
            "img",
            "vhd",
            "vhdx",
        ] {
            assert!(is_blocked_type(&format!("file.{ext}")), "{ext}");
            assert!(
                is_blocked_type(&format!("FILE.{}", ext.to_uppercase())),
                "{ext}"
            );
        }
    }

    #[test]
    fn macro_office_files_stay_openable() {
        // Their program shows its own warning, and Protected View is lost by the import (see the
        // module docs), which is why this is a documented choice and not an oversight.
        for name in ["budget.xlsm", "letter.docm", "slides.pptm", "tpl.dotm"] {
            assert!(!is_blocked_type(name), "{name}");
        }
    }

    #[test]
    fn an_extension_with_a_non_ascii_character_is_refused() {
        for name in [
            "report.\u{435}xe", // Cyrillic e
            "report.ex\u{435}",
            "note.t\u{ff58}t", // fullwidth x
            "photo.jp\u{e9}g",
            "a.\u{202e}txt", // right-to-left override
        ] {
            assert!(is_blocked_type(name), "{name:?}");
        }
        // A non-ASCII *name* is fine.
        assert!(!is_blocked_type("r\u{e9}sum\u{e9}.pdf"));
        assert!(!is_blocked_type("\u{65e5}\u{672c}\u{8a9e}.txt"));
    }

    #[test]
    fn documents_pictures_and_data_are_allowed() {
        for name in [
            "notes.txt",
            "scan.pdf",
            "photo.jpeg",
            "archive.tar.gz",
            "data.json",
            "app.jsx",
            "report.docx",
            "readme",
            "exe",
            "exe.txt",
            "a.com.txt",
            "",
            "dir/",
        ] {
            assert!(!is_blocked_type(name), "{name:?}");
        }
    }
}
