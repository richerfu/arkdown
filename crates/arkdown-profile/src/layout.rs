use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};

pub(crate) struct ProfileProjectLayout<'a> {
    project_root: &'a Path,
}

impl<'a> ProfileProjectLayout<'a> {
    pub(crate) fn new(project_root: &'a Path) -> Self {
        Self { project_root }
    }

    pub(crate) fn app_scope_app_json5(&self) -> PathBuf {
        self.project_root.join("AppScope").join("app.json5")
    }

    pub(crate) fn app_scope_pac_json5(&self) -> PathBuf {
        self.project_root.join("AppScope").join("pac.json5")
    }

    pub(crate) fn build_output_pac_json(&self, product_name: &str) -> PathBuf {
        self.project_root
            .join("build")
            .join("outputs")
            .join(product_name)
            .join("pac.json")
    }
}

pub(crate) struct ProfileModuleLayout<'a> {
    module_root: &'a Path,
}

impl<'a> ProfileModuleLayout<'a> {
    pub(crate) fn new(module_root: &'a Path) -> Self {
        Self { module_root }
    }

    pub(crate) fn src_main(&self) -> PathBuf {
        self.module_root.join("src").join("main")
    }

    pub(crate) fn module_json5(&self) -> PathBuf {
        self.src_main().join("module.json5")
    }

    pub(crate) fn module_json(&self) -> PathBuf {
        self.src_main().join("module.json")
    }

    pub(crate) fn package_module_json5(&self) -> PathBuf {
        self.module_root.join("module.json5")
    }

    pub(crate) fn package_module_json(&self) -> PathBuf {
        self.module_root.join("module.json")
    }

    pub(crate) fn oh_package_json5(&self) -> PathBuf {
        self.module_root.join("oh-package.json5")
    }

    pub(crate) fn pac_json5(&self) -> PathBuf {
        self.module_root.join("pac.json5")
    }

    pub(crate) fn modules_ap(&self) -> PathBuf {
        self.module_root.join("modules.ap")
    }

    pub(crate) fn index_abc(&self) -> PathBuf {
        self.module_root.join("index.abc")
    }

    pub(crate) fn ets_modules_abc(&self) -> PathBuf {
        self.module_root.join("ets").join("modules.abc")
    }

    pub(crate) fn meta_inf_preload_json(&self) -> PathBuf {
        self.module_root.join("META-INF").join("preload.json")
    }

    pub(crate) fn ets_source_root(&self) -> PathBuf {
        self.src_main().join("ets")
    }

    pub(crate) fn workers_source_root(&self) -> PathBuf {
        self.ets_source_root().join("workers")
    }

    pub(crate) fn mock_config_json5(&self) -> PathBuf {
        self.module_root
            .join("src")
            .join("mock")
            .join("mock-config.json5")
    }

    pub(crate) fn ohos_test_ets_root(&self) -> PathBuf {
        self.module_root.join("src").join("ohosTest").join("ets")
    }

    pub(crate) fn profile_dir(&self) -> PathBuf {
        self.src_main()
            .join("resources")
            .join("base")
            .join("profile")
    }

    pub(crate) fn profile_json(&self, profile_name: &str) -> PathBuf {
        self.profile_dir()
            .join(Self::file_name_with_suffix(profile_name, ".json"))
    }

    pub(crate) fn prebuilt_profile_dir(&self) -> PathBuf {
        self.module_root
            .join("resources")
            .join("base")
            .join("profile")
    }

    pub(crate) fn prebuilt_profile_json(&self, profile_name: &str) -> PathBuf {
        self.prebuilt_profile_dir()
            .join(Self::file_name_with_suffix(profile_name, ".json"))
    }

    pub(crate) fn insight_intent_profile_json(&self) -> PathBuf {
        self.profile_json("insight_intent")
    }

    pub(crate) fn resources_root(&self) -> PathBuf {
        self.src_main().join("resources")
    }

    pub(crate) fn source_from_module_src(&self, src_path: &Path) -> PathBuf {
        self.src_main().join(src_path)
    }

    pub(crate) fn source_from_src_main_entry(&self, entry: &Path) -> PathBuf {
        let src_main_prefix = Path::new("src").join("main");
        let src_main_relative = entry.strip_prefix(&src_main_prefix).unwrap_or(entry);
        self.source_from_module_src(src_main_relative)
    }

    pub(crate) fn source_entry_relative_to_ets(&self, entry: &Path) -> Option<PathBuf> {
        let src_main_ets_prefix = Path::new("src").join("main").join("ets");
        entry
            .strip_prefix(src_main_ets_prefix)
            .ok()
            .map(Path::to_path_buf)
    }

    pub(crate) fn build_default(&self) -> PathBuf {
        self.module_root.join("build").join("default")
    }

    pub(crate) fn default_intermediates(&self) -> PathBuf {
        self.build_default().join("intermediates")
    }

    pub(crate) fn default_intermediate_dir(&self, stage: &str) -> PathBuf {
        self.default_intermediates().join(stage).join("default")
    }

    pub(crate) fn default_intermediate_file(&self, stage: &str, file_name: &str) -> PathBuf {
        self.default_intermediate_dir(stage).join(file_name)
    }

    pub(crate) fn privacy_profile_pac_json5(&self, target_name: &str) -> PathBuf {
        self.default_intermediates()
            .join("privacy_profile")
            .join(target_name)
            .join("pac.json5")
    }

    pub(crate) fn loader_out_default_dir(&self) -> PathBuf {
        self.default_intermediate_dir("loader_out")
    }

    pub(crate) fn aot_loader_output_dir(&self) -> PathBuf {
        self.loader_out_default_dir().join("an").join("arm64-v8a")
    }

    pub(crate) fn ap_intermediate_dir(&self) -> PathBuf {
        self.default_intermediates().join("ap")
    }

    pub(crate) fn startup_config_temp_json(&self) -> PathBuf {
        self.default_intermediate_file("startup_config", "temp-startup-config.json")
    }

    pub(crate) fn resource_table_txt(&self) -> PathBuf {
        self.default_intermediate_file("res", "ResourceTable.txt")
    }

    pub(crate) fn merge_profile_module_json(&self) -> PathBuf {
        self.default_intermediate_file("merge_profile", "module.json")
    }

    pub(crate) fn res_module_json(&self) -> PathBuf {
        self.default_intermediate_file("res", "module.json")
    }

    pub(crate) fn compiled_res_profile_dir(&self) -> PathBuf {
        self.default_intermediate_dir("res")
            .join("resources")
            .join("base")
            .join("profile")
    }

    pub(crate) fn compiled_res_profile_json(&self, profile_name: &str) -> PathBuf {
        self.compiled_res_profile_dir()
            .join(Self::file_name_with_suffix(profile_name, ".json"))
    }

    pub(crate) fn build_config_candidates(&self) -> [PathBuf; 4] {
        [
            self.build_default().join("config").join("buildConfig.json"),
            self.default_intermediate_file("build_config", "buildConfig.json"),
            self.default_intermediate_file("build_config", "build-config.json"),
            self.default_intermediate_file("build_config", "build_config.json"),
        ]
    }

    fn file_name_with_suffix(name: &str, suffix: &str) -> OsString {
        let mut file_name = OsString::from(name);
        file_name.push(suffix);
        file_name
    }
}

pub(crate) struct LegacyFormSourceLayout<'a> {
    source_path: &'a Path,
}

impl<'a> LegacyFormSourceLayout<'a> {
    pub(crate) fn new(source_path: &'a Path) -> Self {
        Self { source_path }
    }

    pub(crate) fn hml_candidate(&self) -> PathBuf {
        Self::append_file_suffix(self.source_path, ".hml")
    }

    pub(crate) fn supervisual_candidate(&self) -> PathBuf {
        let mut candidate = PathBuf::new();
        for component in self.source_path.components() {
            match component {
                Component::Normal(part) if part == OsStr::new("js") => {
                    candidate.push("supervisual");
                }
                Component::Normal(part) => candidate.push(part),
                Component::CurDir => candidate.push("."),
                Component::ParentDir => candidate.push(".."),
                _ => candidate.push(component.as_os_str()),
            }
        }
        Self::append_file_suffix(&candidate, ".visual")
    }

    fn append_file_suffix(path: &Path, suffix: &str) -> PathBuf {
        let Some(file_name) = path.file_name() else {
            let file_name = suffix.strip_prefix('.').unwrap_or(suffix);
            return path.join(file_name);
        };
        let mut candidate = path.to_path_buf();
        let mut suffixed_name = OsString::from(file_name);
        suffixed_name.push(suffix);
        candidate.set_file_name(suffixed_name);
        candidate
    }
}

pub(crate) struct ProfileOutputLayout<'a> {
    output_profile_dir: &'a Path,
}

impl<'a> ProfileOutputLayout<'a> {
    pub(crate) fn new(output_profile_dir: &'a Path) -> Self {
        Self { output_profile_dir }
    }

    pub(crate) fn profile_json(&self, profile_name: &str) -> PathBuf {
        self.output_profile_dir
            .join(ProfileModuleLayout::file_name_with_suffix(
                profile_name,
                ".json",
            ))
    }
}

pub(crate) struct ProfileIntermediatesLayout<'a> {
    intermediates_dir: &'a Path,
}

impl<'a> ProfileIntermediatesLayout<'a> {
    pub(crate) fn new(intermediates_dir: &'a Path) -> Self {
        Self { intermediates_dir }
    }

    pub(crate) fn loader_json(&self) -> PathBuf {
        self.intermediates_dir.join("loader.json")
    }

    pub(crate) fn pkg_context_info_json(&self) -> PathBuf {
        self.intermediates_dir.join("pkgContextInfo.json")
    }

    pub(crate) fn module_json(&self) -> PathBuf {
        self.intermediates_dir.join("module.json")
    }
}
