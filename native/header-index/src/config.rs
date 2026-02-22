//! Platform configuration — hardcoded macOS paths (no TOML dependency).

use std::path::PathBuf;
use std::sync::OnceLock;

pub struct PlatformConfig {
    pub paths: PathsConfig,
    pub cache: CacheConfig,
    pub sdk: SdkConfig,
    pub sandbox: SandboxConfig,
    pub build: BuildConfig,
}

pub struct PathsConfig {
    pub public_frameworks: String,
    pub private_frameworks: String,
}

pub struct CacheConfig {
    pub kb_dir: String,
    pub shim_dir: String,
}

pub struct SdkConfig {
    pub command: Vec<String>,
    pub fallback: String,
}

pub struct SandboxConfig {
    pub enabled: bool,
    pub allowed_read: Vec<String>,
}

pub struct BuildConfig {
    pub version_command: Vec<String>,
}

pub fn platform() -> &'static PlatformConfig {
    static CONFIG: OnceLock<PlatformConfig> = OnceLock::new();
    CONFIG.get_or_init(|| PlatformConfig {
        paths: PathsConfig {
            public_frameworks: "/System/Library/Frameworks".into(),
            private_frameworks: "/System/Library/PrivateFrameworks".into(),
        },
        cache: CacheConfig {
            kb_dir: ".mcgyver/kb".into(),
            shim_dir: ".mcgyver/shims".into(),
        },
        sdk: SdkConfig {
            command: vec!["xcrun".into(), "--show-sdk-path".into()],
            fallback: "/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk".into(),
        },
        sandbox: SandboxConfig {
            enabled: true,
            allowed_read: vec![
                "/private/tmp".into(), "/tmp".into(), "/dev".into(),
                "/System".into(), "/usr".into(), "/Library".into(),
            ],
        },
        build: BuildConfig {
            version_command: vec!["sw_vers".into(), "-buildVersion".into()],
        },
    })
}

impl PlatformConfig {
    pub fn kb_cache_dir(&self) -> PathBuf {
        dirs_or_home().join(&self.cache.kb_dir)
    }

    pub fn shim_output_dir(&self) -> PathBuf {
        dirs_or_home().join(&self.cache.shim_dir)
    }

    pub fn framework_dirs(&self) -> Vec<&str> {
        vec![
            &self.paths.public_frameworks,
            &self.paths.private_frameworks,
        ]
    }

    pub fn umbrella_header(&self, framework: &str) -> String {
        format!(
            "{}/{}.framework/Headers/{}.h",
            self.paths.public_frameworks, framework, framework
        )
    }

    pub fn framework_binary(&self, framework: &str, private: bool) -> String {
        let base = if private {
            &self.paths.private_frameworks
        } else {
            &self.paths.public_frameworks
        };
        format!("{}/{}.framework/{}", base, framework, framework)
    }

    pub fn os_build_version(&self) -> String {
        if self.build.version_command.is_empty() {
            return String::new();
        }
        std::process::Command::new(&self.build.version_command[0])
            .args(&self.build.version_command[1..])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }

    pub fn sdk_path(&self) -> Option<String> {
        if self.sdk.command.is_empty() {
            return if self.sdk.fallback.is_empty() {
                None
            } else {
                Some(self.sdk.fallback.clone())
            };
        }
        std::process::Command::new(&self.sdk.command[0])
            .args(&self.sdk.command[1..])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    Some(self.sdk.fallback.clone())
                }
            })
    }
}

fn dirs_or_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}
