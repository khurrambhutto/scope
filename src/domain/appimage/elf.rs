//! Read-only ELF inspection of the AppImage runtime.
//!
//! A type 2 AppImage is an ELF runtime followed by a SquashFS image. The
//! runtime carries `.upd_info` (update information) and the image starts where
//! the ELF section header table ends. Nothing here executes the file.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{bail, Context, Result};

/// Section headers beyond this count are not a real AppImage runtime.
const MAX_SECTIONS: u64 = 512;
/// `.upd_info` is a fixed 1 KiB slot; anything far larger is not update text.
const MAX_UPDATE_INFO: u64 = 8 * 1024;
const MAX_SHSTRTAB: u64 = 64 * 1024;

/// Facts read from the runtime without running it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeInfo {
    /// Byte offset where the SquashFS payload begins.
    pub payload_offset: u64,
    /// Trimmed `.upd_info` contents, when present and non-empty.
    pub update_info: Option<String>,
    /// `x86_64`, `aarch64`, or another machine label.
    pub architecture: &'static str,
}

/// AppImage format from the magic bytes at offset 8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppImageType {
    One,
    Two,
}

pub fn appimage_type(path: &Path) -> Option<AppImageType> {
    let mut buf = [0u8; 11];
    std::fs::File::open(path).ok()?.read_exact(&mut buf).ok()?;
    if &buf[0..4] != b"\x7fELF" || buf[8] != 0x41 || buf[9] != 0x49 {
        return None;
    }
    match buf[10] {
        1 => Some(AppImageType::One),
        2 => Some(AppImageType::Two),
        _ => None,
    }
}

struct Header {
    is_64: bool,
    little: bool,
}

impl Header {
    fn u16(&self, b: &[u8]) -> u16 {
        let a = [b[0], b[1]];
        if self.little {
            u16::from_le_bytes(a)
        } else {
            u16::from_be_bytes(a)
        }
    }
    fn u32(&self, b: &[u8]) -> u32 {
        let a = [b[0], b[1], b[2], b[3]];
        if self.little {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        }
    }
    fn u64(&self, b: &[u8]) -> u64 {
        let mut a = [0u8; 8];
        a.copy_from_slice(&b[..8]);
        if self.little {
            u64::from_le_bytes(a)
        } else {
            u64::from_be_bytes(a)
        }
    }
    fn word(&self, b: &[u8]) -> u64 {
        if self.is_64 {
            self.u64(b)
        } else {
            u64::from(self.u32(b))
        }
    }
}

struct Section {
    name: u32,
    offset: u64,
    size: u64,
}

/// Inspect the ELF runtime of an AppImage.
pub fn inspect(path: &Path) -> Result<RuntimeInfo> {
    let mut file =
        std::fs::File::open(path).with_context(|| format!("Could not open {}", path.display()))?;
    let file_len = file.metadata()?.len();
    let mut ident = [0u8; 64];
    file.read_exact(&mut ident)
        .context("File is too small to be an AppImage")?;
    if &ident[0..4] != b"\x7fELF" {
        bail!("Not an ELF executable");
    }
    let header = Header {
        is_64: match ident[4] {
            1 => false,
            2 => true,
            _ => bail!("Unknown ELF class"),
        },
        little: match ident[5] {
            1 => true,
            2 => false,
            _ => bail!("Unknown ELF byte order"),
        },
    };
    let machine = header.u16(&ident[18..20]);
    let (shoff, shentsize, shnum, shstrndx) = if header.is_64 {
        (
            header.u64(&ident[40..48]),
            u64::from(header.u16(&ident[58..60])),
            u64::from(header.u16(&ident[60..62])),
            u64::from(header.u16(&ident[62..64])),
        )
    } else {
        (
            u64::from(header.u32(&ident[32..36])),
            u64::from(header.u16(&ident[46..48])),
            u64::from(header.u16(&ident[48..50])),
            u64::from(header.u16(&ident[50..52])),
        )
    };
    let min_entry = if header.is_64 { 64 } else { 40 };
    if shnum == 0 || shnum > MAX_SECTIONS || shentsize < min_entry || shstrndx >= shnum {
        bail!("AppImage runtime has no usable section table");
    }
    let payload_offset = shoff
        .checked_add(shentsize * shnum)
        .filter(|end| *end < file_len)
        .context("AppImage payload offset is outside the file")?;

    let mut table = vec![0u8; (shentsize * shnum) as usize];
    file.seek(SeekFrom::Start(shoff))?;
    file.read_exact(&mut table)?;
    let sections: Vec<Section> = table
        .chunks_exact(shentsize as usize)
        .map(|entry| {
            if header.is_64 {
                Section {
                    name: header.u32(&entry[0..4]),
                    offset: header.u64(&entry[24..32]),
                    size: header.u64(&entry[32..40]),
                }
            } else {
                Section {
                    name: header.u32(&entry[0..4]),
                    offset: header.word(&entry[16..20]),
                    size: header.word(&entry[20..24]),
                }
            }
        })
        .collect();

    let names = &sections[shstrndx as usize];
    let names = read_range(&mut file, file_len, names.offset, names.size, MAX_SHSTRTAB)?;
    let update_info = sections
        .iter()
        .find(|section| section_name(&names, section.name) == Some(b".upd_info".as_slice()))
        .map(|section| {
            read_range(
                &mut file,
                file_len,
                section.offset,
                section.size,
                MAX_UPDATE_INFO,
            )
        })
        .transpose()?
        .and_then(|bytes| {
            let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
            let text = String::from_utf8_lossy(&bytes[..end]).trim().to_string();
            (!text.is_empty()).then_some(text)
        });

    Ok(RuntimeInfo {
        payload_offset,
        update_info,
        architecture: match machine {
            0x3e => "x86_64",
            0xb7 => "aarch64",
            0x03 => "i686",
            0x28 => "armhf",
            _ => "unknown",
        },
    })
}

fn read_range(
    file: &mut std::fs::File,
    file_len: u64,
    offset: u64,
    size: u64,
    limit: u64,
) -> Result<Vec<u8>> {
    if size > limit || offset.checked_add(size).is_none_or(|end| end > file_len) {
        bail!("AppImage section is out of bounds");
    }
    let mut bytes = vec![0u8; size as usize];
    file.seek(SeekFrom::Start(offset))?;
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn section_name(names: &[u8], index: u32) -> Option<&[u8]> {
    let start = index as usize;
    let rest = names.get(start..)?;
    let end = rest.iter().position(|b| *b == 0)?;
    Some(&rest[..end])
}

/// Build a minimal little-endian ELF64 AppImage for tests.
#[cfg(test)]
pub(crate) fn test_appimage(update_info: &str, payload: &[u8]) -> Vec<u8> {
    let names = b"\0.shstrtab\0.upd_info\0";
    let mut info = update_info.as_bytes().to_vec();
    info.resize(1024, 0);
    let names_offset = 64u64;
    let info_offset = names_offset + names.len() as u64;
    let shoff = info_offset + info.len() as u64;
    let mut out = vec![0u8; 64];
    out[0..4].copy_from_slice(b"\x7fELF");
    out[4] = 2;
    out[5] = 1;
    out[6] = 1;
    out[8] = 0x41;
    out[9] = 0x49;
    out[10] = 0x02;
    out[18..20].copy_from_slice(&0x3eu16.to_le_bytes());
    out[40..48].copy_from_slice(&shoff.to_le_bytes());
    out[58..60].copy_from_slice(&64u16.to_le_bytes());
    out[60..62].copy_from_slice(&3u16.to_le_bytes());
    out[62..64].copy_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(names);
    out.extend_from_slice(&info);
    let section = |name: u32, offset: u64, size: u64| {
        let mut entry = vec![0u8; 64];
        entry[0..4].copy_from_slice(&name.to_le_bytes());
        entry[24..32].copy_from_slice(&offset.to_le_bytes());
        entry[32..40].copy_from_slice(&size.to_le_bytes());
        entry
    };
    out.extend(vec![0u8; 64]);
    out.extend(section(1, names_offset, names.len() as u64));
    out.extend(section(11, info_offset, info.len() as u64));
    out.extend_from_slice(payload);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_update_information_and_payload_offset() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.AppImage");
        let bytes = test_appimage(
            "gh-releases-zsync|o|r|latest|App-*x86_64.AppImage.zsync",
            b"hsqs",
        );
        std::fs::write(&path, &bytes).unwrap();

        let info = inspect(&path).unwrap();
        assert_eq!(
            info.update_info.as_deref(),
            Some("gh-releases-zsync|o|r|latest|App-*x86_64.AppImage.zsync")
        );
        assert_eq!(&bytes[info.payload_offset as usize..], b"hsqs");
        assert_eq!(info.architecture, "x86_64");
        assert_eq!(appimage_type(&path), Some(AppImageType::Two));
    }

    #[test]
    fn empty_update_information_is_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.AppImage");
        std::fs::write(&path, test_appimage("", b"hsqs")).unwrap();
        assert_eq!(inspect(&path).unwrap().update_info, None);
    }

    #[test]
    fn rejects_truncated_or_non_elf_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.AppImage");
        std::fs::write(
            &path,
            b"not an elf at all, but long enough to read a header from it..........",
        )
        .unwrap();
        assert!(inspect(&path).is_err());
        let mut bytes = test_appimage("", b"");
        bytes.truncate(200);
        std::fs::write(&path, bytes).unwrap();
        assert!(inspect(&path).is_err());
    }
}
