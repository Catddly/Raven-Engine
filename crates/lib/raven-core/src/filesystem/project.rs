use std::path::PathBuf;

use parking_lot::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    pub(super) static ref CUSTUM_MOUNT_POINT: Mutex<CustomProjectMountPoint> = Mutex::new(CustomProjectMountPoint::new());
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ProjectFolder(pub(crate) i32);

impl ProjectFolder {
    #[allow(non_upper_case_globals)]
    pub const Root: Self = Self(-1);
    #[allow(non_upper_case_globals)]
    pub const Log: Self = Self(0);
    #[allow(non_upper_case_globals)]
    pub const Assets: Self = Self(1);
    #[allow(non_upper_case_globals)]
    pub const Scenes: Self = Self(2);
    #[allow(non_upper_case_globals)]
    pub const ShaderSource: Self = Self(3);
    #[allow(non_upper_case_globals)]
    pub const ShaderBinary: Self = Self(4);
    #[allow(non_upper_case_globals)]
    pub const Template: Self = Self(5);
    #[allow(non_upper_case_globals)]
    pub const Vendor: Self = Self(6);
    #[allow(non_upper_case_globals)]
    pub const Baked: Self = Self(7);

    fn to_raw(&self) -> i32 {
        self.0
    }

    #[allow(dead_code)]
    fn from_raw(v: i32) -> Self {
        Self(v)
    }
}

pub struct CustomProjectMountPoint {
    pub mount_points: [Option<PathBuf>; 8],
}

impl CustomProjectMountPoint {
    pub fn new() -> Self {
        Self {
            mount_points: Default::default(),
        }
    }

    pub fn set_custom_mount_point(&mut self, pf: ProjectFolder, path: PathBuf) {
        match pf {
            ProjectFolder::Root => panic!("Cannot reset custom mount point for root here, call set_root_path() instead!"),
            ProjectFolder::Log |
            ProjectFolder::Assets |
            ProjectFolder::Scenes |
            ProjectFolder::ShaderSource |
            ProjectFolder::ShaderBinary |
            ProjectFolder::Template |
            ProjectFolder::Vendor |
            ProjectFolder::Baked => self.mount_points[pf.to_raw() as usize] = Some(path),
            _ => panic!("Invalid project folder!"),
        }
    }

    fn get_custom_mount_point(&self, pf: ProjectFolder) -> Option<&PathBuf> {
        match pf {
            ProjectFolder::Root => panic!("Cannot get custom mount point for root here, call root_path() instead!"),
            ProjectFolder::Log |
            ProjectFolder::Assets |
            ProjectFolder::Scenes |
            ProjectFolder::ShaderSource |
            ProjectFolder::ShaderBinary |
            ProjectFolder::Template |
            ProjectFolder::Vendor |
            ProjectFolder::Baked => self.mount_points[pf.to_raw() as usize].as_ref(),
            _ => panic!("Invalid project folder!"),
        }
    }
}

fn get_project_folder_path_impl(root: &PathBuf, folder: ProjectFolder) -> PathBuf {
    match folder {
        ProjectFolder::Root => root.to_owned(),
        ProjectFolder::Log |
        ProjectFolder::Assets |
        ProjectFolder::Scenes |
        ProjectFolder::ShaderSource |
        ProjectFolder::ShaderBinary |
        ProjectFolder::Template |
        ProjectFolder::Vendor |
        ProjectFolder::Baked => {
            if let Some(path) = CUSTUM_MOUNT_POINT.lock().get_custom_mount_point(folder) {
                path.canonicalize().unwrap().to_owned()
            } else {
                match folder {
                    ProjectFolder::Log => root.join("log"),
                    ProjectFolder::Assets => root.join("assets"),
                    ProjectFolder::Scenes => root.join("scenes"),
                    ProjectFolder::ShaderSource => root.join("shader_src"),
                    ProjectFolder::ShaderBinary => root.join("shader_bin"),
                    ProjectFolder::Template => root.join("template"),
                    ProjectFolder::Vendor => root.join("vendor"),
                    ProjectFolder::Baked => root.join("baked"),
                    _ => unreachable!(),
                }
            }
        }
        _ => panic!("Invalid project folder!"),
    }
}

pub(super) fn get_project_folder_path(root: &PathBuf, folder: ProjectFolder) -> PathBuf {
    get_project_folder_path_impl(root, folder)
}