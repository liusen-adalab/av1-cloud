use anyhow::Result;
use sha2::Digest;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;
use tokio::{fs, io::AsyncWriteExt, task::spawn_blocking};
use tracing::{debug, warn};

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
        spawn_blocking(move || -> Result<()> {
            let tmp_file = self.tmp_file.path();

            // NamedTempFile 和 path 不在同一个文件系统，不能直接 rename
            // NamedTempFile 会在 drop 时自动删除
            std::fs::copy(tmp_file, path)?;

            Ok(())
        })
        .await??;
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

    spawn_blocking(move || {
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

/// delete a file or directory if exists
pub async fn delete(path: &Path) -> Result<()> {
    let path = path.to_owned();
    spawn_blocking(move || delete_inner(&path)).await??;
    Ok(())
}

fn delete_inner(path: &Path) -> Result<()> {
    use std::fs;

    let meta = nx_is_ok!(fs::symlink_metadata(path).or_else(|_err| fs::metadata(path)));
    if meta.is_file() {
        tracing::debug!(?path, "removing file");
        fs::remove_file(path).ignore_nx()?;
    } else {
        tracing::debug!(?path, "removing directory or symlink");
        fs::remove_dir_all(path).ignore_nx()?;
    }

    tracing::debug!(?path, "removed");
    Ok(())
}

/// link file only
pub async fn create_user_link(src: &Path, owner: &VirtualPath) -> Result<()> {
    let owner = PathManager::virtual_to_disk(owner);

    let src = src.to_owned();
    spawn_blocking(move || create_file_link(&src, &owner)).await??;

    Ok(())
}

fn create_file_link(src: &Path, owner: &Path) -> Result<()> {
    delete_inner(&owner)?;

    debug!(?src, ?owner, "creating user link");

    #[cfg(target_family = "unix")]
    std::os::unix::fs::symlink(&src, owner)?;

    #[cfg(target_family = "windows")]
    std::os::windows::fs::symlink_file(&src, owner)?;

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

pub async fn virtual_copy(from: &VirtualPath, to: &VirtualPath) -> Result<()> {
    let from = PathManager::virtual_to_disk(from);
    let to = PathManager::virtual_to_disk(to);

    delete(&to).await?;
    spawn_blocking(move || copy_dir_all(&from, &to)).await??;

    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    use std::fs;

    let meta = fs::symlink_metadata(src).or_else(|_err| std::fs::metadata(src))?;
    if meta.is_dir() {
        let dir = fs::read_dir(src)?;
        fs::create_dir(dst)?;
        for entry in dir {
            let entry = entry?;
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        }
    } else if meta.is_symlink() {
        let src = fs::read_link(src)?;
        create_file_link(&src, dst)?;
    } else {
        warn!(?src, ?dst, "copying file");
        fs::copy(src, dst)?;
    }
    Ok(())
}
