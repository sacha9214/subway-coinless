//! File reads that also work in the browser.
//!
//! Natively these are `std::fs`. The web build has no file system: the
//! files the game reads at start-up are packed into the binary
//! (`web/pack.bin`, written by `tools/web_pack.py`) under the paths
//! [`crate::install::Install::locate`] gives there (`/site/...`,
//! `/port/...`).

use std::path::Path;

#[cfg(target_arch = "wasm32")]
mod pack {
    use std::collections::BTreeMap;
    use std::sync::OnceLock;

    static PACK: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/pack.bin"));

    /// path -> bytes. Layout: entries of u16 path length, path, u32 length, bytes.
    pub fn files() -> &'static BTreeMap<String, &'static [u8]> {
        static FILES: OnceLock<BTreeMap<String, &'static [u8]>> = OnceLock::new();
        FILES.get_or_init(|| {
            let mut m = BTreeMap::new();
            let mut i = 0;
            while i < PACK.len() {
                let n = u16::from_le_bytes([PACK[i], PACK[i + 1]]) as usize;
                let name = std::str::from_utf8(&PACK[i + 2..i + 2 + n]).unwrap().to_string();
                i += 2 + n;
                let len = u32::from_le_bytes([PACK[i], PACK[i + 1], PACK[i + 2], PACK[i + 3]]) as usize;
                m.insert(name, &PACK[i + 4..i + 4 + len]);
                i += 4 + len;
            }
            m
        })
    }

    /// `a/./b/../c` -> `a/c`.
    pub fn key(path: &std::path::Path) -> String {
        let mut parts: Vec<&str> = Vec::new();
        for p in path.to_str().unwrap_or("").split('/') {
            match p {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                p => parts.push(p),
            }
        }
        format!("/{}", parts.join("/"))
    }
}

pub fn read(path: impl AsRef<Path>) -> std::io::Result<Vec<u8>> {
    #[cfg(not(target_arch = "wasm32"))]
    return std::fs::read(path);
    #[cfg(target_arch = "wasm32")]
    pack::files().get(&pack::key(path.as_ref())).map(|b| b.to_vec()).ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, pack::key(path.as_ref())))
}

pub fn read_to_string(path: impl AsRef<Path>) -> std::io::Result<String> {
    #[cfg(not(target_arch = "wasm32"))]
    return std::fs::read_to_string(path);
    #[cfg(target_arch = "wasm32")]
    read(path).and_then(|b| String::from_utf8(b).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)))
}

/// The files directly in `dir` (full paths), sorted.
pub fn list(dir: impl AsRef<Path>) -> Vec<std::path::PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut v: Vec<_> = std::fs::read_dir(dir).map(|d| d.flatten().map(|e| e.path()).collect()).unwrap_or_default();
        v.sort();
        v
    }
    #[cfg(target_arch = "wasm32")]
    {
        let prefix = format!("{}/", pack::key(dir.as_ref()));
        pack::files().keys().filter(|k| k.strip_prefix(&prefix).is_some_and(|rest| !rest.contains('/'))).map(std::path::PathBuf::from).collect()
    }
}

/// Seconds since the Unix epoch.
pub fn now_secs() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    return std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    #[cfg(target_arch = "wasm32")]
    return (js_sys::Date::now() / 1000.0) as u64;
}
