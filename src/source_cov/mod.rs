use std::{
    collections::HashMap,
    fs::read,
    path::{Path, PathBuf},
};

use anyhow::Result;
use md5::compute;
use pdb::{FileChecksum, FileInfo};
use sha1::{Digest, Sha1};
use sha2::Sha256;
use simics::{debug, get_object};
use typed_path::{TypedComponent, TypedPath, UnixComponent, WindowsComponent};
use walkdir::WalkDir;

#[derive(Debug, Clone, Default)]
pub struct SourceCache {
    prefix_lookup: HashMap<Vec<String>, PathBuf>,
    md5_lookup: HashMap<Vec<u8>, PathBuf>,
    sha1_lookup: HashMap<Vec<u8>, PathBuf>,
    sha256_lookup: HashMap<Vec<u8>, PathBuf>,
}

impl SourceCache {
    pub fn new<P>(src_dir: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let mut prefix_lookup = HashMap::new();
        let mut md5_lookup = HashMap::new();
        let mut sha1_lookup = HashMap::new();
        let mut sha256_lookup = HashMap::new();

        let file_paths = WalkDir::new(src_dir)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| entry.path().to_path_buf())
            .collect::<Vec<_>>();

        for path in &file_paths {
            let contents = read(path)?;
            let md5 = compute(&contents).0.to_vec();
            let sha1 = Sha1::digest(&contents).to_vec();
            let sha256 = Sha256::digest(&contents).to_vec();
            md5_lookup.insert(md5, path.clone());
            sha1_lookup.insert(sha1, path.clone());
            sha256_lookup.insert(sha256, path.clone());
            let mut components = path
                .components()
                .filter_map(|c| {
                    if let std::path::Component::Normal(c) = c {
                        Some(c.to_string_lossy().to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            // Create a list of component lists starting from the full path, then the full path
            // minus the first component, then the full path minus the first two components, etc.
            // This is used to create a lookup table for the source files.
            while !components.is_empty() {
                prefix_lookup.insert(components.clone(), path.clone());
                components.remove(0);
            }
        }

        if let Ok(o) = get_object("tsffs") {
            debug!(o, "Cached {} source files", file_paths.len());
        }

        Ok(Self {
            prefix_lookup,
            md5_lookup,
            sha1_lookup,
            sha256_lookup,
        })
    }

    pub fn lookup_file_name_components(&self, file_name: &str) -> Option<&Path> {
        let mut file_name_components = TypedPath::derive(&file_name.to_string().to_string())
            .components()
            .filter_map(|c| match c {
                TypedComponent::Unix(u) => {
                    if let UnixComponent::Normal(c) = u {
                        String::from_utf8(c.to_vec()).ok()
                    } else {
                        None
                    }
                }
                TypedComponent::Windows(w) => {
                    if let WindowsComponent::Normal(c) = w {
                        String::from_utf8(c.to_vec()).ok()
                    } else {
                        None
                    }
                }
            })
            .collect::<Vec<_>>();

        while !file_name_components.is_empty() {
            if let Some(file_path) = self.prefix_lookup.get(&file_name_components) {
                return Some(file_path);
            }

            file_name_components.remove(0);
        }

        None
    }

    pub fn lookup_pdb(&self, file_info: &FileInfo, file_name: &str) -> Result<Option<&Path>> {
        Ok(match file_info.checksum {
            FileChecksum::None => self.lookup_file_name_components(file_name),
            FileChecksum::Md5(m) => self
                .md5_lookup
                .get(m)
                .map(|p| p.as_path())
                .or_else(|| self.lookup_file_name_components(file_name)),
            FileChecksum::Sha1(s1) => self
                .sha1_lookup
                .get(s1)
                .map(|p| p.as_path())
                .or_else(|| self.lookup_file_name_components(file_name)),
            FileChecksum::Sha256(s256) => self
                .sha256_lookup
                .get(s256)
                .map(|p| p.as_path())
                .or_else(|| self.lookup_file_name_components(file_name)),
        })
    }

    /// Look up a source file referenced from a DWARF line number program, mirroring
    /// `lookup_pdb`. DWARF5 line-number program headers may carry an optional
    /// `DW_LNCT_MD5` checksum entry per file (`gimli`'s `FileEntry::md5`, valid only
    /// when `LineProgramHeader::file_has_md5` returns `true`); when present, we try
    /// that checksum against the same MD5 table populated by `SourceCache::new`
    /// (which hashes every candidate source file with MD5/SHA1/SHA256 up front).
    /// DWARF doesn't specify SHA1/SHA256 file checksums, so there's nothing to try
    /// there. If there's no MD5 (DWARF <= 4, or a DWARF5 producer that omitted it,
    /// e.g. some EDK2 GCC5 builds), we fall back to the same format-agnostic
    /// path-suffix lookup PDB uses.
    pub fn lookup_dwarf(&self, md5: Option<&[u8; 16]>, file_name: &str) -> Result<Option<&Path>> {
        Ok(match md5 {
            Some(m) => self
                .md5_lookup
                .get(m.as_slice())
                .map(|p| p.as_path())
                .or_else(|| self.lookup_file_name_components(file_name)),
            None => self.lookup_file_name_components(file_name),
        })
    }
}
