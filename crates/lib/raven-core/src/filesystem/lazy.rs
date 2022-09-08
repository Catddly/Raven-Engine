use std::path::PathBuf;
use std::fs::File;
use std::io::Write;

use anyhow::Context;
use bytes::Bytes;
use turbosloth::*;

use super::{FILE_HOT_WATCHER, ProjectFolder};

#[derive(Clone, Hash)]
pub struct LoadFile {
    path: PathBuf,
}

impl LoadFile {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
        }
    }
}

#[async_trait]
impl LazyWorker for LoadFile {
    type Output = anyhow::Result<Bytes>;

    async fn run(self, ctx: RunContext) -> Self::Output {
        let invalidate_trigger = ctx.get_invalidation_trigger();

        FILE_HOT_WATCHER
            .lock()
            .unwrap()
            .watch(self.path.clone(), move |event| {
                if matches!(event, hotwatch::Event::Write(_) | hotwatch::Event::NoticeWrite(_)) {
                    // The period between LoadFile begin to run on another thread and loading from the file,
                    // the file might be changed by someone accidentally.
                    // When this happened, we need to invalidate this loading operation, and start a new one.
                    invalidate_trigger();

                    // will this watch forever?
                }
            })
            .with_context(|| format!("Failed to watch file {:?}!", self.path))?;

        let mut buffer = Vec::new();
        std::io::Read::read_to_end(&mut File::open(&self.path)?, &mut buffer)
            .with_context(|| format!("Failed to read file {:?}", self.path))?;

        Ok(Bytes::from(buffer))
    }
}

pub struct StoreFile {
    bytes: Bytes,
    folder: ProjectFolder,
    name: PathBuf,
}

impl StoreFile {
    pub fn new(bytes: Bytes, folder: ProjectFolder, name: PathBuf) -> Self {
        Self {
            bytes,
            folder,
            name,
        }
    }
}

#[async_trait]
impl LazyWorker for StoreFile {
    type Output = anyhow::Result<PathBuf>;

    async fn run(self, _ctx: RunContext) -> Self::Output {
        let mut path = super::project_folder_path(&self.folder).unwrap();
        path.extend(self.name.iter());
        assert!(path.is_file());

        let mut file = if std::path::Path::try_exists(&path)? {
            std::fs::File::open(path.clone()).with_context(|| format!("Failed to open file: {:?}", path))?
        } else {
            std::fs::File::create(path.clone()).with_context(|| format!("Failed to create file: {:?}", path))?
        };

        let bytes = self.bytes.to_vec();
        let bytes = bytes.as_slice();
        file.write_all(bytes)
            .with_context(|| format!("Failed to write out file: {:?}", path))?;
        Ok(self.name)
    }
}