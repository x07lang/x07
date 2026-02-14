use crate::VmBackend;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmCaps {
    pub supports_bind_mount_ro: bool,
    pub supports_network_none: bool,
    pub supports_vm_sizing: bool,
    pub supports_readonly_rootfs: bool,
    pub supports_kill_by_id: bool,
}

impl VmCaps {
    pub fn for_backend(backend: VmBackend) -> Self {
        match backend {
            VmBackend::Vz => VmCaps {
                supports_bind_mount_ro: true,
                supports_network_none: true,
                supports_vm_sizing: true,
                supports_readonly_rootfs: false,
                supports_kill_by_id: true,
            },
            VmBackend::AppleContainer => VmCaps {
                supports_bind_mount_ro: true,
                supports_network_none: true,
                supports_vm_sizing: true,
                supports_readonly_rootfs: false,
                supports_kill_by_id: true,
            },
            VmBackend::Docker | VmBackend::Podman => VmCaps {
                supports_bind_mount_ro: true,
                supports_network_none: true,
                supports_vm_sizing: false,
                supports_readonly_rootfs: false,
                supports_kill_by_id: true,
            },
            VmBackend::FirecrackerCtr => VmCaps {
                supports_bind_mount_ro: true,
                supports_network_none: true,
                supports_vm_sizing: false,
                supports_readonly_rootfs: false,
                supports_kill_by_id: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_for_backend_is_stable() {
        assert!(VmCaps::for_backend(VmBackend::Vz).supports_vm_sizing);
        assert!(!VmCaps::for_backend(VmBackend::FirecrackerCtr).supports_vm_sizing);
        assert!(VmCaps::for_backend(VmBackend::Vz).supports_network_none);
        assert!(VmCaps::for_backend(VmBackend::Vz).supports_bind_mount_ro);
    }
}
