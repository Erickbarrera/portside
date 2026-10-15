//! SHA-256 checks against what the release publishes.

use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::{IoContext, Result};
use crate::resolve::{ReleaseFile, UpstreamRelease};

/// Lowercase hex SHA-256 of a file. Blocking; run it off the async runtime.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).at(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf).at(path)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// The release file most likely to hold `asset`'s checksum: `<asset>.sha256` (or
/// `.sha256sum`), else a combined list such as `SHA256SUMS` or `checksums.txt`.
pub fn sums_file_for<'a>(release: &'a UpstreamRelease, asset: &str) -> Option<&'a ReleaseFile> {
    let lower = |f: &&ReleaseFile| f.name.to_ascii_lowercase();
    let own =
        [format!("{asset}.sha256"), format!("{asset}.sha256sum")].map(|n| n.to_ascii_lowercase());
    release
        .files
        .iter()
        .find(|f| own.contains(&lower(f)))
        .or_else(|| {
            release.files.iter().find(|f| {
                let n = lower(f);
                n != asset.to_ascii_lowercase()
                    && (n.contains("sha256")
                        || n == "checksums.txt"
                        || n.ends_with("-checksums.txt"))
                    && f.size < 1 << 20
            })
        })
}

/// Finds `asset`'s hash in a checksum file. Understands `<hex>  <name>` (GNU, with `*` for
/// binary mode), `SHA256 (<name>) = <hex>` (BSD), and a file holding only one hash.
pub fn parse_sums(text: &str, asset: &str) -> Option<String> {
    let is_hash = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    for line in &lines {
        if let Some(rest) = line.strip_prefix("SHA256 (")
            && let Some((name, hash)) = rest.split_once(") = ")
            && base_name(name) == asset
            && is_hash(hash.trim())
        {
            return Some(hash.trim().to_ascii_lowercase());
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        if let (Some(hash), Some(name)) = (parts.next(), parts.next()) {
            let name = name.trim().trim_start_matches('*');
            if is_hash(hash) && base_name(name) == asset {
                return Some(hash.to_ascii_lowercase());
            }
        }
    }
    match lines.as_slice() {
        [only] => only
            .split_whitespace()
            .next()
            .filter(|h| is_hash(h))
            .map(str::to_ascii_lowercase),
        _ => None,
    }
}

fn base_name(name: &str) -> &str {
    name.rsplit(['/', '\\']).next().unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

    #[test]
    fn parses_common_formats() {
        assert_eq!(
            parse_sums(&format!("{H}  Example.zip\n"), "Example.zip").as_deref(),
            Some(H)
        );
        assert_eq!(
            parse_sums(&format!("{H} *dist/Example.zip"), "Example.zip").as_deref(),
            Some(H)
        );
        assert_eq!(
            parse_sums(&format!("SHA256 (Example.zip) = {H}"), "Example.zip").as_deref(),
            Some(H)
        );
        assert_eq!(
            parse_sums(&H.to_uppercase(), "Example.zip").as_deref(),
            Some(H)
        );
        let two = format!("{H}  Other.zip\n{H}  Third.zip\n");
        assert_eq!(parse_sums(&two, "Example.zip"), None);
    }

    #[test]
    fn hashes_files() -> Result<()> {
        let dir = tempfile::tempdir().at("tempdir")?;
        let p = dir.path().join("t");
        std::fs::write(&p, b"test").at(&p)?;
        assert_eq!(sha256_file(&p)?, H);
        Ok(())
    }
}
