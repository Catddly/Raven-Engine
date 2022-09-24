use std::path::PathBuf;
use std::env;
use std::time::Duration;

use parking_lot::Mutex;
use anyhow::bail;
use lazy_static::lazy_static;
use hotwatch::Hotwatch;

mod project;
pub mod lazy;
pub use project::ProjectFolder as ProjectFolder;
use project::CUSTUM_MOUNT_POINT;

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

pub fn set_custom_mount_point(pf: ProjectFolder, relative_path: impl Into<PathBuf>) -> anyhow::Result<()> {
    let relative_path = relative_path.into();
    if relative_path.is_absolute() {
        anyhow::bail!("Try to set custom mount point with absolute path!")
    }

    CUSTUM_MOUNT_POINT.lock().set_custom_mount_point(pf, relative_path);
    Ok(())
}

fn to_relative(path: PathBuf) -> anyhow::Result<PathBuf> {
    if path.is_absolute() {
        let current_dir = root_path()?;
        if !path.starts_with(current_dir.clone()) {
            anyhow::bail!("Pass in absolute path is an invalid path for engine!");
        }

        let path = path.strip_prefix(current_dir).unwrap();
        Ok(path.to_path_buf())
    } else {
        anyhow::bail!("Pass in a relative path, required absolute path!");
    }
}

/// Get the absolute path of ProjectFolder pf.
#[inline]
pub fn get_project_folder_path_absolute(pf: ProjectFolder) -> anyhow::Result<PathBuf> {
    Ok(project::get_project_folder_path(&root_path()?, pf))
}

/// Get the relative path of ProjectFolder pf.
#[inline]
pub fn get_project_folder_path_relative(pf: ProjectFolder) -> anyhow::Result<PathBuf> {
    Ok(to_relative(project::get_project_folder_path(&root_path()?, pf))?)
}

/// Set current engine root path to the path where the .exe exists.
pub fn set_default_root_path() -> anyhow::Result<()> {
    let exe_path = env::current_exe().expect("Failed to fetch vaild exe path!");
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
    let folder_path = get_project_folder_path_absolute(pf)?;
    if !folder_path.as_path().exists() {
        std::fs::create_dir(folder_path)?;
    }

    Ok(())
}

/// Check if a file exists.
pub fn exist(file: &PathBuf, folder: ProjectFolder) -> anyhow::Result<bool> {
    assert!(file.is_file());

    let mut folder_path = get_project_folder_path_absolute(folder)?;
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
        env::set_current_dir(p)?
    }
    Ok(())
}