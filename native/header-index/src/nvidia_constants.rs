//! Hand-extracted NVIDIA driver constants.
//!
//! Sourced from open-gpu-kernel-modules headers via cuda_ioctl_sniffer
//! and siliconscribe/crates/protocol. When the `nvidia-bindings` feature
//! is enabled, these are replaced by bindgen output.

// =============================================================================
// RM Escape ioctl numbers (NV_ESC_*)
// From siliconscribe/crates/protocol/src/specs/nvidia/escapes.rs
// =============================================================================

/// IOCTL_BASE for NV kernel control node escapes.
pub const NV_IOCTL_BASE: u32 = 200;

pub const NV_ESC_CARD_INFO: u32 = NV_IOCTL_BASE; // 200
pub const NV_ESC_REGISTER_FD: u32 = NV_IOCTL_BASE + 1; // 201
pub const NV_ESC_ALLOC_OS_EVENT: u32 = NV_IOCTL_BASE + 6; // 206
pub const NV_ESC_FREE_OS_EVENT: u32 = NV_IOCTL_BASE + 7; // 207
pub const NV_ESC_STATUS_CODE: u32 = NV_IOCTL_BASE + 9; // 209
pub const NV_ESC_CHECK_VERSION_STR: u32 = NV_IOCTL_BASE + 10; // 210
pub const NV_ESC_IOCTL_XFER_CMD: u32 = NV_IOCTL_BASE + 11; // 211
pub const NV_ESC_ATTACH_GPUS_TO_FD: u32 = NV_IOCTL_BASE + 12; // 212
pub const NV_ESC_QUERY_DEVICE_INTR: u32 = NV_IOCTL_BASE + 13; // 213
pub const NV_ESC_SYS_PARAMS: u32 = NV_IOCTL_BASE + 14; // 214
pub const NV_ESC_NUMA_INFO: u32 = NV_IOCTL_BASE + 15; // 215
pub const NV_ESC_SET_NUMA_STATUS: u32 = NV_IOCTL_BASE + 16; // 216
pub const NV_ESC_EXPORT_TO_DMABUF_FD: u32 = NV_IOCTL_BASE + 17; // 217

// Legacy RM escape selectors (from nv_escape.h)
pub const NV_ESC_RM_ALLOC_MEMORY: u32 = 0x27;
pub const NV_ESC_RM_ALLOC_OBJECT: u32 = 0x28;
pub const NV_ESC_RM_FREE: u32 = 0x29;
pub const NV_ESC_RM_CONTROL: u32 = 0x2A;
pub const NV_ESC_RM_ALLOC: u32 = 0x2B;
pub const NV_ESC_RM_CONFIG_GET: u32 = 0x32;
pub const NV_ESC_RM_CONFIG_SET: u32 = 0x33;
pub const NV_ESC_RM_DUP_OBJECT: u32 = 0x34;
pub const NV_ESC_RM_SHARE: u32 = 0x35;
pub const NV_ESC_RM_VID_HEAP_CONTROL: u32 = 0x4A;
pub const NV_ESC_RM_MAP_MEMORY: u32 = 0x4E;
pub const NV_ESC_RM_UNMAP_MEMORY: u32 = 0x4F;
pub const NV_ESC_RM_MAP_MEMORY_DMA: u32 = 0x57;
pub const NV_ESC_RM_UNMAP_MEMORY_DMA: u32 = 0x58;
pub const NV_ESC_RM_UPDATE_DEVICE_MAPPING_INFO: u32 = 0x5E;

// =============================================================================
// RM Object class IDs
// From cuda_ioctl_sniffer/src/sniff/platform.rs handle_nv_rm_alloc()
// =============================================================================

pub const NV01_ROOT_CLIENT: u32 = 0x0041;
pub const NV01_DEVICE_0: u32 = 0x0080;
pub const NV01_EVENT_OS_EVENT: u32 = 0x0079;
pub const NV20_SUBDEVICE_0: u32 = 0x2080;
pub const FERMI_VASPACE_A: u32 = 0x90F1;
pub const KEPLER_CHANNEL_GROUP_A: u32 = 0xA06C;
pub const FERMI_CONTEXT_SHARE_A: u32 = 0x9067;
pub const AMPERE_CHANNEL_GPFIFO_A: u32 = 0xC46F;
pub const AMPERE_COMPUTE_B: u32 = 0xC7C0;
pub const TURING_USERMODE_A: u32 = 0xC461;
pub const AMPERE_DMA_COPY_B: u32 = 0xC7B5;
pub const GT200_DEBUGGER: u32 = 0x83DE;
pub const NV50_P2P: u32 = 0x503B;

// =============================================================================
// RM Control commands (NV_ESC_RM_CONTROL sub-commands)
// From cuda_ioctl_sniffer/src/sniff/platform.rs describe_rm_control_command()
// and open-gpu-kernel-modules headers
// =============================================================================

// NV0000 root-client control commands
pub const NV0000_CTRL_CMD_SYSTEM_GET_BUILD_VERSION: u32 = 0x00000101;
pub const NV0000_CTRL_CMD_SYSTEM_GET_FABRIC_STATUS: u32 = 0x00000136;
pub const NV0000_CTRL_CMD_GPU_GET_ATTACHED_IDS: u32 = 0x00000201;
pub const NV0000_CTRL_CMD_GPU_GET_ID_INFO: u32 = 0x00000202;
pub const NV0000_CTRL_CMD_GPU_GET_DEVICE_IDS: u32 = 0x00000204;
pub const NV0000_CTRL_CMD_GPU_GET_PROBED_IDS: u32 = 0x00000203;
pub const NV0000_CTRL_CMD_GPU_ATTACH_IDS: u32 = 0x00000280;
pub const NV0000_CTRL_CMD_GPU_DETACH_IDS: u32 = 0x00000281;
pub const NV0000_CTRL_CMD_GPU_GET_MEMOP_ENABLE: u32 = 0x00000286;
pub const NV0000_CTRL_CMD_SYNC_GPU_BOOST_GROUP_INFO: u32 = 0x00000A04;
pub const NV0000_CTRL_CMD_CLIENT_GET_ADDR_SPACE_TYPE: u32 = 0x00000D01;
pub const NV0000_CTRL_CMD_CLIENT_SET_INHERITED_SHARE_POLICY: u32 = 0x00000D03;
pub const NV0000_CTRL_CMD_SYSTEM_GET_P2P_CAPS_MATRIX: u32 = 0x00000139;

// NV0080 device control commands
pub const NV0080_CTRL_CMD_GPU_GET_CLASSLIST: u32 = 0x00800201;

// NV2080 subdevice control commands
pub const NV2080_CTRL_CMD_GPU_GET_GID_INFO: u32 = 0x2080014A;
pub const NV2080_CTRL_CMD_FB_GET_INFO: u32 = 0x20801301;
pub const NV2080_CTRL_CMD_GR_GET_INFO: u32 = 0x20801201;
pub const NV2080_CTRL_CMD_GR_GET_GPC_MASK: u32 = 0x20801202;
pub const NV2080_CTRL_CMD_GR_GET_CTX_BUFFER_SIZE: u32 = 0x20801218;
pub const NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE: u32 = 0x20801210;
pub const NV2080_CTRL_CMD_GR_GET_TPC_MASK: u32 = 0x20801203;
pub const NV2080_CTRL_CMD_GR_GET_CAPS_V2: u32 = 0x20801227;
pub const NV2080_CTRL_CMD_GR_GET_GLOBAL_SM_ORDER: u32 = 0x2080120B;
pub const NV2080_CTRL_CMD_MC_GET_ARCH_INFO: u32 = 0x20801701;
pub const NV2080_CTRL_CMD_BUS_GET_PCI_INFO: u32 = 0x20801801;
pub const NV2080_CTRL_CMD_BUS_GET_INFO: u32 = 0x20801802;
pub const NV2080_CTRL_CMD_BUS_GET_PCI_BAR_INFO: u32 = 0x20801803;
pub const NV2080_CTRL_CMD_NVLINK_GET_NVLINK_STATUS: u32 = 0x20803002;
pub const NV2080_CTRL_CMD_GSP_GET_FEATURES: u32 = 0x20803601;
pub const NV2080_CTRL_CMD_PERF_BOOST: u32 = 0x20802009;
pub const NV2080_CTRL_CMD_CE_GET_CAPS: u32 = 0x20802A01;
pub const NV2080_CTRL_CMD_GR_GET_SM_ISSUE_RATE_MODIFIER: u32 = 0x20801230;

// Channel/GPFIFO control commands
pub const NVC36F_CTRL_GET_CLASS_ENGINEID: u32 = 0xC36F0101;
pub const NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN: u32 = 0xC36F0108;
pub const NV906F_CTRL_GET_CLASS_ENGINEID: u32 = 0x906F0101;
pub const NVA06C_CTRL_CMD_GPFIFO_SCHEDULE: u32 = 0xA06C0101;
pub const NVA06C_CTRL_CMD_SET_TIMESLICE: u32 = 0xA06C0103;
pub const NV83DE_CTRL_CMD_DEBUG_SET_EXCEPTION_MASK: u32 = 0x83DE0309;

// =============================================================================
// UVM commands
// From siliconscribe/crates/protocol/src/specs/nvidia/escapes.rs uvm module
// =============================================================================

pub const UVM_RESERVE_VA: u32 = 1;
pub const UVM_RELEASE_VA: u32 = 2;
pub const UVM_REGION_COMMIT: u32 = 3;
pub const UVM_REGION_DECOMMIT: u32 = 4;
pub const UVM_RUN_TEST: u32 = 9;
pub const UVM_ADD_SESSION: u32 = 10;
pub const UVM_REMOVE_SESSION: u32 = 11;
pub const UVM_ENABLE_COUNTERS: u32 = 12;
pub const UVM_CREATE_RANGE_GROUP: u32 = 23;
pub const UVM_DESTROY_RANGE_GROUP: u32 = 24;
pub const UVM_REGISTER_GPU_VASPACE: u32 = 25;
pub const UVM_UNREGISTER_GPU_VASPACE: u32 = 26;
pub const UVM_REGISTER_CHANNEL: u32 = 27;
pub const UVM_UNREGISTER_CHANNEL: u32 = 28;
pub const UVM_ENABLE_PEER_ACCESS: u32 = 29;
pub const UVM_DISABLE_PEER_ACCESS: u32 = 30;
pub const UVM_MAP_EXTERNAL_ALLOCATION: u32 = 33;
pub const UVM_FREE: u32 = 34;
pub const UVM_MEM_MAP: u32 = 35;
pub const UVM_REGISTER_GPU: u32 = 37;
pub const UVM_UNREGISTER_GPU: u32 = 38;
pub const UVM_PAGEABLE_MEM_ACCESS: u32 = 39;
pub const UVM_SET_PREFERRED_LOCATION: u32 = 42;
pub const UVM_CREATE_EXTERNAL_RANGE: u32 = 73;
pub const UVM_INITIALIZE: u32 = 0x30000001;

// =============================================================================
// Hailo constants
// From siliconscribe/crates/protocol/src/specs/hailo.rs
// =============================================================================

pub const HAILO_MAX_VDMA_ENGINES: u32 = 3;
pub const HAILO_MAX_CHANNELS_PER_ENGINE: u32 = 32;
pub const HAILO_FW_CONTROL_BUFFER_LEN: u32 = 1500;
/// IRQ data length: MAX_CHANNELS_PER_ENGINE * MAX_VDMA_ENGINES * 3
pub const HAILO_IRQ_DATA_LEN: u32 = 288;

// Hailo VDMA IOCTL opcodes
pub const HAILO_VDMA_ENABLE_CHANNELS: u64 = 0x800D_7600;
pub const HAILO_VDMA_DISABLE_CHANNELS: u64 = 0x800C_7601;
pub const HAILO_VDMA_BUFFER_MAP: u64 = 0xC028_7604;
pub const HAILO_VDMA_BUFFER_UNMAP: u64 = 0x8008_7605;
pub const HAILO_VDMA_DESC_LIST_CREATE: u64 = 0xC020_7607;
pub const HAILO_VDMA_DESC_LIST_RELEASE: u64 = 0x8008_7608;
pub const HAILO_VDMA_DESC_LIST_PROGRAM: u64 = 0x8040_7609;
pub const HAILO_VDMA_LAUNCH_TRANSFER: u64 = 0x80E8_760D;
pub const HAILO_VDMA_INTERRUPTS_WAIT: u64 = 0xC130_7602;
pub const HAILO_NNC_FW_CONTROL: u64 = 0xC5F8_6E00;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nv_esc_base_constants() {
        assert_eq!(NV_ESC_CARD_INFO, 200);
        assert_eq!(NV_ESC_REGISTER_FD, 201);
        assert_eq!(NV_ESC_ALLOC_OS_EVENT, 206);
        assert_eq!(NV_ESC_SYS_PARAMS, 214);
        assert_eq!(NV_ESC_NUMA_INFO, 215);
    }

    #[test]
    fn test_nv_esc_rm_escapes() {
        assert_eq!(NV_ESC_RM_ALLOC_MEMORY, 0x27);
        assert_eq!(NV_ESC_RM_FREE, 0x29);
        assert_eq!(NV_ESC_RM_CONTROL, 0x2A);
        assert_eq!(NV_ESC_RM_ALLOC, 0x2B);
        assert_eq!(NV_ESC_RM_MAP_MEMORY, 0x4E);
        assert_eq!(NV_ESC_RM_UNMAP_MEMORY, 0x4F);
        assert_eq!(NV_ESC_RM_MAP_MEMORY_DMA, 0x57);
        assert_eq!(NV_ESC_RM_UNMAP_MEMORY_DMA, 0x58);
        assert_eq!(NV_ESC_RM_VID_HEAP_CONTROL, 0x4A);
    }

    #[test]
    fn test_rm_object_classes() {
        assert_eq!(NV01_ROOT_CLIENT, 0x0041);
        assert_eq!(NV01_DEVICE_0, 0x0080);
        assert_eq!(NV20_SUBDEVICE_0, 0x2080);
        assert_eq!(FERMI_VASPACE_A, 0x90F1);
        assert_eq!(KEPLER_CHANNEL_GROUP_A, 0xA06C);
        assert_eq!(AMPERE_CHANNEL_GPFIFO_A, 0xC46F);
        assert_eq!(AMPERE_COMPUTE_B, 0xC7C0);
        assert_eq!(TURING_USERMODE_A, 0xC461);
    }

    #[test]
    fn test_rm_control_commands() {
        assert_eq!(NV0000_CTRL_CMD_GPU_GET_ATTACHED_IDS, 0x00000201);
        assert_eq!(NV0000_CTRL_CMD_GPU_GET_DEVICE_IDS, 0x00000204);
        assert_eq!(NV2080_CTRL_CMD_GPU_GET_GID_INFO, 0x2080014A);
        assert_eq!(NV2080_CTRL_CMD_MC_GET_ARCH_INFO, 0x20801701);
        assert_eq!(NV2080_CTRL_CMD_GR_GET_INFO, 0x20801201);
        assert_eq!(NV2080_CTRL_CMD_GR_GET_CAPS_V2, 0x20801227);
        assert_eq!(NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN, 0xC36F0108);
        assert_eq!(NVA06C_CTRL_CMD_GPFIFO_SCHEDULE, 0xA06C0101);
    }

    #[test]
    fn test_uvm_commands() {
        assert_eq!(UVM_REGISTER_GPU, 37);
        assert_eq!(UVM_REGISTER_GPU_VASPACE, 25);
        assert_eq!(UVM_REGISTER_CHANNEL, 27);
        assert_eq!(UVM_CREATE_EXTERNAL_RANGE, 73);
        assert_eq!(UVM_MAP_EXTERNAL_ALLOCATION, 33);
        assert_eq!(UVM_FREE, 34);
        assert_eq!(UVM_INITIALIZE, 0x30000001);
    }

    #[test]
    fn test_hailo_constants() {
        assert_eq!(HAILO_MAX_VDMA_ENGINES, 3);
        assert_eq!(HAILO_MAX_CHANNELS_PER_ENGINE, 32);
        assert_eq!(HAILO_FW_CONTROL_BUFFER_LEN, 1500);
        assert_eq!(HAILO_IRQ_DATA_LEN, 288);
    }

    #[test]
    fn test_hailo_opcodes() {
        assert_eq!(HAILO_VDMA_ENABLE_CHANNELS, 0x800D_7600);
        assert_eq!(HAILO_VDMA_BUFFER_MAP, 0xC028_7604);
        assert_eq!(HAILO_VDMA_DESC_LIST_CREATE, 0xC020_7607);
        assert_eq!(HAILO_VDMA_LAUNCH_TRANSFER, 0x80E8_760D);
        assert_eq!(HAILO_VDMA_INTERRUPTS_WAIT, 0xC130_7602);
        assert_eq!(HAILO_NNC_FW_CONTROL, 0xC5F8_6E00);
    }
}
