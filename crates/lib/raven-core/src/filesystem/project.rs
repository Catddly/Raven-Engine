use std::path::PathBuf;

pub enum ProjectFolder {
    Root,
    Log,
    Assets,
    Scenes,
    ShaderSource,
    ShaderBinary,
    Template,
    Vendor,
}

pub(super) fn get_project_folder_path(root: &PathBuf, folder: &ProjectFolder) -> PathBuf {
    match folder {
        ProjectFolder::Root => root.to_owned(),
        ProjectFolder::Log => root.join("log"),
        ProjectFolder::Assets => root.join("assets"),
        ProjectFolder::Scenes => root.join("scenes"),
        ProjectFolder::ShaderSource => root.join("shader_src"),
        ProjectFolder::ShaderBinary => root.join("shader_bin"),
        ProjectFolder::Template => root.join("template"),
        ProjectFolder::Vendor => root.join("vendor"),
    }
}