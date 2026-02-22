//! Minimal device profile — just enough for seed loading.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: String,
    pub vram_bytes: u64,
    pub compute_units: u32,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceleratorInfo {
    pub name: String,
    pub kind: String,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameworkInfo {
    pub name: String,
    pub path: String,
    pub is_private: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThermalInfo {
    pub thermal_pressure: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub name: String,
    pub cores_physical: u32,
    pub cores_logical: u32,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceProfile {
    pub platform: String,
    pub cpu: CpuInfo,
    pub gpus: Vec<GpuInfo>,
    pub accelerators: Vec<AcceleratorInfo>,
    pub frameworks: Vec<FrameworkInfo>,
    pub memory: memory::DeviceMemoryModel,
    pub thermal: ThermalInfo,
}

pub mod memory {
    use serde::{Deserialize, Serialize};
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct DeviceMemoryModel {
        pub total_bytes: u64,
        pub unified: bool,
    }
    impl DeviceMemoryModel {
        pub fn discover() -> Self {
            #[cfg(target_os = "macos")]
            {
                Self {
                    total_bytes: sysctl_u64("hw.memsize").unwrap_or(0),
                    unified: true,
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                Self::default()
            }
        }
    }
    #[cfg(target_os = "macos")]
    fn sysctl_u64(name: &str) -> Option<u64> {
        let c_name = std::ffi::CString::new(name).ok()?;
        let mut val: u64 = 0;
        let mut sz = std::mem::size_of::<u64>() as libc::size_t;
        unsafe {
            if libc::sysctlbyname(
                c_name.as_ptr(),
                &mut val as *mut u64 as _,
                &mut sz,
                std::ptr::null_mut(),
                0,
            ) == 0
            {
                Some(val)
            } else {
                None
            }
        }
    }
}

pub fn discover() -> DeviceProfile {
    let logical = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1);
    let mut frameworks = Vec::new();

    let public = [
        "Metal",
        "Accelerate",
        "CoreML",
        "VideoToolbox",
        "CoreVideo",
        "CoreMedia",
        "IOKit",
        "IOSurface",
        "CoreFoundation",
        "CoreGraphics",
    ];
    for name in &public {
        let dir = format!("/System/Library/Frameworks/{name}.framework");
        if Path::new(&dir).is_dir() {
            frameworks.push(FrameworkInfo {
                name: name.to_string(),
                path: format!("{dir}/{name}"),
                is_private: false,
            });
        }
    }
    let private = [
        "kperf",
        "MTLCompiler",
        "SkyLight",
        "AppleNeuralEngine",
        "IOGPU",
        "IOAccelerator",
        "AGXCompilerCore",
    ];
    for name in &private {
        let dir = format!("/System/Library/PrivateFrameworks/{name}.framework");
        if Path::new(&dir).is_dir() {
            frameworks.push(FrameworkInfo {
                name: name.to_string(),
                path: format!("{dir}/{name}"),
                is_private: true,
            });
        }
    }

    DeviceProfile {
        platform: "macos".into(),
        cpu: CpuInfo {
            name: "Apple Silicon".into(),
            cores_physical: logical,
            cores_logical: logical,
            features: vec!["AMX".into(), "NEON".into()],
        },
        gpus: vec![GpuInfo {
            name: "Apple GPU".into(),
            vendor: "Apple".into(),
            vram_bytes: 0,
            compute_units: 0,
            features: vec!["metal3".into()],
        }],
        accelerators: vec![],
        frameworks,
        memory: memory::DeviceMemoryModel::discover(),
        thermal: ThermalInfo::default(),
    }
}
