//! One launcher per drive: the single-instance plugin names its mutex after the bundle
//! identifier, so two launchers on two drives would share it and the second would hand off to
//! the first. Appending a hash of the install folder to the identifier gives each drive its own.

use std::path::Path;

/// FNV-1a, 64 bit. Written out because the value must not change between builds
/// (`DefaultHasher` makes no such promise) and must not need a crate.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// `<base>.<16 hex digits>` for the install folder `root`. Letter case and the kind of
/// separator do not matter (Windows paths are case-insensitive), so the same install always
/// gets the same identifier.
pub fn install_identifier(base: &str, root: &Path) -> String {
    let normal = root
        .to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase();
    format!("{base}.{:016x}", fnv1a(normal.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "xyz.genslate.slatecore.launcher";

    #[test]
    fn single_instance_identifier_is_per_install_dir() {
        let a = install_identifier(BASE, Path::new(r"E:\SLATECORE"));
        assert!(a.starts_with("xyz.genslate.slatecore.launcher."));
        assert_eq!(a.len(), BASE.len() + 1 + 16);
        assert_eq!(a, install_identifier(BASE, Path::new(r"e:\slatecore\")));
        assert_eq!(a, install_identifier(BASE, Path::new("E:/SLATECORE")));
        assert_ne!(a, install_identifier(BASE, Path::new(r"F:\SLATECORE")));
        assert_ne!(a, install_identifier(BASE, Path::new(r"E:\OTHER")));
    }

    #[test]
    fn the_hash_is_stable() {
        // FNV-1a test vectors: "" and "a".
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
