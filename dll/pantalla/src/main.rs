//! Monitor virtual para las pruebas: el juego corre ahí y la pantalla de verdad queda libre.
//! Reimplementa el protocolo IOCTL del Parsec Virtual Display Driver (ver nomi-san/parsec-vdd).
//!
//! Uso:
//!   pantalla version
//!   pantalla run <archivo_stop>   añade un monitor, lo mantiene vivo y lo quita al aparecer <archivo_stop>

use std::ffi::c_void;
use std::path::Path;
use std::time::Duration;

use windows::Win32::Devices::DeviceAndDriverInstallation::*;
use windows::Win32::Foundation::*;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::IO::{DeviceIoControl, GetOverlappedResultEx, OVERLAPPED};
use windows::Win32::System::Threading::CreateEventW;
use windows::core::{GUID, PCWSTR};

const VDD_GUID: GUID = GUID::from_u128(0x00b41627_04c4_429e_a26e_0265cf50c8fa);
const IOCTL_ADD: u32 = 0x0022e004;
const IOCTL_REMOVE: u32 = 0x0022a008;
const IOCTL_UPDATE: u32 = 0x0022a00c;
const IOCTL_VERSION: u32 = 0x0022e010;

fn open_device() -> Option<HANDLE> {
    unsafe {
        let info = SetupDiGetClassDevsW(Some(&VDD_GUID), PCWSTR::null(), None, DIGCF_PRESENT | DIGCF_DEVICEINTERFACE).ok()?;
        let mut iface = SP_DEVICE_INTERFACE_DATA { cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32, ..Default::default() };
        let mut i = 0;
        let mut result = None;
        while SetupDiEnumDeviceInterfaces(info, None, &VDD_GUID, i, &mut iface).is_ok() {
            i += 1;
            let mut size = 0u32;
            let _ = SetupDiGetDeviceInterfaceDetailW(info, &iface, None, 0, Some(&mut size), None);
            let mut buf = vec![0u8; size as usize];
            let detail = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
            (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            if SetupDiGetDeviceInterfaceDetailW(info, &iface, Some(detail), size, Some(&mut size), None).is_ok() {
                let path = PCWSTR((*detail).DevicePath.as_ptr());
                if let Ok(h) = CreateFileW(
                    path,
                    (GENERIC_READ | GENERIC_WRITE).0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    None,
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL | FILE_FLAG_NO_BUFFERING | FILE_FLAG_OVERLAPPED | FILE_FLAG_WRITE_THROUGH,
                    None,
                ) {
                    result = Some(h);
                    break;
                }
            }
        }
        let _ = SetupDiDestroyDeviceInfoList(info);
        result
    }
}

fn ioctl(dev: HANDLE, code: u32, data: &[u8]) -> Option<u32> {
    unsafe {
        let mut inb = [0u8; 32];
        inb[..data.len().min(32)].copy_from_slice(&data[..data.len().min(32)]);
        let mut out: u32 = 0;
        let mut ov = OVERLAPPED { hEvent: CreateEventW(None, true, false, PCWSTR::null()).ok()?, ..Default::default() };
        let _ = DeviceIoControl(dev, code, Some(inb.as_ptr() as *const c_void), 32, Some(&mut out as *mut u32 as *mut c_void), 4, None, Some(&mut ov));
        let mut n = 0u32;
        let ok = GetOverlappedResultEx(dev, &ov, &mut n, 5000, false).is_ok();
        let _ = CloseHandle(ov.hEvent);
        ok.then_some(out)
    }
}

/// Nombre GDI (\\.\DISPLAYn) del monitor virtual de Parsec, si está activo.
fn parsec_display() -> Option<Vec<u16>> {
    use windows::Win32::Graphics::Gdi::*;
    for i in 0..32 {
        let mut dd = DISPLAY_DEVICEW { cb: size_of::<DISPLAY_DEVICEW>() as u32, ..Default::default() };
        if !unsafe { EnumDisplayDevicesW(PCWSTR::null(), i, &mut dd, 0) }.as_bool() {
            break;
        }
        let desc = String::from_utf16_lossy(&dd.DeviceString);
        if desc.contains("Parsec") && dd.StateFlags.contains(DISPLAY_DEVICE_ACTIVE) {
            let n = dd.DeviceName.iter().position(|&c| c == 0).unwrap_or(32);
            let mut name = dd.DeviceName[..n].to_vec();
            name.push(0);
            return Some(name);
        }
    }
    None
}

/// Coloca el monitor virtual en diagonal respecto al principal (solo se tocan por una esquina),
/// para que el ratón no se escape a él. Devuelve (x, y, ancho, alto).
fn place_diagonal() -> Option<(i32, i32, u32, u32)> {
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    let name = parsec_display()?;
    let (pw, ph) = unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
    let mut dm = DEVMODEW { dmSize: size_of::<DEVMODEW>() as u16, ..Default::default() };
    unsafe {
        if !EnumDisplaySettingsW(PCWSTR(name.as_ptr()), ENUM_CURRENT_SETTINGS, &mut dm).as_bool() {
            return None;
        }
        dm.dmFields = DM_POSITION;
        dm.Anonymous1.Anonymous2.dmPosition.x = pw;
        dm.Anonymous1.Anonymous2.dmPosition.y = ph;
        let r = ChangeDisplaySettingsExW(PCWSTR(name.as_ptr()), Some(&dm), None, CDS_NORESET | CDS_UPDATEREGISTRY, None);
        let r2 = ChangeDisplaySettingsExW(PCWSTR::null(), None, None, CDS_TYPE(0), None);
        if r != DISP_CHANGE_SUCCESSFUL || r2 != DISP_CHANGE_SUCCESSFUL {
            eprintln!("aviso: no se pudo mover el monitor virtual ({r:?} / {r2:?})");
        }
        let _ = EnumDisplaySettingsW(PCWSTR(name.as_ptr()), ENUM_CURRENT_SETTINGS, &mut dm);
        let p = dm.Anonymous1.Anonymous2.dmPosition;
        Some((p.x, p.y, dm.dmPelsWidth, dm.dmPelsHeight))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(dev) = open_device() else {
        eprintln!("error: no encuentro el Parsec Virtual Display Driver");
        std::process::exit(1);
    };
    match args.get(1).map(String::as_str) {
        Some("version") => println!("VDD versión menor: {:?}", ioctl(dev, IOCTL_VERSION, &[])),
        Some("run") => {
            let stop = Path::new(args.get(2).map(String::as_str).unwrap_or("pantalla.stop")).to_path_buf();
            let _ = std::fs::remove_file(&stop);
            let idx = ioctl(dev, IOCTL_ADD, &[]).unwrap_or(u32::MAX);
            ioctl(dev, IOCTL_UPDATE, &[]);
            println!("monitor virtual añadido, índice {idx}");
            // El monitor tarda un momento en aparecer para GDI; mantenerlo vivo mientras tanto.
            let mut placed = None;
            for _ in 0..60 {
                ioctl(dev, IOCTL_UPDATE, &[]);
                std::thread::sleep(Duration::from_millis(50));
                if parsec_display().is_some() {
                    placed = place_diagonal();
                    break;
                }
            }
            // Para lanzar.ps1: dónde ha quedado el monitor.
            if let Some((x, y, w, h)) = placed {
                println!("posicion {x} {y} {w} {h}");
                let _ = std::fs::write(stop.with_extension("pos"), format!("{x} {y} {w} {h}"));
            }
            // El driver quita los monitores si no recibe UPDATE cada <100 ms.
            while !stop.exists() {
                ioctl(dev, IOCTL_UPDATE, &[]);
                std::thread::sleep(Duration::from_millis(50));
            }
            let be = (idx as u16).to_be_bytes();
            ioctl(dev, IOCTL_REMOVE, &be);
            ioctl(dev, IOCTL_UPDATE, &[]);
            let _ = std::fs::remove_file(&stop);
            println!("monitor virtual quitado");
        }
        _ => eprintln!("uso: pantalla version | pantalla run <archivo_stop>"),
    }
    unsafe {
        let _ = CloseHandle(dev);
    }
}
