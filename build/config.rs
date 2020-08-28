/**
Converts environment variables into Cargo cfgs that can then be used in code.
Crates can opt-in to a standard set of cross-cutting configurations by using [`Cfgs`].
*/
pub mod config {
    use std::{collections::HashSet, env};

    #[derive(Debug)]
    pub struct Cfgs {
        enabled: HashSet<String>,
    }

    pub struct Cfg(&'static str);

    impl Cfgs {
        /**
        Perform a checked build.

        These builds do extra checking and are suitable for testing and fuzzing.
        */
        pub const SQUIRRELJSON_CHECKED: Cfg = Cfg("checked");

        /**
        Perform a publish build.

        These builds have extra checks at build-time to ensure they're suitable for release.
        */
        pub const SQUIRRELJSON_PUBLISHED: Cfg = Cfg("published");

        /**
        Create a build configuration and read the default variables.
        */
        pub fn new() -> Self {
            let mut enabled = HashSet::new();

            let cfgs = &[Self::SQUIRRELJSON_CHECKED, Self::SQUIRRELJSON_PUBLISHED];

            cfg_from_env_value("wasm", "TARGET", "wasm32-unknown-unknown", &mut enabled);
            cfg_from_env_value("release", "PROFILE", "release", &mut enabled);
            cfg_from_env_value("debug", "PROFILE", "debug", &mut enabled);

            if unstable() {
                enabled.insert("unstable".to_owned());
            }

            for cfg in cfgs {
                cfg_from_env_present(cfg.0, &mut enabled);
            }

            Cfgs { enabled }
        }

        pub fn is_wasm(&self) -> bool {
            self.enabled.contains("wasm")
        }

        pub fn is_debug(&self) -> bool {
            self.enabled.contains("debug")
        }

        pub fn is_release(&self) -> bool {
            self.enabled.contains("release")
        }

        pub fn is_checked(&self) -> bool {
            self.enabled.contains(Self::SQUIRRELJSON_CHECKED.0)
        }

        pub fn is_publish(&self) -> bool {
            self.enabled.contains(Self::SQUIRRELJSON_PUBLISHED.0)
        }

        pub fn is_unstable(&self) -> bool {
            self.enabled.contains("unstable")
        }

        pub fn is_enabled(&self, cfg: Cfg) -> bool {
            self.enabled.contains(cfg.0)
        }

        pub fn enable(&mut self, cfg: Cfg) {
            self.enabled.insert(cfg.0.into());
        }

        pub fn apply(self) {
            if self.is_publish() {
                assert!(
                    !self.is_checked(),
                    "a build may be either checked or published, but not both"
                );
                assert!(self.is_release(), "published builds must be optimized");
            }

            for cfg in &self.enabled {
                println!("cargo:rustc-cfg={}", cfg);
            }

            println!("cargo:rerun-if-changed=build.rs")
        }
    }

    fn cfg_from_env_present(cfg: impl AsRef<str>, enabled: &mut HashSet<String>) {
        let cfg = cfg.as_ref();

        let var = format!("SQUIRRELJSON_{}", cfg.to_uppercase());
        println!("cargo:rerun-if-env-changed={}", var);

        if let Ok(env_cfg) = env::var(var) {
            if env_cfg != "0" {
                enabled.insert(cfg.into());
            } else {
                enabled.remove(cfg);
            }
        }
    }

    fn cfg_from_env_value(
        cfg: impl AsRef<str>,
        key: impl AsRef<str>,
        value: impl AsRef<str>,
        enabled: &mut HashSet<String>,
    ) {
        println!("cargo:rerun-if-env-changed={}", key.as_ref());

        if let Ok(cargo_cfg) = env::var(key.as_ref()) {
            if cargo_cfg == value.as_ref() {
                enabled.insert(cfg.as_ref().into());
            }
        }
    }

    fn unstable() -> bool {
        version_check::is_feature_flaggable().unwrap_or(false)
    }
}
