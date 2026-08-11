//! Content-addressed immutable artifacts for governed Plugin releases.

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

use conductor_domain::DraftFile;
use sha2::{Digest, Sha256};

const ARTIFACT_DIR: &str = "artifacts/sha256";
const STAGING_DIR: &str = "staging";

#[derive(Debug, Clone)]
pub struct ArtifactStore {
    root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StoredArtifact {
    pub key: String,
    pub sha256: String,
    pub size: u64,
}

impl ArtifactStore {
    pub fn from_env() -> Self {
        let root = std::env::var("CONDUCTOR_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("data"));
        Self { root }
    }

    #[cfg(test)]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn put_plugin(&self, files: &[DraftFile]) -> std::io::Result<StoredArtifact> {
        let bytes = deterministic_zip(files)?;
        self.put(&bytes)
    }

    pub fn put(&self, bytes: &[u8]) -> std::io::Result<StoredArtifact> {
        let digest = hex::encode(Sha256::digest(bytes));
        let key = format!("sha256/{}/{}", &digest[..2], digest);
        let target = self
            .root
            .join(ARTIFACT_DIR)
            .join(&digest[..2])
            .join(&digest);
        if !target.exists() {
            let staging = self.root.join(ARTIFACT_DIR).join(STAGING_DIR);
            fs::create_dir_all(&staging)?;
            fs::create_dir_all(target.parent().expect("artifact parent"))?;
            let temporary = staging.join(format!("{}.{}.tmp", digest, uuid::Uuid::new_v4()));
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            match fs::rename(&temporary, &target) {
                Ok(()) => {}
                Err(error) if target.exists() => {
                    let _ = fs::remove_file(&temporary);
                    if !target.exists() {
                        return Err(error);
                    }
                }
                Err(error) => return Err(error),
            }
        }
        Ok(StoredArtifact {
            key,
            sha256: digest,
            size: bytes.len().try_into().unwrap_or(u64::MAX),
        })
    }

    pub fn read(&self, key: &str) -> std::io::Result<Vec<u8>> {
        let digest = key
            .strip_prefix("sha256/")
            .and_then(|value| value.split_once('/'))
            .filter(|(prefix, digest)| {
                prefix.len() == 2
                    && digest.len() == 64
                    && digest.starts_with(prefix)
                    && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
            .map(|(_, digest)| digest)
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid artifact key")
            })?;
        let path = self.root.join(ARTIFACT_DIR).join(&digest[..2]).join(digest);
        let mut file = OpenOptions::new().read(true).open(path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}

fn deterministic_zip(files: &[DraftFile]) -> std::io::Result<Vec<u8>> {
    let mut files = files.to_vec();
    files.sort_by(|left, right| left.path.cmp(&right.path));
    if files.len() > u16::MAX as usize {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "too many files",
        ));
    }
    let mut output = Vec::new();
    let mut central = Vec::new();
    for file in &files {
        let name = file.path.as_bytes();
        let body = file.content.as_bytes();
        let name_len = u16::try_from(name.len())
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "path too long"))?;
        let body_len = u32::try_from(body.len())
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "file too large"))?;
        let offset = u32::try_from(output.len()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "archive too large")
        })?;
        let crc = crc32(body);

        push_u32(&mut output, 0x0403_4b50);
        push_u16(&mut output, 20);
        push_u16(&mut output, 0x0800);
        push_u16(&mut output, 0);
        push_u16(&mut output, 0);
        push_u16(&mut output, 0);
        push_u32(&mut output, crc);
        push_u32(&mut output, body_len);
        push_u32(&mut output, body_len);
        push_u16(&mut output, name_len);
        push_u16(&mut output, 0);
        output.extend_from_slice(name);
        output.extend_from_slice(body);

        push_u32(&mut central, 0x0201_4b50);
        push_u16(&mut central, 20);
        push_u16(&mut central, 20);
        push_u16(&mut central, 0x0800);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u32(&mut central, crc);
        push_u32(&mut central, body_len);
        push_u32(&mut central, body_len);
        push_u16(&mut central, name_len);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u32(&mut central, 0);
        push_u32(&mut central, offset);
        central.extend_from_slice(name);
    }
    let central_offset = u32::try_from(output.len())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "archive too large"))?;
    let central_size = u32::try_from(central.len())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "archive too large"))?;
    output.extend_from_slice(&central);
    push_u32(&mut output, 0x0605_4b50);
    push_u16(&mut output, 0);
    push_u16(&mut output, 0);
    push_u16(&mut output, files.len() as u16);
    push_u16(&mut output, files.len() as u16);
    push_u32(&mut output, central_size);
    push_u32(&mut output, central_offset);
    push_u16(&mut output, 0);
    Ok(output)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_is_deterministic_and_has_zip_markers() {
        let files = vec![
            DraftFile {
                path: "plugin.json".into(),
                content: "{}".into(),
            },
            DraftFile {
                path: "skills/demo/SKILL.md".into(),
                content: "demo".into(),
            },
        ];
        let first = deterministic_zip(&files).unwrap();
        let second = deterministic_zip(&files.into_iter().rev().collect::<Vec<_>>()).unwrap();
        assert_eq!(first, second);
        assert_eq!(&first[..4], &[0x50, 0x4b, 0x03, 0x04]);
        assert!(first
            .windows(4)
            .any(|window| window == [0x50, 0x4b, 0x05, 0x06]));
    }
}
