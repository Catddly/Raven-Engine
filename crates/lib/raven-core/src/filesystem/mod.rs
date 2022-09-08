use std::path::PathBuf;
use std::env::{set_current_dir, current_exe};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::bail;
use lazy_static::lazy_static;
use hotwatch::Hotwatch;

mod project;
pub mod lazy;
pub use project::ProjectFolder as ProjectFolder;

lazy_static! {
    pub(crate) static ref FILE_HOT_WATCHER: Mutex<Hotwatch> = Mutex::new(Hotwatch::new_with_custom_delay(Duration::from_millis(200)).unwrap());
}

/// Get the root path of the engine.
#[inline]
pub fn root_path() -> anyhow::Result<PathBuf> {
    Ok(std::env::current_dir()?)
}


/// Set the root path of the engine.
pub fn set_root_path(p: impl Into<PathBuf>) -> anyhow::Result<()> {
    let p = p.into();
    if !p.is_dir() {
        bail!("{} is not a root path!", p.display())
    }
    mount(p)?;
    
    Ok(())
}

/// Get the absolute path of ProjectFolder pf.
#[inline]
pub fn project_folder_path(pf: &ProjectFolder) -> anyhow::Result<PathBuf> {
    Ok(project::get_project_folder_path(&root_path()?, pf))
}

/// Set current engine root path to the path where the .exe exists.
pub fn set_default_root_path() -> anyhow::Result<()> {
    let exe_path = current_exe().expect("Failed to fetch vaild exe path!");
    let mut ancestors = exe_path.ancestors();
    ancestors.next();

    let root_path = match ancestors.next() {
        Some(path) => path.to_owned(),
        None => panic!("Failed to fetch vaild exe path!"),
    };
    
    mount(root_path)?;
    Ok(())
}

/// Check if ProjectFolder pf exists, if not, create a empty folder.
pub fn exist_or_create(pf: ProjectFolder) -> anyhow::Result<()> {
    let folder_path = project_folder_path(&pf)?;
    if !folder_path.as_path().exists() {
        std::fs::create_dir(folder_path)?;
    }

    Ok(())
}

/// Check if a file exists.
pub fn exist(file: &PathBuf, folder: ProjectFolder) -> anyhow::Result<bool> {
    assert!(file.is_file());

    let mut folder_path = project_folder_path(&folder)?;
    folder_path.extend(file.iter());

    // to avoid symbolic links changed maliciously by someone
    Ok(std::path::Path::try_exists(&folder_path)?)
}

/// Mount engine root path to p.
fn mount(p: impl Into<PathBuf>) -> anyhow::Result<()> {
    let p = p.into();
    if !p.is_dir() {
        bail!("Invalid root path: {}", p.display())
    } else {
        set_current_dir(p)?
    }
    Ok(())
}