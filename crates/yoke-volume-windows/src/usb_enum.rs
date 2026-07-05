// Consumed by provider.rs in a later task; not wired up yet.
#![allow(dead_code)]

use crate::ids::{split_multi_sz, to_wide, utf16_to_string, vid_pid_from_pnp_id};
use std::path::PathBuf;
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CM_Get_DevNode_PropertyW, CM_Get_Device_ID_Size,
    CM_Get_Device_IDW, CM_Get_Device_Interface_List_SizeW, CM_Get_Device_Interface_ListW,
    CM_Get_Device_Interface_PropertyW, CM_Get_Parent, CM_LOCATE_DEVNODE_NORMAL, CM_Locate_DevNodeW,
    CR_BUFFER_SMALL, CR_SUCCESS,
};
use windows::Win32::Devices::Properties::{
    DEVPKEY_Device_InstanceId, DEVPKEY_Device_LocationPaths, DEVPROPTYPE,
};
use windows::Win32::Devices::Usb::GUID_DEVINTERFACE_USB_DEVICE;
use windows::Win32::Storage::FileSystem::{
    GetVolumeInformationW, GetVolumeNameForVolumeMountPointW, GetVolumePathNamesForVolumeNameW,
};
use windows::Win32::System::Ioctl::GUID_DEVINTERFACE_VOLUME;
use windows::core::{GUID, PCWSTR};
use yoke_volume::state::VidPid;

pub struct UsbDevice {
    pub vid_pid: VidPid,
    pub location: Option<String>,
}

pub struct UsbVolume {
    pub vid_pid: VidPid,
    // Present only when a drive letter (or mount folder) is assigned.
    pub mount_point: Option<PathBuf>,
    pub label: Option<String>,
}

/// `None` = enumeration failed (transient `CfgMgr32` error); the caller must
/// keep its last good state. `Some(vec![])` = genuinely nothing present.
pub fn list_usb_devices() -> Option<Vec<UsbDevice>> {
    Some(
        interface_list(&GUID_DEVINTERFACE_USB_DEVICE)?
            .into_iter()
            .filter_map(|iface| {
                let instance = interface_instance_id(&iface)?;
                let vid_pid = vid_pid_from_pnp_id(&instance)?;
                let location = devnode_for_instance(&instance).and_then(location_paths);
                Some(UsbDevice { vid_pid, location })
            })
            .collect(),
    )
}

/// Same `None` semantics as `list_usb_devices`.
pub fn list_usb_volumes() -> Option<Vec<UsbVolume>> {
    Some(
        interface_list(&GUID_DEVINTERFACE_VOLUME)?
            .into_iter()
            .filter_map(|iface| {
                let instance = interface_instance_id(&iface)?;
                let devinst = devnode_for_instance(&instance)?;
                let vid_pid = usb_ancestor_vid_pid(devinst)?;
                let (mount_point, label) = volume_mount_and_label(&iface);
                Some(UsbVolume {
                    vid_pid,
                    mount_point,
                    label,
                })
            })
            .collect(),
    )
}

/// All present device-interface paths for `class`, via the documented
/// size-then-list loop (the set can grow between the two calls, hence the
/// retry on `CR_BUFFER_SMALL`). `None` on `CfgMgr32` failure — distinct from an
/// empty list, so a transient error is never read as "all devices gone".
fn interface_list(class: &GUID) -> Option<Vec<String>> {
    for _ in 0..4 {
        let mut len: u32 = 0;
        // SAFETY: out-pointer is a valid local; class GUID lives across the call.
        let cr = unsafe {
            CM_Get_Device_Interface_List_SizeW(
                &raw mut len,
                class,
                PCWSTR::null(),
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            )
        };
        if cr != CR_SUCCESS {
            return None;
        }
        if len < 2 {
            return Some(Vec::new());
        }
        let mut buf = vec![0u16; len as usize];
        // SAFETY: buffer sized by the size call just above.
        let cr = unsafe {
            CM_Get_Device_Interface_ListW(
                class,
                PCWSTR::null(),
                &mut buf,
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            )
        };
        if cr == CR_SUCCESS {
            return Some(split_multi_sz(&buf));
        }
        if cr != CR_BUFFER_SMALL {
            return None;
        }
    }
    None
}

/// `DEVPKEY_Device_InstanceId` for a device-interface path (size probe, then
/// fetch; the property arrives as UTF-16 bytes).
fn interface_instance_id(interface_path: &str) -> Option<String> {
    let wide = to_wide(interface_path);
    let mut prop_type = DEVPROPTYPE::default();
    let mut size: u32 = 0;
    // SAFETY: size probe with a null buffer is the documented calling pattern.
    let _ = unsafe {
        CM_Get_Device_Interface_PropertyW(
            PCWSTR(wide.as_ptr()),
            &DEVPKEY_Device_InstanceId,
            &raw mut prop_type,
            None,
            &raw mut size,
            0,
        )
    };
    if size == 0 {
        return None;
    }
    let mut buf = vec![0u8; size as usize];
    // SAFETY: buffer sized by the probe above.
    let cr = unsafe {
        CM_Get_Device_Interface_PropertyW(
            PCWSTR(wide.as_ptr()),
            &DEVPKEY_Device_InstanceId,
            &raw mut prop_type,
            Some(buf.as_mut_ptr()),
            &raw mut size,
            0,
        )
    };
    if cr != CR_SUCCESS {
        return None;
    }
    Some(utf16_to_string(&bytes_to_wide(&buf)))
}

fn devnode_for_instance(instance_id: &str) -> Option<u32> {
    let wide = to_wide(instance_id);
    let mut devinst: u32 = 0;
    // SAFETY: valid NUL-terminated instance ID and a local out-pointer.
    let cr = unsafe {
        CM_Locate_DevNodeW(
            &raw mut devinst,
            PCWSTR(wide.as_ptr()),
            CM_LOCATE_DEVNODE_NORMAL,
        )
    };
    (cr == CR_SUCCESS).then_some(devinst)
}

fn device_id(devinst: u32) -> Option<String> {
    let mut len: u32 = 0;
    // SAFETY: local out-pointer.
    if unsafe { CM_Get_Device_ID_Size(&raw mut len, devinst, 0) } != CR_SUCCESS {
        return None;
    }
    let mut buf = vec![0u16; len as usize + 1];
    // SAFETY: buffer sized by the size call (+1 for the NUL).
    let cr = unsafe { CM_Get_Device_IDW(devinst, &mut buf, 0) };
    (cr == CR_SUCCESS).then(|| utf16_to_string(&buf))
}

/// Walk parents until a device instance ID with the USB enumerator prefix
/// carries a parseable VID:PID. Bounded like the macOS `IORegistry` walk.
fn usb_ancestor_vid_pid(mut devinst: u32) -> Option<VidPid> {
    for _ in 0..16 {
        if let Some(id) = device_id(devinst)
            && id.to_ascii_uppercase().starts_with("USB\\")
            && let Some(vp) = vid_pid_from_pnp_id(&id)
        {
            return Some(vp);
        }
        let mut parent: u32 = 0;
        // SAFETY: local out-pointer; devinst from CM_Locate_DevNodeW/CM_Get_Parent.
        if unsafe { CM_Get_Parent(&raw mut parent, devinst, 0) } != CR_SUCCESS {
            return None;
        }
        devinst = parent;
    }
    None
}

/// First entry of `DEVPKEY_Device_LocationPaths` — a stable physical-port path
/// (e.g. `PCIROOT(0)#PCI(1400)#USBROOT(0)#USB(2)`), the Windows analogue of
/// the macOS locationID.
fn location_paths(devinst: u32) -> Option<String> {
    let mut prop_type = DEVPROPTYPE::default();
    let mut size: u32 = 0;
    // SAFETY: size probe, documented pattern.
    let _ = unsafe {
        CM_Get_DevNode_PropertyW(
            devinst,
            &DEVPKEY_Device_LocationPaths,
            &raw mut prop_type,
            None,
            &raw mut size,
            0,
        )
    };
    if size == 0 {
        return None;
    }
    let mut buf = vec![0u8; size as usize];
    // SAFETY: buffer sized by the probe above.
    let cr = unsafe {
        CM_Get_DevNode_PropertyW(
            devinst,
            &DEVPKEY_Device_LocationPaths,
            &raw mut prop_type,
            Some(buf.as_mut_ptr()),
            &raw mut size,
            0,
        )
    };
    if cr != CR_SUCCESS {
        return None;
    }
    split_multi_sz(&bytes_to_wide(&buf)).into_iter().next()
}

/// `GUID_DEVINTERFACE_VOLUME` docs: append a backslash to the interface path
/// and pass it as a mount point to obtain the `\\?\Volume{GUID}\` name.
fn volume_mount_and_label(interface_path: &str) -> (Option<PathBuf>, Option<String>) {
    let mount_input = to_wide(&format!("{interface_path}\\"));
    let mut volume_name = [0u16; 64];
    // SAFETY: NUL-terminated input; fixed output buffer (volume GUID names
    // are ~49 chars).
    if unsafe { GetVolumeNameForVolumeMountPointW(PCWSTR(mount_input.as_ptr()), &mut volume_name) }
        .is_err()
    {
        return (None, None);
    }
    let mut paths = [0u16; 1024];
    let mut len: u32 = 0;
    // SAFETY: NUL-terminated volume name from the call above.
    if unsafe {
        GetVolumePathNamesForVolumeNameW(
            PCWSTR(volume_name.as_ptr()),
            Some(&mut paths),
            &raw mut len,
        )
    }
    .is_err()
    {
        return (None, None);
    }
    let Some(root) = split_multi_sz(&paths).into_iter().next() else {
        // Volume exists but has no drive letter / mount folder yet.
        return (None, None);
    };
    let root_wide = to_wide(&root);
    let mut label_buf = [0u16; 256];
    // SAFETY: NUL-terminated root path; fixed label buffer per the API's
    // MAX_PATH+1 guidance.
    let label = unsafe {
        GetVolumeInformationW(
            PCWSTR(root_wide.as_ptr()),
            Some(&mut label_buf),
            None,
            None,
            None,
            None,
        )
    }
    .ok()
    .map(|()| utf16_to_string(&label_buf))
    .filter(|l| !l.is_empty());
    (Some(PathBuf::from(root)), label)
}

fn bytes_to_wide(buf: &[u8]) -> Vec<u16> {
    buf.chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect()
}
