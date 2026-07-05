use windows::Win32::Devices::Usb::GUID_DEVINTERFACE_USB_DEVICE;
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::System::Ioctl::GUID_DEVINTERFACE_VOLUME;
use windows::Win32::UI::WindowsAndMessaging::{
    DBT_DEVTYP_DEVICEINTERFACE, DEV_BROADCAST_DEVICEINTERFACE_W, DEVICE_NOTIFY_WINDOW_HANDLE,
    HDEVNOTIFY, RegisterDeviceNotificationW, UnregisterDeviceNotification,
};
use windows::core::GUID;
use yoke_volume::error::VolumeError;

/// Registration handles for USB-device and volume interface-class
/// notifications, delivered as `WM_DEVICECHANGE` to the owning window.
pub struct DeviceNotifications {
    usb: HDEVNOTIFY,
    volume: HDEVNOTIFY,
}

impl DeviceNotifications {
    pub fn register(hwnd: HWND) -> Result<Self, VolumeError> {
        let usb = register_interface_class(hwnd, GUID_DEVINTERFACE_USB_DEVICE)?;
        let volume = match register_interface_class(hwnd, GUID_DEVINTERFACE_VOLUME) {
            Ok(v) => v,
            Err(e) => {
                // SAFETY: `usb` came from a successful registration above.
                unsafe {
                    let _ = UnregisterDeviceNotification(usb);
                }
                return Err(e);
            }
        };
        Ok(Self { usb, volume })
    }
}

impl Drop for DeviceNotifications {
    fn drop(&mut self) {
        // SAFETY: both handles came from successful registrations in
        // `register` and are unregistered exactly once, here.
        unsafe {
            let _ = UnregisterDeviceNotification(self.usb);
            let _ = UnregisterDeviceNotification(self.volume);
        }
    }
}

fn register_interface_class(hwnd: HWND, class: GUID) -> Result<HDEVNOTIFY, VolumeError> {
    let filter = DEV_BROADCAST_DEVICEINTERFACE_W {
        dbcc_size: u32::try_from(size_of::<DEV_BROADCAST_DEVICEINTERFACE_W>())
            .expect("struct size fits u32"),
        dbcc_devicetype: DBT_DEVTYP_DEVICEINTERFACE.0,
        dbcc_reserved: 0,
        dbcc_classguid: class,
        dbcc_name: [0],
    };
    // SAFETY: `filter` outlives the call; RegisterDeviceNotificationW copies
    // the filter before returning. The HWND is valid for the caller's window.
    unsafe {
        RegisterDeviceNotificationW(
            HANDLE(hwnd.0),
            std::ptr::from_ref(&filter).cast(),
            DEVICE_NOTIFY_WINDOW_HANDLE,
        )
    }
    .map_err(|e| VolumeError::BackendInit(format!("RegisterDeviceNotificationW: {e}")))
}
