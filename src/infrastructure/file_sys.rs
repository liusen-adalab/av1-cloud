use anyhow::Result;
use sha2::Digest;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;
use tokio::{fs, io::AsyncWriteExt};
use tracing::debug;

use crate::domain::file_system::{file::VirtualPath, service::PathManager};

pub struct UploadFileSlice<'a> {
    pub index: u32,
    pub data: &'a [u8],
    pub dir: &'a Path,
}

fn slice_file_name(idx: u32) -> String {
    format!("part-{}", idx)
}

fn slice_file_path(dir: &Path, idx: u32) -> PathBuf {
    dir.join(slice_file_name(idx))
}

pub async fn store_slice(slice: UploadFileSlice<'_>) -> Result<()> {
    let path = slice_file_path(&slice.dir, slice.index);
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&path)
        .await?;
    file.write_all(slice.data).await?;

    Ok(())
}

pub struct MergedFile {
    pub hash: String,
    pub size: u64,
    pub tmp_file: NamedTempFile,
}

impl MergedFile {
    pub async fn persist(self, path: &Path) -> Result<()> {
        let path = path.to_owned();
        tokio::task::spawn_blocking(move || self.tmp_file.persist(path)).await??;
        Ok(())
    }
}

pub async fn merge_slices(slice_dir: &Path) -> Result<Option<MergedFile>> {
    debug!("merging slices");
    let mut hasher = sha2::Sha256::new();
    let mut size = 0;
    let slices = load_slices_sorted(&slice_dir).await?;
    if slices.is_empty() {
        return Ok(None);
    }

    tokio::task::spawn_blocking(move || {
        let mut dst_file = NamedTempFile::new()?;
        for slice in slices {
            let mut data = vec![];
            let mut slice = std::fs::File::open(slice)?;
            slice.read_to_end(&mut data)?;

            size += data.len();
            hasher.update(&data);

            dst_file.write_all(&data)?;
        }
        let hash = hex::encode(hasher.finalize());
        Ok(Some(MergedFile {
            hash,
            size: size as u64,
            tmp_file: dst_file,
        }))
    })
    .await?
}

async fn load_slices_sorted(dir: &Path) -> Result<Vec<PathBuf>> {
    debug!(?dir, "reading slices");
    let mut dir = fs::read_dir(&dir).await?;
    let mut paths = vec![];
    while let Some(entry) = dir.next_entry().await? {
        if !entry.metadata().await?.is_dir() {
            paths.push(entry.path())
        }
    }
    paths.sort_by(|a, b| {
        let index_a: u32 = a
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .split('-')
            .last()
            .unwrap()
            .parse()
            .unwrap();

        let index_b: u32 = b
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .split('-')
            .last()
            .unwrap()
            .parse()
            .unwrap();
        index_a.cmp(&index_b)
    });
    let paths: Vec<_> = paths.into_iter().collect::<Vec<_>>();
    Ok(paths)
}

pub async fn virtual_delete(path: &VirtualPath) -> Result<()> {
    let path = PathManager::virtual_to_disk(path);
    delete(&path).await?;
    Ok(())
}

trait IgnoreNotExist {
    fn ignore_nx(self) -> Self;
}

impl IgnoreNotExist for std::io::Result<()> {
    fn ignore_nx(self) -> Self {
        match self {
            Ok(_) => Ok(()),
            Err(err) => match err.kind() {
                std::io::ErrorKind::NotFound => Ok(()),
                _ => Err(err),
            },
        }
    }
}

macro_rules! nx_is_ok {
    ($fs_op:expr) => {{
        match $fs_op {
            Ok(res) => res,
            Err(err) => match err.kind() {
                std::io::ErrorKind::NotFound => {
                    return Ok(());
                }
                _ => return { Err(err.into()) },
            },
        }
    }};
}

pub async fn delete(path: &Path) -> Result<()> {
    let meta = nx_is_ok!(fs::metadata(&path).await);
    if meta.is_file() {
        fs::remove_file(path).await.ignore_nx()?;
    } else {
        fs::remove_dir_all(path).await.ignore_nx()?;
    }
    Ok(())
}

pub async fn create_user_link(src: &Path, owner: &VirtualPath) -> Result<()> {
    let owner = PathManager::virtual_to_disk(owner);

    delete(&owner).await?;

    debug!(?src, ?owner, "creating user link");
    #[cfg(target_family = "unix")]
    fs::symlink(&src, owner).await?;

    #[cfg(target_family = "windows")]
    fs::symlink_file(&src, owner).await?;

    Ok(())
}

pub(crate) async fn create_dir(dir: &VirtualPath) -> Result<()> {
    let path = PathManager::virtual_to_disk(dir);
    create_dir_all(&path).await?;
    Ok(())
}

pub(crate) async fn create_dir_all(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).await?;
    Ok(())
}

pub(crate) async fn virtual_move(from: &VirtualPath, to: &VirtualPath) -> Result<()> {
    let from = PathManager::virtual_to_disk(from);
    let to = PathManager::virtual_to_disk(to);
    fs::rename(from, to).await?;
    Ok(())
}

pub(crate) async fn virtual_copy(from: &VirtualPath, to: &VirtualPath) -> Result<()> {
    let from = PathManager::virtual_to_disk(from);
    let to = PathManager::virtual_to_disk(to);
    fs::copy(from, to).await?;
    Ok(())
}
