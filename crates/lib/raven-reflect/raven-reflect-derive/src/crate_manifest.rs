extern crate proc_macro;

use std::path::PathBuf;

use proc_macro::TokenStream;
use toml::{map::Map, Value};

/// Container for crate manifest fetched from external toml crate. 
pub struct CrateManifest {
    manifest: Map<String, Value>,
}

/// Get the binary crate manifest by default.
impl Default for CrateManifest {
    fn default() -> Self {
        Self {
            manifest: std::env::var("CARGO_MANIFEST_DIR")
                .map(PathBuf::from)
                .map(|mut path| {
                    path.push("Cargo.toml");
                    let manifest = std::fs::read_to_string(path).unwrap();
                    toml::from_str(&manifest).unwrap()
                })
                .unwrap()
        }
    }
}

impl CrateManifest {
    pub fn new(cargo_manifest_path: PathBuf) -> Self {
        Self {
            manifest: {
                let manifest = std::fs::read_to_string(cargo_manifest_path).unwrap();
                toml::from_str(&manifest).unwrap()
            }
        }
    }

    /// Try to get a deps path within a given crate.
    pub fn try_get_path(&self, name: &str) -> Option<syn::Path> {
        fn get_deps(dep: &Value) -> Option<&str> {
            if dep.as_str().is_some() {
                None
            } else {
                dep.as_table()
                    .unwrap()
                    .get("package")
                    .map(|name| name.as_str().unwrap())
            }
        }

        let find_in_deps_func = |deps: &Map<String, Value>| -> Option<syn::Path> {
            let package = if let Some(dep) = deps.get(name) {
                // directly depends on crate
                return Some(Self::parse_str(get_deps(dep).unwrap_or(name)));
            } else if let Some(dep) = deps.get("raven-engine") {
                // access by raven-engine crate
                get_deps(dep).unwrap_or("raven_engine")
            } else if let Some(dep) = deps.get("raven-facade") {
                // access by raven-facade crate
                get_deps(dep).unwrap_or("raven_facade")
            } else {
                // can't access raven-reflect crate in this situation
                return None;
            };

            let mut path = Self::parse_str::<syn::Path>(package);
            // strip prefix to access inner crate
            if let Some(module) = name.strip_prefix("raven_") {
                // add mod name to path
                // e.g. raven_facade::reflect
                path.segments.push(Self::parse_str(module));
            }
            Some(path)
        };

        let deps = self.manifest
            .get("dependencies")
            .map(|deps| deps.as_table().unwrap());
        let dev_deps = self.manifest
            .get("dev-dependencies")
            .map(|deps| deps.as_table().unwrap());
        
        deps.and_then(find_in_deps_func)
            .or_else(|| dev_deps.and_then(find_in_deps_func))
    }

    pub fn get_path_default(name: &str) -> syn::Path {
        Self::default().get_path(name)
    }

    pub fn get_path(&self, name: &str) -> syn::Path {
        self.try_get_path(name)
            // cannot find path in dependencies, assume crate is inside current crate
            .unwrap_or_else(|| Self::parse_str(name))
    }
 
    /// Parse the path as TokenStream and return a &str represent this TokenStream.
    pub fn parse_str<T: syn::parse::Parse>(path: &str) -> T {
        syn::parse(path.parse::<TokenStream>().unwrap()).unwrap()
    }
}