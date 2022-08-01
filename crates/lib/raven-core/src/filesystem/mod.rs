use std::path::PathBuf;
use std::env::{set_current_dir, current_exe};
use anyhow::bail;

mod project;
pub use project::ProjectFolder as ProjectFolder;

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
pub fn exist_or_create(pf: &ProjectFolder) -> anyhow::Result<()> {
    let folder_path = project_folder_path(&pf)?;
    if !folder_path.as_path().exists() {
        std::fs::create_dir(folder_path)?;
    }

    Ok(())
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