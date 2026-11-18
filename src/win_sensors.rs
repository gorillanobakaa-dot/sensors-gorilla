//! Windows hardware sensor collector (Generic Windows + ThinkPad L15 Gen 3 extensions).
//! Pure Win32/IOCTL/PDH/WMI/COM + optional SHA-256-verified read-only PawnIO modules.
//! Zero TPFanControl code.

#![cfg(target_os = "windows")]

use std::collections::{BTreeMap, HashMap};
use std::ffi::c_void;
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::sha256;
use crate::win_ffi::*;
use crate::win_wmi;

static CTRL_C_PRESSED: AtomicBool = AtomicBool::new(false);

unsafe extern "system" fn ctrl_handler(_ctrl_type: DWORD) -> BOOL {
    CTRL_C_PRESSED.store(true, Ordering::SeqCst);
    1
}

pub fn install_ctrl_c() {
    unsafe {
        SetConsoleCtrlHandler(Some(ctrl_handler), 1);
    }
}

pub fn ctrl_c_pressed() -> bool {
    CTRL_C_PRESSED.load(Ordering::SeqCst)
}

/// Enable UTF-8 output and 24-bit TrueColor VT processing on Windows Terminal / ConHost.
pub fn init_console() {
    unsafe {
        SetConsoleOutputCP(CP_UTF8);
        let h_out = GetStdHandle(STD_OUTPUT_HANDLE);
        if !h_out.is_null() && h_out != INVALID_HANDLE_VALUE {
            let mut mode: DWORD = 0;
            if GetConsoleMode(h_out, &mut mode) != 0 {
                let _ = SetConsoleMode(h_out, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
            }
        }
    }
}

pub fn stdout_isatty() -> bool {
    unsafe {
        let h_out = GetStdHandle(STD_OUTPUT_HANDLE);
        if h_out.is_null() || h_out == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut mode: DWORD = 0;
        GetConsoleMode(h_out, &mut mode) != 0
    }
}

pub fn terminal_size() -> Option<(usize, usize)> {
    unsafe {
        let h_out = GetStdHandle(STD_OUTPUT_HANDLE);
        if h_out.is_null() || h_out == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut csbi = CONSOLE_SCREEN_BUFFER_INFO::default();
        if GetConsoleScreenBufferInfo(h_out, &mut csbi) != 0 {
            let cols = (csbi.srWindow.Right - csbi.srWindow.Left + 1).max(1) as usize;
            let rows = (csbi.srWindow.Bottom - csbi.srWindow.Top + 1).max(1) as usize;
            Some((cols, rows))
        } else {
            None
        }
    }
}

/// Wait up to `timeout_ms` ms, waking immediately on console window resize (`WINDOW_BUFFER_SIZE_EVENT`) or Ctrl+C.
pub fn wait_resize_or_timeout(timeout_ms: u32) {
    let start = Instant::now();
    let total = Duration::from_millis(timeout_ms as u64);
    let h_in = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    let mut last_size = terminal_size();

    loop {
        if ctrl_c_pressed() {
            return;
        }
        let elapsed = start.elapsed();
        if elapsed >= total {
            return;
        }
        let rem_ms = (total - elapsed).as_millis().min(200) as u32;
        if !h_in.is_null() && h_in != INVALID_HANDLE_VALUE && stdout_isatty() {
            let rc = unsafe { WaitForSingleObject(h_in, rem_ms) };
            if rc == WAIT_OBJECT_0 {
                let mut rec: [INPUT_RECORD; 16] = unsafe { std::mem::zeroed() };
                let mut read_cnt: DWORD = 0;
                if unsafe { PeekConsoleInputW(h_in, rec.as_mut_ptr(), 16, &mut read_cnt) } != 0 && read_cnt > 0 {
                    let mut saw_resize = false;
                    let mut saw_ctrl_c = false;
                    for i in 0..(read_cnt as usize) {
                        if rec[i].EventType == WINDOW_BUFFER_SIZE_EVENT {
                            saw_resize = true;
                        } else if rec[i].EventType == KEY_EVENT {
                            let ke = unsafe { rec[i].Event.KeyEvent };
                            if ke.bKeyDown != 0 && ke.uChar == 3 {
                                saw_ctrl_c = true;
                            }
                        }
                    }
                    if saw_resize {
                        let _ = unsafe { ReadConsoleInputW(h_in, rec.as_mut_ptr(), read_cnt, &mut read_cnt) };
                        return;
                    }
                    if saw_ctrl_c {
                        CTRL_C_PRESSED.store(true, Ordering::SeqCst);
                        return;
                    }
                    // Consume non-resize input records that would otherwise stay signaled
                    if !saw_resize {
                        let _ = unsafe { ReadConsoleInputW(h_in, rec.as_mut_ptr(), read_cnt, &mut read_cnt) };
                    }
                }
            }
        } else {
            std::thread::sleep(Duration::from_millis(rem_ms as u64));
        }
        let cur_size = terminal_size();
        if cur_size != last_size {
            last_size = cur_size;
            let _ = last_size;
            return;
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct CoreTopology {
    pub physical_cores: usize,
    pub logical_cpus: usize,
    pub p_cores: usize,
    pub e_cores: usize,
    /// For each logical CPU index -> (core_id, is_p_core)
    pub logical_map: Vec<(usize, bool)>,
    /// Representative logical CPU index per physical core -> (core_idx, is_p_core, first_logical_cpu)
    pub core_reps: Vec<(usize, bool, usize)>,
}

pub fn detect_cpu_topology() -> CoreTopology {
    let logical_cpus = unsafe { GetActiveProcessorCount(0xFFFF) as usize }.max(1);
    let mut topo = CoreTopology {
        physical_cores: 0,
        logical_cpus,
        p_cores: 0,
        e_cores: 0,
        logical_map: vec![(0, true); logical_cpus],
        core_reps: Vec::new(),
    };

    let mut len: DWORD = 0;
    unsafe {
        GetLogicalProcessorInformationEx(0, ptr::null_mut(), &mut len);
    }
    if len == 0 {
        topo.physical_cores = logical_cpus;
        for i in 0..logical_cpus {
            topo.logical_map[i] = (i, true);
            topo.core_reps.push((i, true, i));
        }
        return topo;
    }

    let mut buf = vec![0u8; len as usize];
    let ok = unsafe { GetLogicalProcessorInformationEx(0, buf.as_mut_ptr() as *mut c_void, &mut len) };
    if ok == 0 {
        return topo;
    }

    let mut offset = 0usize;
    let mut raw_cores: Vec<(u8, usize)> = Vec::new();
    while offset + 48 <= len as usize {
        let rel = u32::from_ne_bytes(buf[offset..offset + 4].try_into().unwrap());
        let size = u32::from_ne_bytes(buf[offset + 4..offset + 8].try_into().unwrap()) as usize;
        if size == 0 || offset + size > len as usize {
            break;
        }
        if rel == 0 && offset + 40 <= len as usize {
            let eff_class = buf[offset + 9];
            let mask = usize::from_ne_bytes(buf[offset + 32..offset + 40].try_into().unwrap());
            raw_cores.push((eff_class, mask));
        }
        offset += size;
    }

    let max_eff = raw_cores.iter().map(|(e, _)| *e).max().unwrap_or(0);
    let has_hybrid = raw_cores.iter().any(|(e, _)| *e != max_eff);
    topo.physical_cores = raw_cores.len();

    for (core_idx, (eff, mask)) in raw_cores.iter().enumerate() {
        let is_p = !has_hybrid || *eff == max_eff;
        if is_p {
            topo.p_cores += 1;
        } else {
            topo.e_cores += 1;
        }
        let mut first_l = None;
        for bit in 0..(usize::BITS as usize).min(logical_cpus) {
            if (mask & (1usize << bit)) != 0 {
                topo.logical_map[bit] = (core_idx, is_p);
                if first_l.is_none() {
                    first_l = Some(bit);
                }
            }
        }
        if let Some(fl) = first_l {
            topo.core_reps.push((core_idx, is_p, fl));
        }
    }
    topo
}

#[derive(Clone, Debug, Default)]
pub struct ThermalZoneReading {
    pub name: String,
    pub temp_c: f64,
    pub passive_pct: Option<f64>,
    pub throttle_reasons: Option<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct DriveReading {
    pub index: u32,
    pub model: String,
    pub bus_type: String,
    pub capacity_bytes: u64,
    pub temp_c: Option<f64>,
    pub extra_temps_c: Vec<f64>,
    pub life_pct: Option<u8>,
    pub pct_used: Option<u8>,
    pub avail_spare_pct: Option<u8>,
    pub spare_thresh_pct: Option<u8>,
    pub written_gb: Option<f64>,
    pub read_gb: Option<f64>,
    pub power_on_hours: Option<u64>,
    pub power_cycles: Option<u64>,
    pub unsafe_shutdowns: Option<u64>,
    pub media_errors: Option<u64>,
    pub critical_warning: Option<u8>,
}

/// Parse 512-byte NVMe SMART / Health Information Log (Log Page 0x02).
pub fn parse_nvme_smart_log(smart: &[u8], drive: &mut DriveReading) {
    if smart.len() < 216 {
        return;
    }
    drive.critical_warning = Some(smart[0]);
    let comp_k = u16::from_le_bytes([smart[1], smart[2]]);
    if comp_k > 100 && comp_k < 600 {
        drive.temp_c = Some(comp_k as f64 - 273.15);
    }
    drive.avail_spare_pct = Some(smart[3]);
    drive.spare_thresh_pct = Some(smart[4]);
    let used = smart[5];
    drive.pct_used = Some(used);
    drive.life_pct = Some(100u8.saturating_sub(used));

    let u128_at = |off: usize| -> u128 {
        let lo = u64::from_le_bytes(smart[off..off + 8].try_into().unwrap()) as u128;
        let hi = u64::from_le_bytes(smart[off + 8..off + 16].try_into().unwrap()) as u128;
        lo | (hi << 64)
    };
    let du_read = u128_at(32);
    let du_written = u128_at(48);
    drive.read_gb = Some((du_read as f64) * 512_000.0 / 1_000_000_000.0);
    drive.written_gb = Some((du_written as f64) * 512_000.0 / 1_000_000_000.0);
    drive.power_cycles = Some(u64::from_le_bytes(smart[112..120].try_into().unwrap()));
    drive.power_on_hours = Some(u64::from_le_bytes(smart[128..136].try_into().unwrap()));
    drive.unsafe_shutdowns = Some(u64::from_le_bytes(smart[144..152].try_into().unwrap()));
    drive.media_errors = Some(u64::from_le_bytes(smart[160..168].try_into().unwrap()));

    for i in 0..8 {
        let tk = u16::from_le_bytes([smart[200 + i * 2], smart[200 + i * 2 + 1]]);
        if tk > 100 && tk < 600 {
            drive.extra_temps_c.push(tk as f64 - 273.15);
        }
    }
}

fn bus_type_name(bus: u32) -> &'static str {
    match bus {
        1 => "SCSI",
        2 => "ATAPI",
        3 => "ATA",
        7 => "USB",
        8 => "RAID",
        10 => "SAS",
        11 => "SATA",
        17 => "NVMe",
        _ => "Storage",
    }
}

fn read_cstr_at(buf: &[u8], off: u32) -> String {
    let off = off as usize;
    if off == 0 || off >= buf.len() {
        return String::new();
    }
    let slice = &buf[off..];
    let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    String::from_utf8_lossy(&slice[..end]).trim().to_string()
}

pub fn probe_drives() -> Vec<DriveReading> {
    let mut drives = Vec::new();
    // Pre-fetch WMI physical disk sizes & reliability counters
    let wmi_disks = win_wmi::query(
        r"root\Microsoft\Windows\Storage",
        "SELECT DeviceId, FriendlyName, Size FROM MSFT_PhysicalDisk",
        &["DeviceId", "FriendlyName", "Size"],
    );
    let mut size_by_id: HashMap<u32, (String, u64)> = HashMap::new();
    for row in &wmi_disks {
        if let Some(id_s) = row.get("DeviceId").and_then(|v| v.as_str()) {
            if let Ok(id) = id_s.trim().parse::<u32>() {
                let name = row.get("FriendlyName").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let sz = row.get("Size").and_then(|v| v.as_i64()).unwrap_or(0).max(0) as u64;
                size_by_id.insert(id, (name, sz));
            }
        }
    }
    let wmi_rel = win_wmi::query(
        r"root\Microsoft\Windows\Storage",
        "SELECT DeviceId, Temperature, Wear, PowerOnHours FROM MSFT_StorageReliabilityCounter",
        &["DeviceId", "Temperature", "Wear", "PowerOnHours"],
    );
    let mut rel_by_id: HashMap<u32, (Option<f64>, Option<u8>, Option<u64>)> = HashMap::new();
    for row in &wmi_rel {
        if let Some(id_s) = row.get("DeviceId").and_then(|v| v.as_str()) {
            if let Ok(id) = id_s.trim().parse::<u32>() {
                let t = row.get("Temperature").and_then(|v| v.as_f64()).filter(|&x| x > 0.0 && x < 150.0);
                let wear = row.get("Wear").and_then(|v| v.as_i64()).map(|w| 100u8.saturating_sub(w.clamp(0, 100) as u8));
                let poh = row.get("PowerOnHours").and_then(|v| v.as_i64()).filter(|&x| x >= 0).map(|x| x as u64);
                rel_by_id.insert(id, (t, wear, poh));
            }
        }
    }

    for idx in 0..16u32 {
        let path = to_wide(&format!(r"\\.\PhysicalDrive{idx}"));
        let h = unsafe {
            CreateFileW(
                path.as_ptr(),
                0, // Attribute-only access works unprivileged for NVMe SMART & DeviceProperty!
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null_mut(),
                OPEN_EXISTING,
                0,
                ptr::null_mut(),
            )
        };
        if h.is_null() || h == INVALID_HANDLE_VALUE {
            continue;
        }
        let mut drv = DriveReading {
            index: idx,
            ..Default::default()
        };
        if let Some((nm, sz)) = size_by_id.get(&idx) {
            drv.model = nm.clone();
            drv.capacity_bytes = *sz;
        }

        // StorageDeviceProperty (PropertyId = 0)
        let q = STORAGE_PROPERTY_QUERY {
            PropertyId: 0,
            QueryType: 0,
            AdditionalParameters: [0],
        };
        let mut desc_buf = [0u8; 512];
        let mut ret: DWORD = 0;
        let ok = unsafe {
            DeviceIoControl(
                h,
                IOCTL_STORAGE_QUERY_PROPERTY,
                &q as *const _ as *const c_void,
                std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as DWORD,
                desc_buf.as_mut_ptr() as *mut c_void,
                desc_buf.len() as DWORD,
                &mut ret,
                ptr::null_mut(),
            )
        };
        if ok != 0 && ret >= 32 {
            let v_off = u32::from_le_bytes(desc_buf[12..16].try_into().unwrap());
            let p_off = u32::from_le_bytes(desc_buf[16..20].try_into().unwrap());
            let bus = u32::from_le_bytes(desc_buf[28..32].try_into().unwrap());
            drv.bus_type = bus_type_name(bus).to_string();
            let vend = read_cstr_at(&desc_buf[..ret as usize], v_off);
            let prod = read_cstr_at(&desc_buf[..ret as usize], p_off);
            let full = format!("{vend} {prod}").trim().to_string();
            if !full.is_empty() && drv.model.is_empty() {
                drv.model = full;
            }
        }

        // NVMe SMART Log Page 0x02 (PropertyId = 49)
        let mut qn = STORAGE_PROPERTY_QUERY_PROTOCOL {
            PropertyId: 49,
            QueryType: 0,
            ProtocolSpecific: STORAGE_PROTOCOL_SPECIFIC_DATA {
                ProtocolType: 3, // ProtocolTypeNvme
                DataType: 2,     // NVMeDataTypeLogPage
                ProtocolDataRequestValue: 2,
                ProtocolDataRequestSubValue: 0,
                ProtocolDataOffset: std::mem::size_of::<STORAGE_PROTOCOL_SPECIFIC_DATA>() as DWORD,
                ProtocolDataLength: 512,
                ..Default::default()
            },
            Buffer: [0; 512],
        };
        let out_sz = 8 + std::mem::size_of::<STORAGE_PROTOCOL_SPECIFIC_DATA>() + 512;
        let mut out_buf = vec![0u8; out_sz];
        let ok_nvme = unsafe {
            DeviceIoControl(
                h,
                IOCTL_STORAGE_QUERY_PROPERTY,
                &mut qn as *mut _ as *mut c_void,
                std::mem::size_of::<STORAGE_PROPERTY_QUERY_PROTOCOL>() as DWORD,
                out_buf.as_mut_ptr() as *mut c_void,
                out_sz as DWORD,
                &mut ret,
                ptr::null_mut(),
            )
        };
        if ok_nvme != 0 && ret as usize >= 48 + 216 {
            let off = 8 + std::mem::size_of::<STORAGE_PROTOCOL_SPECIFIC_DATA>();
            parse_nvme_smart_log(&out_buf[off..ret as usize], &mut drv);
        } else {
            // Fallback: StorageDeviceTemperatureProperty (PropertyId = 19)
            let qt = STORAGE_PROPERTY_QUERY {
                PropertyId: 19,
                QueryType: 0,
                AdditionalParameters: [0],
            };
            let mut tbuf = [0u8; 512];
            let ok_t = unsafe {
                DeviceIoControl(
                    h,
                    IOCTL_STORAGE_QUERY_PROPERTY,
                    &qt as *const _ as *const c_void,
                    std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as DWORD,
                    tbuf.as_mut_ptr() as *mut c_void,
                    tbuf.len() as DWORD,
                    &mut ret,
                    ptr::null_mut(),
                )
            };
            if ok_t != 0 && ret as usize >= 48 {
                let cnt = tbuf[14] as usize;
                if cnt > 0 {
                    let temp = i16::from_le_bytes([tbuf[26], tbuf[27]]);
                    if temp > -50 && temp < 150 {
                        drv.temp_c = Some(temp as f64);
                    }
                }
            }
        }

        if let Some((t, life, poh)) = rel_by_id.get(&idx) {
            if drv.temp_c.is_none() {
                drv.temp_c = *t;
            }
            if drv.life_pct.is_none() {
                drv.life_pct = *life;
            }
            if drv.power_on_hours.is_none() {
                drv.power_on_hours = *poh;
            }
        }

        unsafe {
            CloseHandle(h);
        }
        drives.push(drv);
    }
    drives
}

#[derive(Clone, Debug, Default)]
pub struct BatteryReading {
    pub present: bool,
    pub ac_online: bool,
    pub charging: bool,
    pub discharging: bool,
    pub battery_saver: bool,
    pub charge_pct: Option<f64>,
    pub chemistry: String,
    pub design_mwh: Option<u32>,
    pub full_mwh: Option<u32>,
    pub health_pct: Option<f64>,
    pub cycles: Option<u32>,
    pub remaining_mwh: Option<u32>,
    pub voltage_v: Option<f64>,
    pub rate_w: Option<f64>,
    pub temp_c: Option<f64>,
}

pub fn probe_battery() -> BatteryReading {
    let mut b = BatteryReading::default();
    let mut sps = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut sps) } != 0 {
        b.ac_online = sps.ACLineStatus == 1;
        b.battery_saver = (sps.SystemStatusFlag & 1) != 0;
        if sps.BatteryLifePercent <= 100 {
            b.charge_pct = Some(sps.BatteryLifePercent as f64);
        }
        if (sps.BatteryFlag & 128) == 0 {
            b.present = true;
        }
    }

    unsafe {
        let hdev = SetupDiGetClassDevsW(
            &GUID_DEVINTERFACE_BATTERY,
            ptr::null(),
            ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        );
        if !hdev.is_null() && hdev != INVALID_HANDLE_VALUE {
            let mut did = SP_DEVICE_INTERFACE_DATA {
                cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as DWORD,
                InterfaceClassGuid: GUID_DEVINTERFACE_BATTERY,
                Flags: 0,
                Reserved: 0,
            };
            if SetupDiEnumDeviceInterfaces(hdev, ptr::null_mut(), &GUID_DEVINTERFACE_BATTERY, 0, &mut did) != 0 {
                let mut req: DWORD = 0;
                SetupDiGetDeviceInterfaceDetailW(hdev, &mut did, ptr::null_mut(), 0, &mut req, ptr::null_mut());
                if req >= 8 {
                    let mut buf = vec![0u8; req as usize];
                    // On 64-bit Windows SP_DEVICE_INTERFACE_DETAIL_DATA_W::cbSize is 8
                    buf[0..4].copy_from_slice(&8u32.to_le_bytes());
                    if SetupDiGetDeviceInterfaceDetailW(
                        hdev,
                        &mut did,
                        buf.as_mut_ptr() as *mut c_void,
                        req,
                        ptr::null_mut(),
                        ptr::null_mut(),
                    ) != 0
                    {
                        let path_ptr = buf.as_ptr().add(4) as *const u16;
                        let mut bh = CreateFileW(
                            path_ptr,
                            GENERIC_READ | GENERIC_WRITE,
                            FILE_SHARE_READ | FILE_SHARE_WRITE,
                            ptr::null_mut(),
                            OPEN_EXISTING,
                            0,
                            ptr::null_mut(),
                        );
                        if bh.is_null() || bh == INVALID_HANDLE_VALUE {
                            bh = CreateFileW(
                                path_ptr,
                                GENERIC_READ,
                                FILE_SHARE_READ | FILE_SHARE_WRITE,
                                ptr::null_mut(),
                                OPEN_EXISTING,
                                0,
                                ptr::null_mut(),
                            );
                        }
                        if !bh.is_null() && bh != INVALID_HANDLE_VALUE {
                            let wait: ULONG = 0;
                            let mut tag: ULONG = 0;
                            let mut ret: DWORD = 0;
                            if DeviceIoControl(
                                bh,
                                IOCTL_BATTERY_QUERY_TAG,
                                &wait as *const _ as *const c_void,
                                4,
                                &mut tag as *mut _ as *mut c_void,
                                4,
                                &mut ret,
                                ptr::null_mut(),
                            ) != 0
                            {
                                b.present = true;
                                let bqi = BATTERY_QUERY_INFORMATION {
                                    BatteryTag: tag,
                                    InformationLevel: 0, // BatteryInformation
                                    AtRate: 0,
                                };
                                let mut bi = BATTERY_INFORMATION::default();
                                if DeviceIoControl(
                                    bh,
                                    IOCTL_BATTERY_QUERY_INFORMATION,
                                    &bqi as *const _ as *const c_void,
                                    std::mem::size_of::<BATTERY_QUERY_INFORMATION>() as DWORD,
                                    &mut bi as *mut _ as *mut c_void,
                                    std::mem::size_of::<BATTERY_INFORMATION>() as DWORD,
                                    &mut ret,
                                    ptr::null_mut(),
                                ) != 0
                                {
                                    b.chemistry = String::from_utf8_lossy(&bi.Chemistry).trim_end_matches('\0').trim().to_string();
                                    if bi.DesignedCapacity > 0 {
                                        b.design_mwh = Some(bi.DesignedCapacity);
                                    }
                                    if bi.FullChargedCapacity > 0 {
                                        b.full_mwh = Some(bi.FullChargedCapacity);
                                    }
                                    if bi.DesignedCapacity > 0 && bi.FullChargedCapacity > 0 {
                                        b.health_pct = Some(100.0 * bi.FullChargedCapacity as f64 / bi.DesignedCapacity as f64);
                                    }
                                    if bi.CycleCount > 0 {
                                        b.cycles = Some(bi.CycleCount);
                                    }
                                }

                                let bqi_temp = BATTERY_QUERY_INFORMATION {
                                    BatteryTag: tag,
                                    InformationLevel: 2, // BatteryTemperature
                                    AtRate: 0,
                                };
                                let mut temp_dk: ULONG = 0;
                                if DeviceIoControl(
                                    bh,
                                    IOCTL_BATTERY_QUERY_INFORMATION,
                                    &bqi_temp as *const _ as *const c_void,
                                    std::mem::size_of::<BATTERY_QUERY_INFORMATION>() as DWORD,
                                    &mut temp_dk as *mut _ as *mut c_void,
                                    4,
                                    &mut ret,
                                    ptr::null_mut(),
                                ) != 0
                                {
                                    let tc = temp_dk as f64 / 10.0 - 273.15;
                                    if tc > -20.0 && tc < 120.0 {
                                        b.temp_c = Some(tc);
                                    }
                                }

                                let bws = BATTERY_WAIT_STATUS {
                                    BatteryTag: tag,
                                    Timeout: 0,
                                    PowerState: 0,
                                    LowCapacity: 0,
                                    HighCapacity: 0,
                                };
                                let mut bs = BATTERY_STATUS::default();
                                if DeviceIoControl(
                                    bh,
                                    IOCTL_BATTERY_QUERY_STATUS,
                                    &bws as *const _ as *const c_void,
                                    std::mem::size_of::<BATTERY_WAIT_STATUS>() as DWORD,
                                    &mut bs as *mut _ as *mut c_void,
                                    std::mem::size_of::<BATTERY_STATUS>() as DWORD,
                                    &mut ret,
                                    ptr::null_mut(),
                                ) != 0
                                {
                                    b.ac_online = (bs.PowerState & 0x1) != 0;
                                    b.discharging = (bs.PowerState & 0x2) != 0;
                                    b.charging = (bs.PowerState & 0x4) != 0;
                                    if bs.Capacity != 0xFFFF_FFFF {
                                        b.remaining_mwh = Some(bs.Capacity);
                                        if let Some(full) = b.full_mwh {
                                            if full > 0 {
                                                b.charge_pct = Some(100.0 * bs.Capacity as f64 / full as f64);
                                            }
                                        }
                                    }
                                    if bs.Voltage != 0 && bs.Voltage != 0xFFFF_FFFF {
                                        b.voltage_v = Some(bs.Voltage as f64 / 1000.0);
                                    }
                                    if bs.Rate != i32::MIN {
                                        b.rate_w = Some((bs.Rate as f64).abs() / 1000.0);
                                    }
                                }
                            }
                            CloseHandle(bh);
                        }
                    }
                }
            }
            SetupDiDestroyDeviceInfoList(hdev);
        }
    }

    if b.cycles.is_none() {
        let rows = win_wmi::query(r"root\WMI", "SELECT CycleCount FROM BatteryCycleCount", &["CycleCount"]);
        if let Some(c) = rows.first().and_then(|r| r.get("CycleCount")).and_then(|v| v.as_i64()) {
            if c >= 0 {
                b.cycles = Some(c as u32);
            }
        }
    }
    b
}

pub fn active_power_plan() -> (String, Option<u32>, Option<u32>) {
    let mut scheme_ptr: *mut GUID = ptr::null_mut();
    if unsafe { PowerGetActiveScheme(ptr::null_mut(), &mut scheme_ptr) } != 0 || scheme_ptr.is_null() {
        return ("N/A".to_string(), None, None);
    }
    let mut name = "Custom".to_string();
    let mut sz: DWORD = 256;
    let mut buf = vec![0u8; sz as usize];
    if unsafe {
        PowerReadFriendlyName(
            ptr::null_mut(),
            scheme_ptr,
            ptr::null(),
            ptr::null(),
            buf.as_mut_ptr(),
            &mut sz,
        )
    } == 0
        && sz >= 2
    {
        let u16s: Vec<u16> = buf[..sz as usize]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&w| w != 0)
            .collect();
        let s = String::from_utf16_lossy(&u16s).trim().to_string();
        if !s.is_empty() {
            name = s;
        }
    }
    let mut boost: DWORD = 0;
    let mut max_pct: DWORD = 0;
    let boost_ok = unsafe {
        PowerReadACValueIndex(
            ptr::null_mut(),
            scheme_ptr,
            &GUID_PROCESSOR_SETTINGS_SUBGROUP,
            &GUID_PROCESSOR_PERF_BOOST_MODE,
            &mut boost,
        )
    } == 0;
    let max_ok = unsafe {
        PowerReadACValueIndex(
            ptr::null_mut(),
            scheme_ptr,
            &GUID_PROCESSOR_SETTINGS_SUBGROUP,
            &GUID_PROCESSOR_THROTTLE_MAXIMUM,
            &mut max_pct,
        )
    } == 0;
    unsafe {
        LocalFree(scheme_ptr as *mut c_void);
    }
    (name, if boost_ok { Some(boost) } else { None }, if max_ok { Some(max_pct) } else { None })
}

#[derive(Clone, Debug, Default)]
pub struct PdhSample {
    pub thermal_zones: Vec<ThermalZoneReading>,
    pub cpu_total_pct: Option<f64>,
    pub cpu_perf_pct: Option<f64>,
    pub per_cpu_perf_pct: BTreeMap<usize, f64>,
    pub per_cpu_util_pct: BTreeMap<usize, f64>,
    pub energy_pkg_w: Option<f64>,
    pub energy_pp0_w: Option<f64>,
    pub energy_pp1_w: Option<f64>,
    pub energy_dram_w: Option<f64>,
    pub system_power_w: Option<f64>,
    pub cpu_c1_pct: Option<f64>,
    pub cpu_c2_pct: Option<f64>,
    pub cpu_c3_pct: Option<f64>,
    pub perf_limit_flags: Option<u32>,
    pub perf_limit_pct: Option<f64>,
    pub gpu_dedicated_mb: f64,
    pub gpu_shared_mb: f64,
    pub gpu_3d_pct: f64,
    pub gpu_decode_pct: f64,
    pub gpu_encode_pct: f64,
    pub disk_read_mbps: f64,
    pub disk_write_mbps: f64,
    pub disk_time_pct: f64,
    pub net_rx_kbps: f64,
    pub net_tx_kbps: f64,
    pub net_link_mbps: f64,
    pub net_active_iface: String,
}

/// Persistent PDH query for sub-millisecond performance counter sampling across ticks.
pub struct PdhMonitor {
    query: PDH_HQUERY,
    c_tz_temp: PDH_HCOUNTER,
    c_tz_passive: PDH_HCOUNTER,
    c_tz_throttle: PDH_HCOUNTER,
    c_cpu_util: PDH_HCOUNTER,
    c_cpu_perf: PDH_HCOUNTER,
    c_energy_pwr: PDH_HCOUNTER,
    c_sys_pwr: PDH_HCOUNTER,
    c_cpu_c1: PDH_HCOUNTER,
    c_cpu_c2: PDH_HCOUNTER,
    c_cpu_c3: PDH_HCOUNTER,
    c_perf_lim_flags: PDH_HCOUNTER,
    c_perf_lim_pct: PDH_HCOUNTER,
    c_gpu_ded: PDH_HCOUNTER,
    c_gpu_shr: PDH_HCOUNTER,
    c_gpu_eng: PDH_HCOUNTER,
    c_disk_r: PDH_HCOUNTER,
    c_disk_w: PDH_HCOUNTER,
    c_disk_t: PDH_HCOUNTER,
    c_net_rx: PDH_HCOUNTER,
    c_net_tx: PDH_HCOUNTER,
    c_net_bw: PDH_HCOUNTER,
    primed: bool,
}

impl PdhMonitor {
    pub fn new() -> Option<Self> {
        let mut q: PDH_HQUERY = ptr::null_mut();
        if unsafe { PdhOpenQueryW(ptr::null(), 0, &mut q) } != 0 || q.is_null() {
            return None;
        }
        let add = |path: &str| -> PDH_HCOUNTER {
            let wp = to_wide(path);
            let mut c: PDH_HCOUNTER = ptr::null_mut();
            let rc = unsafe { PdhAddEnglishCounterW(q, wp.as_ptr(), 0, &mut c) };
            if rc == 0 { c } else { ptr::null_mut() }
        };
        let mut m = Self {
            query: q,
            c_tz_temp: add(r"\Thermal Zone Information(*)\High Precision Temperature"),
            c_tz_passive: add(r"\Thermal Zone Information(*)\% Passive Limit"),
            c_tz_throttle: add(r"\Thermal Zone Information(*)\Throttle Reasons"),
            c_cpu_util: add(r"\Processor Information(*)\% Processor Time"),
            c_cpu_perf: add(r"\Processor Information(*)\% Processor Performance"),
            c_energy_pwr: add(r"\Energy Meter(*)\Power"),
            c_sys_pwr: add(r"\Power Meter(_Total)\Power"),
            c_cpu_c1: add(r"\Processor Information(_Total)\% C1 Time"),
            c_cpu_c2: add(r"\Processor Information(_Total)\% C2 Time"),
            c_cpu_c3: add(r"\Processor Information(_Total)\% C3 Time"),
            c_perf_lim_flags: add(r"\Processor Information(_Total)\Performance Limit Flags"),
            c_perf_lim_pct: add(r"\Processor Information(_Total)\% Performance Limit"),
            c_gpu_ded: add(r"\GPU Adapter Memory(*)\Dedicated Usage"),
            c_gpu_shr: add(r"\GPU Adapter Memory(*)\Shared Usage"),
            c_gpu_eng: add(r"\GPU Engine(*)\Utilization Percentage"),
            c_disk_r: add(r"\PhysicalDisk(_Total)\Disk Read Bytes/sec"),
            c_disk_w: add(r"\PhysicalDisk(_Total)\Disk Write Bytes/sec"),
            c_disk_t: add(r"\PhysicalDisk(_Total)\% Disk Time"),
            c_net_rx: add(r"\Network Interface(*)\Bytes Received/sec"),
            c_net_tx: add(r"\Network Interface(*)\Bytes Sent/sec"),
            c_net_bw: add(r"\Network Interface(*)\Current Bandwidth"),
            primed: false,
        };
        unsafe {
            PdhCollectQueryData(m.query);
        }
        std::thread::sleep(Duration::from_millis(120));
        m.primed = true;
        Some(m)
    }

    fn read_array(counter: PDH_HCOUNTER) -> Vec<(String, f64)> {
        if counter.is_null() {
            return Vec::new();
        }
        let mut buf_sz: DWORD = 0;
        let mut count: DWORD = 0;
        let rc = unsafe {
            PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut buf_sz, &mut count, ptr::null_mut())
        };
        if rc != PDH_MORE_DATA || buf_sz == 0 {
            return Vec::new();
        }
        let elem_sz = std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>();
        let cap = (buf_sz as usize + elem_sz - 1) / elem_sz;
        let mut items: Vec<PDH_FMT_COUNTERVALUE_ITEM_W> = unsafe {
            let mut v = Vec::with_capacity(cap);
            v.set_len(cap);
            v
        };
        let rc2 = unsafe {
            PdhGetFormattedCounterArrayW(
                counter,
                PDH_FMT_DOUBLE,
                &mut buf_sz,
                &mut count,
                items.as_mut_ptr(),
            )
        };
        if rc2 != 0 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(count as usize);
        for item in &items[..(count as usize).min(items.len())] {
            if item.FmtValue.CStatus == 0 {
                let name = unsafe { from_wide_ptr(item.szName) };
                let val = unsafe { item.FmtValue.value.doubleValue };
                out.push((name, val));
            }
        }
        out
    }

    fn read_scalar(counter: PDH_HCOUNTER) -> Option<f64> {
        if counter.is_null() {
            return None;
        }
        let mut v = PDH_FMT_COUNTERVALUE {
            CStatus: 0,
            value: PDH_FMT_COUNTERVALUE_U { doubleValue: 0.0 },
        };
        if unsafe { PdhGetFormattedCounterValue(counter, PDH_FMT_DOUBLE, ptr::null_mut(), &mut v) } == 0
            && v.CStatus == 0
        {
            Some(unsafe { v.value.doubleValue })
        } else {
            None
        }
    }

    pub fn collect(&mut self) -> PdhSample {
        let mut s = PdhSample::default();
        if unsafe { PdhCollectQueryData(self.query) } != 0 {
            return s;
        }

        let mut tz_map: BTreeMap<String, ThermalZoneReading> = BTreeMap::new();
        for (name, val) in Self::read_array(self.c_tz_temp) {
            let c = val / 10.0 - 273.15;
            if c > -20.0 && c < 150.0 {
                tz_map.entry(name.clone()).or_insert_with(|| ThermalZoneReading {
                    name,
                    temp_c: c,
                    ..Default::default()
                });
            }
        }
        for (name, val) in Self::read_array(self.c_tz_passive) {
            if let Some(z) = tz_map.get_mut(&name) {
                z.passive_pct = Some(val);
            }
        }
        for (name, val) in Self::read_array(self.c_tz_throttle) {
            if let Some(z) = tz_map.get_mut(&name) {
                z.throttle_reasons = Some(val as u32);
            }
        }
        s.thermal_zones = tz_map.into_values().collect();

        for (name, val) in Self::read_array(self.c_cpu_util) {
            if name == "_Total" {
                s.cpu_total_pct = Some(val);
            } else if let Some((_, idx_s)) = name.split_once(',') {
                if let Ok(idx) = idx_s.parse::<usize>() {
                    s.per_cpu_util_pct.insert(idx, val);
                }
            }
        }
        for (name, val) in Self::read_array(self.c_cpu_perf) {
            if name == "_Total" {
                s.cpu_perf_pct = Some(val);
            } else if let Some((_, idx_s)) = name.split_once(',') {
                if let Ok(idx) = idx_s.parse::<usize>() {
                    s.per_cpu_perf_pct.insert(idx, val);
                }
            }
        }

        for (name, val) in Self::read_array(self.c_energy_pwr) {
            let low = name.to_ascii_lowercase();
            let w = val / 1000.0; // Windows Energy Meter reports milliwatts
            if w > 0.0 && w < 500.0 {
                if low.contains("pkg") {
                    s.energy_pkg_w = Some(w);
                } else if low.contains("pp0") {
                    s.energy_pp0_w = Some(w);
                } else if low.contains("pp1") {
                    s.energy_pp1_w = Some(w);
                } else if low.contains("dram") {
                    s.energy_dram_w = Some(w);
                }
            }
        }
        s.system_power_w = Self::read_scalar(self.c_sys_pwr)
            .map(|mw| mw / 1000.0)
            .filter(|&w| w > 0.0 && w < 1000.0);
        s.cpu_c1_pct = Self::read_scalar(self.c_cpu_c1);
        s.cpu_c2_pct = Self::read_scalar(self.c_cpu_c2);
        s.cpu_c3_pct = Self::read_scalar(self.c_cpu_c3);
        s.perf_limit_flags = Self::read_scalar(self.c_perf_lim_flags).map(|v| v as u32);
        s.perf_limit_pct = Self::read_scalar(self.c_perf_lim_pct);

        s.gpu_dedicated_mb = Self::read_array(self.c_gpu_ded).iter().map(|(_, v)| *v).sum::<f64>() / 1_048_576.0;
        s.gpu_shared_mb = Self::read_array(self.c_gpu_shr).iter().map(|(_, v)| *v).sum::<f64>() / 1_048_576.0;
        for (name, val) in Self::read_array(self.c_gpu_eng) {
            let low = name.to_ascii_lowercase();
            if low.contains("engtype_3d") {
                s.gpu_3d_pct += val;
            } else if low.contains("engtype_videodecode") {
                s.gpu_decode_pct += val;
            } else if low.contains("engtype_videoencode") {
                s.gpu_encode_pct += val;
            }
        }

        s.disk_read_mbps = Self::read_scalar(self.c_disk_r).unwrap_or(0.0) / 1_048_576.0;
        s.disk_write_mbps = Self::read_scalar(self.c_disk_w).unwrap_or(0.0) / 1_048_576.0;
        s.disk_time_pct = Self::read_scalar(self.c_disk_t).unwrap_or(0.0);

        let rx_arr = Self::read_array(self.c_net_rx);
        let tx_arr = Self::read_array(self.c_net_tx);
        let bw_arr = Self::read_array(self.c_net_bw);
        s.net_rx_kbps = rx_arr.iter().map(|(_, v)| *v).sum::<f64>() / 1024.0;
        s.net_tx_kbps = tx_arr.iter().map(|(_, v)| *v).sum::<f64>() / 1024.0;
        if let Some((best_name, best_bw)) = bw_arr
            .into_iter()
            .filter(|(n, bw)| *bw > 0.0 && !n.to_ascii_lowercase().contains("isatap") && !n.to_ascii_lowercase().contains("teredo"))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        {
            s.net_active_iface = best_name;
            s.net_link_mbps = best_bw / 1_000_000.0;
        }
        s
    }
}

impl Drop for PdhMonitor {
    fn drop(&mut self) {
        if !self.query.is_null() {
            unsafe {
                PdhCloseQuery(self.query);
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ExternalSensors {
    pub hwinfo_temps: BTreeMap<String, f64>,
    pub hwinfo_fans: BTreeMap<String, f64>,
    pub hwinfo_power: BTreeMap<String, f64>,
    pub lhm_temps: BTreeMap<String, f64>,
    pub lhm_fans: BTreeMap<String, f64>,
    pub lhm_power: BTreeMap<String, f64>,
    pub nvml: Option<NvmlReading>,
}

#[derive(Clone, Debug, Default)]
pub struct NvmlReading {
    pub name: String,
    pub temp_c: Option<f64>,
    pub fan_pct: Option<u32>,
    pub power_w: Option<f64>,
    pub sm_mhz: Option<u32>,
    pub mem_used_mb: Option<u64>,
    pub mem_total_mb: Option<u64>,
}

fn read_hwinfo_registry(ext: &mut ExternalSensors) {
    let subkey = to_wide(r"Software\HWiNFO64\VSB");
    let mut hkey: HKEY = ptr::null_mut();
    if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut hkey) } != 0 {
        return;
    }
    let mut raw: HashMap<String, String> = HashMap::new();
    let mut idx: DWORD = 0;
    loop {
        let mut name_buf = [0u16; 256];
        let mut name_len: DWORD = 256;
        let mut vtype: DWORD = 0;
        let mut data_buf = [0u8; 512];
        let mut data_len: DWORD = 512;
        let rc = unsafe {
            RegEnumValueW(
                hkey,
                idx,
                name_buf.as_mut_ptr(),
                &mut name_len,
                ptr::null_mut(),
                &mut vtype,
                data_buf.as_mut_ptr(),
                &mut data_len,
            )
        };
        if rc != 0 {
            break;
        }
        let k = String::from_utf16_lossy(&name_buf[..name_len as usize]);
        let val = if vtype == 1 && data_len >= 2 {
            let u16s: Vec<u16> = data_buf[..data_len as usize]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .take_while(|&w| w != 0)
                .collect();
            String::from_utf16_lossy(&u16s)
        } else if vtype == 4 && data_len >= 4 {
            u32::from_le_bytes(data_buf[0..4].try_into().unwrap()).to_string()
        } else {
            String::new()
        };
        raw.insert(k, val);
        idx += 1;
    }
    unsafe {
        RegCloseKey(hkey);
    }

    let mut i = 0;
    while let Some(label) = raw.get(&format!("Label{i}")) {
        if let Some(val_s) = raw.get(&format!("ValueRaw{i}")) {
            if let Ok(v) = val_s.trim().replace(',', ".").parse::<f64>() {
                let unit = raw.get(&format!("Value{i}")).map(|s| s.as_str()).unwrap_or("");
                if unit.contains('C') || label.contains("Temp") || label.contains("Package") {
                    ext.hwinfo_temps.entry(label.clone()).or_insert(v);
                } else if unit.contains("RPM") || label.contains("Fan") {
                    ext.hwinfo_fans.entry(label.clone()).or_insert(v);
                } else if unit.contains('W') || label.contains("Power") {
                    ext.hwinfo_power.entry(label.clone()).or_insert(v);
                }
            }
        }
        i += 1;
    }
}

fn read_lhm_wmi(ext: &mut ExternalSensors) {
    for ns in [r"root\LibreHardwareMonitor", r"root\OpenHardwareMonitor"] {
        let rows = win_wmi::query(ns, "SELECT Name, SensorType, Value FROM Sensor", &["Name", "SensorType", "Value"]);
        if rows.is_empty() {
            continue;
        }
        for r in rows {
            let name = r.get("Name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let stype = r.get("SensorType").and_then(|v| v.as_str()).unwrap_or("");
            let val = match r.get("Value").and_then(|v| v.as_f64()) {
                Some(v) => v,
                None => continue,
            };
            match stype {
                "Temperature" => {
                    ext.lhm_temps.entry(name).or_insert(val);
                }
                "Fan" => {
                    ext.lhm_fans.entry(name).or_insert(val);
                }
                "Power" => {
                    ext.lhm_power.entry(name).or_insert(val);
                }
                _ => {}
            }
        }
        break;
    }
}

fn read_nvml() -> Option<NvmlReading> {
    let dll = to_wide("nvml.dll");
    let h = unsafe { LoadLibraryW(dll.as_ptr()) };
    if h.is_null() {
        return None;
    }
    #[repr(C)]
    struct NvmlMem {
        total: u64,
        free: u64,
        used: u64,
    }
    unsafe {
        let p_init = GetProcAddress(h, b"nvmlInit_v2\0".as_ptr() as LPCSTR);
        let p_shut = GetProcAddress(h, b"nvmlShutdown\0".as_ptr() as LPCSTR);
        let p_cnt = GetProcAddress(h, b"nvmlDeviceGetCount_v2\0".as_ptr() as LPCSTR);
        let p_h = GetProcAddress(h, b"nvmlDeviceGetHandleByIndex_v2\0".as_ptr() as LPCSTR);
        if p_init.is_null() || p_cnt.is_null() || p_h.is_null() {
            FreeLibrary(h);
            return None;
        }
        let init: extern "C" fn() -> i32 = std::mem::transmute(p_init);
        let shut: Option<extern "C" fn() -> i32> = if p_shut.is_null() {
            None
        } else {
            Some(std::mem::transmute(p_shut))
        };
        let get_cnt: extern "C" fn(*mut u32) -> i32 = std::mem::transmute(p_cnt);
        let get_h: extern "C" fn(u32, *mut *mut c_void) -> i32 = std::mem::transmute(p_h);
        if init() != 0 {
            FreeLibrary(h);
            return None;
        }
        let mut cnt = 0u32;
        if get_cnt(&mut cnt) != 0 || cnt == 0 {
            if let Some(s) = shut {
                s();
            }
            FreeLibrary(h);
            return None;
        }
        let mut dev: *mut c_void = ptr::null_mut();
        if get_h(0, &mut dev) != 0 || dev.is_null() {
            if let Some(s) = shut {
                s();
            }
            FreeLibrary(h);
            return None;
        }
        let mut r = NvmlReading::default();
        let p_name = GetProcAddress(h, b"nvmlDeviceGetName\0".as_ptr() as LPCSTR);
        if !p_name.is_null() {
            let f: extern "C" fn(*mut c_void, *mut u8, u32) -> i32 = std::mem::transmute(p_name);
            let mut buf = [0u8; 96];
            if f(dev, buf.as_mut_ptr(), 96) == 0 {
                r.name = String::from_utf8_lossy(&buf).trim_end_matches('\0').trim().to_string();
            }
        }
        let p_temp = GetProcAddress(h, b"nvmlDeviceGetTemperature\0".as_ptr() as LPCSTR);
        if !p_temp.is_null() {
            let f: extern "C" fn(*mut c_void, u32, *mut u32) -> i32 = std::mem::transmute(p_temp);
            let mut t = 0u32;
            if f(dev, 0, &mut t) == 0 {
                r.temp_c = Some(t as f64);
            }
        }
        let p_fan = GetProcAddress(h, b"nvmlDeviceGetFanSpeed\0".as_ptr() as LPCSTR);
        if !p_fan.is_null() {
            let f: extern "C" fn(*mut c_void, *mut u32) -> i32 = std::mem::transmute(p_fan);
            let mut fan = 0u32;
            if f(dev, &mut fan) == 0 {
                r.fan_pct = Some(fan);
            }
        }
        let p_pwr = GetProcAddress(h, b"nvmlDeviceGetPowerUsage\0".as_ptr() as LPCSTR);
        if !p_pwr.is_null() {
            let f: extern "C" fn(*mut c_void, *mut u32) -> i32 = std::mem::transmute(p_pwr);
            let mut mw = 0u32;
            if f(dev, &mut mw) == 0 {
                r.power_w = Some(mw as f64 / 1000.0);
            }
        }
        let p_clk = GetProcAddress(h, b"nvmlDeviceGetClockInfo\0".as_ptr() as LPCSTR);
        if !p_clk.is_null() {
            let f: extern "C" fn(*mut c_void, u32, *mut u32) -> i32 = std::mem::transmute(p_clk);
            let mut mhz = 0u32;
            if f(dev, 0, &mut mhz) == 0 {
                r.sm_mhz = Some(mhz);
            }
        }
        let p_mem = GetProcAddress(h, b"nvmlDeviceGetMemoryInfo\0".as_ptr() as LPCSTR);
        if !p_mem.is_null() {
            let f: extern "C" fn(*mut c_void, *mut NvmlMem) -> i32 = std::mem::transmute(p_mem);
            let mut m = NvmlMem { total: 0, free: 0, used: 0 };
            if f(dev, &mut m) == 0 {
                r.mem_used_mb = Some(m.used / 1_048_576);
                r.mem_total_mb = Some(m.total / 1_048_576);
            }
        }
        if let Some(s) = shut {
            s();
        }
        FreeLibrary(h);
        Some(r)
    }
}

/// Optional read-only PawnIO silicon & ThinkPad EC telemetry (fail-closed SHA-256 verification).
#[derive(Clone, Debug, Default)]
pub struct PawnIoReadings {
    pub active: bool,
    pub permission_denied: bool,
    pub status: String,
    pub package_temp_c: Option<f64>,
    pub core_temps_c: Vec<(usize, bool, f64)>, // (core_id, is_p_core, temp_c)
    pub pch_temp_c: Option<f64>,
    pub rapl_pkg_w: Option<f64>,
    pub rapl_cores_w: Option<f64>,
    pub rapl_uncore_w: Option<f64>,
    pub throttle_pkg_now: bool,
    pub throttle_pkg_log: bool,
    pub throttle_reasons: Vec<String>,
    pub tp_fan_rpm: Option<u16>,
    pub tp_fan_mode: Option<String>,
    pub tp_ec_temps: Vec<(String, i32)>,
}

const HASH_INTEL_MSR: &str = "30a5a302283de1e080dfee99e686eca91d7e53e911095cb91a320fb961f1ce12";
const HASH_INTEL_PCH: &str = "478500ae78aebef3ae7a0b5fe4848d2d5737e8f7c5d51200c284a61c21d7865d";
const HASH_THINKPAD_EC: &str = "54bbf7f10d3511abfff167597485926ddf1587045f6ae849671cfaa85f97a1db";
const HASH_ACPI_EC: &str = "c38fd116e7aff4d1fdb0a494e296be0a6708e5a22fc72f14587442fb7f8f7906";
const HASH_AMD_FAM17: &str = "dae74615761b78bdf064dfb3e136252ddcc6fc727d88f14738d0e5800d427a91";

const EMBED_INTEL_MSR: &[u8] = include_bytes!("../pawnio/IntelMSR.bin");
const EMBED_INTEL_PCH: &[u8] = include_bytes!("../pawnio/IntelPCHThermal.bin");
const EMBED_THINKPAD_EC: &[u8] = include_bytes!("../pawnio/LpcThinkpadEC.bin");
const EMBED_ACPI_EC: &[u8] = include_bytes!("../pawnio/LpcACPIEC.bin");
const EMBED_AMD_FAM17: &[u8] = include_bytes!("../pawnio/AMDFamily17.bin");

fn load_verified_module(name: &str, expected_sha: &str, embedded: &[u8]) -> Option<Vec<u8>> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("pawnio").join(name));
            candidates.push(dir.join(name));
        }
    }
    for p in candidates {
        if p.is_file() {
            if let Ok(bytes) = fs::read(&p) {
                if sha256::sha256_hex(&bytes) == expected_sha {
                    return Some(bytes);
                }
            }
        }
    }
    // Always fall back to the SHA-256-verified module embedded in the executable
    if sha256::sha256_hex(embedded) == expected_sha {
        Some(embedded.to_vec())
    } else {
        None
    }
}

fn ensure_pawnio_driver_started() {
    let scm = unsafe { OpenSCManagerW(ptr::null(), ptr::null(), 0x0001) }; // SC_MANAGER_CONNECT
    if scm.is_null() {
        return;
    }
    let svc_name = to_wide("PawnIO");
    let svc = unsafe { OpenServiceW(scm, svc_name.as_ptr(), 0x0010) }; // SERVICE_START
    if !svc.is_null() {
        unsafe {
            let _ = StartServiceW(svc, 0, ptr::null());
            let _ = CloseServiceHandle(svc);
        }
    }
    unsafe {
        let _ = CloseServiceHandle(scm);
    }
}

// PawnIO direct DeviceIoControl constants: CTL_CODE(41394, func, METHOD_BUFFERED=0, FILE_ANY_ACCESS=0)
const PAWNIO_IOCTL_LOAD_BINARY: DWORD = (41394 << 16) | (0x821 << 2);
const PAWNIO_IOCTL_EXECUTE_FN: DWORD = (41394 << 16) | (0x841 << 2);

type FnPawnOpen = unsafe extern "system" fn(*mut HANDLE) -> HRESULT;
type FnPawnLoad = unsafe extern "system" fn(HANDLE, *const u8, usize) -> HRESULT;
type FnPawnExec = unsafe extern "system" fn(HANDLE, *const i8, *const u64, usize, *mut u64, usize, *mut usize) -> HRESULT;
type FnPawnClose = unsafe extern "system" fn(HANDLE) -> HRESULT;

struct PawnLib {
    hmod: HMODULE,
    open: FnPawnOpen,
    load: FnPawnLoad,
    exec: FnPawnExec,
    close: FnPawnClose,
}

impl PawnLib {
    fn load_dll() -> Option<Self> {
        let candidates = [r"C:\Program Files\PawnIO\PawnIOLib.dll", "PawnIOLib.dll"];
        for c in candidates {
            let p = Path::new(c);
            if c.contains('\\') && !p.exists() {
                continue;
            }
            let w = to_wide(c);
            let hmod = unsafe { LoadLibraryW(w.as_ptr()) };
            if !hmod.is_null() {
                unsafe {
                    let p_open = GetProcAddress(hmod, b"pawnio_open\0".as_ptr() as LPCSTR);
                    let p_load = GetProcAddress(hmod, b"pawnio_load\0".as_ptr() as LPCSTR);
                    let p_exec = GetProcAddress(hmod, b"pawnio_execute\0".as_ptr() as LPCSTR);
                    let p_close = GetProcAddress(hmod, b"pawnio_close\0".as_ptr() as LPCSTR);
                    if !p_open.is_null() && !p_load.is_null() && !p_exec.is_null() && !p_close.is_null() {
                        return Some(Self {
                            hmod,
                            open: std::mem::transmute(p_open),
                            load: std::mem::transmute(p_load),
                            exec: std::mem::transmute(p_exec),
                            close: std::mem::transmute(p_close),
                        });
                    }
                    FreeLibrary(hmod);
                }
            }
        }
        None
    }
}

impl Drop for PawnLib {
    fn drop(&mut self) {
        if !self.hmod.is_null() {
            unsafe { FreeLibrary(self.hmod) };
        }
    }
}

enum PawnConn<'a> {
    Dll(&'a PawnLib, HANDLE),
    Direct(HANDLE),
}

impl<'a> PawnConn<'a> {
    fn open(lib: Option<&'a PawnLib>) -> Result<Self, DWORD> {
        let try_direct = || -> Result<HANDLE, DWORD> {
            for dev in [r"\\.\GLOBALROOT\Device\PawnIO", r"\\.\PawnIO"] {
                let w = to_wide(dev);
                let h = unsafe {
                    CreateFileW(
                        w.as_ptr(),
                        GENERIC_READ | GENERIC_WRITE,
                        FILE_SHARE_READ | FILE_SHARE_WRITE,
                        ptr::null_mut(),
                        OPEN_EXISTING,
                        0,
                        ptr::null_mut(),
                    )
                };
                if !h.is_null() && h != INVALID_HANDLE_VALUE {
                    return Ok(h);
                }
            }
            Err(unsafe { GetLastError() })
        };

        if let Some(l) = lib {
            let mut h: HANDLE = ptr::null_mut();
            let mut hr = unsafe { (l.open)(&mut h) };
            let err = (hr as u32) & 0xFFFF;
            if hr < 0 && (err == 2 || err == 3) {
                ensure_pawnio_driver_started();
                hr = unsafe { (l.open)(&mut h) };
            }
            if hr >= 0 && !h.is_null() {
                return Ok(PawnConn::Dll(l, h));
            }
            // Fall through to direct DeviceIoControl in case PawnIOLib.dll returned an error
            if let Ok(dh) = try_direct() {
                return Ok(PawnConn::Direct(dh));
            }
            return Err(hr as DWORD);
        }

        match try_direct() {
            Ok(h) => Ok(PawnConn::Direct(h)),
            Err(2 | 3) => {
                ensure_pawnio_driver_started();
                try_direct().map(PawnConn::Direct)
            }
            Err(e) => Err(e),
        }
    }

    fn load_blob(&self, blob: &[u8]) -> Result<(), i32> {
        match self {
            PawnConn::Dll(l, h) => {
                let hr = unsafe { (l.load)(*h, blob.as_ptr(), blob.len()) };
                if hr >= 0 { Ok(()) } else { Err(hr) }
            }
            PawnConn::Direct(h) => {
                let mut ret: DWORD = 0;
                let ok = unsafe {
                    DeviceIoControl(
                        *h,
                        PAWNIO_IOCTL_LOAD_BINARY,
                        blob.as_ptr() as *const c_void,
                        blob.len() as DWORD,
                        ptr::null_mut(),
                        0,
                        &mut ret,
                        ptr::null_mut(),
                    )
                };
                if ok != 0 {
                    Ok(())
                } else {
                    Err(unsafe { GetLastError() as i32 })
                }
            }
        }
    }

    fn exec(&self, name: &str, inp: &[u64], out: &mut [u64]) -> bool {
        match self {
            PawnConn::Dll(l, h) => {
                let mut cname = name.as_bytes().to_vec();
                cname.push(0);
                let mut ret = 0usize;
                unsafe {
                    (l.exec)(
                        *h,
                        cname.as_ptr() as *const i8,
                        inp.as_ptr(),
                        inp.len(),
                        out.as_mut_ptr(),
                        out.len(),
                        &mut ret,
                    ) >= 0
                }
            }
            PawnConn::Direct(h) => {
                // Input buffer: 32-byte zero-padded function name + u64 args
                let mut in_buf = vec![0u8; 32 + inp.len() * 8];
                let nb = name.as_bytes();
                let n = nb.len().min(31);
                in_buf[..n].copy_from_slice(&nb[..n]);
                for (i, val) in inp.iter().enumerate() {
                    in_buf[32 + i * 8..32 + (i + 1) * 8].copy_from_slice(&val.to_le_bytes());
                }
                let mut ret: DWORD = 0;
                let ok = unsafe {
                    DeviceIoControl(
                        *h,
                        PAWNIO_IOCTL_EXECUTE_FN,
                        in_buf.as_ptr() as *const c_void,
                        in_buf.len() as DWORD,
                        out.as_mut_ptr() as *mut c_void,
                        (out.len() * 8) as DWORD,
                        &mut ret,
                        ptr::null_mut(),
                    )
                };
                ok != 0 && (ret as usize) >= out.len() * 8
            }
        }
    }
}

impl<'a> Drop for PawnConn<'a> {
    fn drop(&mut self) {
        match self {
            PawnConn::Dll(l, h) => unsafe {
                let _ = (l.close)(*h);
            },
            PawnConn::Direct(h) => unsafe {
                let _ = CloseHandle(*h);
            },
        }
    }
}

fn read_ec_byte_readonly(conn: &PawnConn, data_port: u64, cmd_port: u64, offset: u8) -> Option<u8> {
    let pio_read = |port: u64| -> Option<u8> {
        let mut out = [0u64; 1];
        if conn.exec("ioctl_pio_read", &[port], &mut out) {
            Some((out[0] & 0xFF) as u8)
        } else {
            None
        }
    };
    let pio_write = |port: u64, val: u64| -> bool {
        let mut out = [];
        conn.exec("ioctl_pio_write", &[port, val], &mut out)
    };

    for _ in 0..4 {
        let st = pio_read(cmd_port)?;
        if (st & 0x01) == 0 {
            break;
        }
        let _ = pio_read(data_port);
    }
    for _ in 0..250 {
        if (pio_read(cmd_port)? & 0x02) == 0 {
            break;
        }
        std::thread::sleep(Duration::from_micros(20));
    }
    if (pio_read(cmd_port)? & 0x02) != 0 {
        return None;
    }
    // READ command ONLY (0x80) — NEVER 0x81 write
    if !pio_write(cmd_port, 0x80) {
        return None;
    }
    for _ in 0..250 {
        if (pio_read(cmd_port)? & 0x02) == 0 {
            break;
        }
        std::thread::sleep(Duration::from_micros(20));
    }
    if !pio_write(data_port, offset as u64) {
        return None;
    }
    for _ in 0..250 {
        if (pio_read(cmd_port)? & 0x01) != 0 {
            return pio_read(data_port);
        }
        std::thread::sleep(Duration::from_micros(20));
    }
    None
}

pub fn probe_pawnio(topo: &CoreTopology, cpu_name: &str, read_thinkpad_ec: bool) -> PawnIoReadings {
    let mut r = PawnIoReadings::default();
    let lib = PawnLib::load_dll();
    let mut loaded_parts: Vec<&str> = Vec::new();
    let is_amd = cpu_name.to_ascii_uppercase().contains("AMD");

    // Verify we can open PawnIO at all (via PawnIOLib.dll or direct DeviceIoControl)
    match PawnConn::open(lib.as_ref()) {
        Err(e) => {
            let low = e & 0xFFFF;
            if e == (0x8007_0005) || low == 5 {
                r.permission_denied = true;
                r.status = "Run as Administrator for CPU die/MSR temps, RAPL watts & EC Fan RPM".to_string();
            } else if low == 2 || low == 3 {
                r.status = "PawnIO driver not installed (winget install namazso.PawnIO)".to_string();
            } else {
                r.status = format!("PawnIO open failed (err 0x{e:08X})");
            }
            return r;
        }
        Ok(_probe_conn) => {}
    }

    // 1A. Intel MSR (read-only RDMSR)
    if !is_amd {
        if let Some(msr_blob) = load_verified_module("IntelMSR.bin", HASH_INTEL_MSR, EMBED_INTEL_MSR) {
            if let Ok(conn) = PawnConn::open(lib.as_ref()) {
                match conn.load_blob(&msr_blob) {
                    Ok(()) => {
                        r.active = true;
                        loaded_parts.push("IntelMSR");
                        let read_msr = |msr: u64| -> Option<u64> {
                            let mut out = [0u64; 1];
                            if conn.exec("ioctl_read_msr", &[msr], &mut out) {
                                Some(out[0])
                            } else {
                                None
                            }
                        };

                        let tjmax = read_msr(0x1A2)
                            .map(|v| ((v >> 16) & 0xFF) as f64)
                            .filter(|&t| t > 50.0 && t < 130.0)
                            .unwrap_or(100.0);

                        if let Some(pkg) = read_msr(0x1B1) {
                            if ((pkg >> 31) & 1) != 0 {
                                let dist = ((pkg >> 16) & 0x7F) as f64;
                                r.package_temp_c = Some(tjmax - dist);
                            }
                            r.throttle_pkg_now = (pkg & 1) != 0;
                            r.throttle_pkg_log = ((pkg >> 1) & 1) != 0;
                        }

                        if let Some(lim) = read_msr(0x64F) {
                            for (bit, label) in [
                                (0, "PROCHOT"),
                                (1, "Thermal"),
                                (5, "RATL"),
                                (6, "VR Thermal"),
                                (10, "Pkg Power Limit"),
                                (11, "Current/EDP"),
                            ] {
                                if ((lim >> bit) & 1) != 0 {
                                    r.throttle_reasons.push(label.to_string());
                                }
                            }
                        }

                        let thr = unsafe { GetCurrentThread() };
                        for (core_idx, is_p, logical_cpu) in &topo.core_reps {
                            if *logical_cpu < (usize::BITS as usize) {
                                let prev = unsafe { SetThreadAffinityMask(thr, 1usize << *logical_cpu) };
                                if prev != 0 {
                                    if let Some(ts) = read_msr(0x19C) {
                                        if ((ts >> 31) & 1) != 0 {
                                            let dist = ((ts >> 16) & 0x7F) as f64;
                                            r.core_temps_c.push((*core_idx, *is_p, tjmax - dist));
                                        }
                                    }
                                    unsafe {
                                        SetThreadAffinityMask(thr, prev);
                                    }
                                }
                            }
                        }

                        if let Some(unit_raw) = read_msr(0x606) {
                            let e_unit = 0.5_f64.powi(((unit_raw >> 8) & 0x1F) as i32);
                            let e1_pkg = read_msr(0x611).map(|v| v & 0xFFFF_FFFF);
                            let e1_pp0 = read_msr(0x639).map(|v| v & 0xFFFF_FFFF);
                            let e1_pp1 = read_msr(0x641).map(|v| v & 0xFFFF_FFFF);
                            let t1 = Instant::now();
                            std::thread::sleep(Duration::from_millis(150));
                            let e2_pkg = read_msr(0x611).map(|v| v & 0xFFFF_FFFF);
                            let e2_pp0 = read_msr(0x639).map(|v| v & 0xFFFF_FFFF);
                            let e2_pp1 = read_msr(0x641).map(|v| v & 0xFFFF_FFFF);
                            let dt = t1.elapsed().as_secs_f64();
                            if dt > 0.0 {
                                if let (Some(a), Some(b)) = (e1_pkg, e2_pkg) {
                                    let d = if b >= a { b - a } else { (u32::MAX as u64 - a) + b + 1 };
                                    r.rapl_pkg_w = Some(d as f64 * e_unit / dt);
                                }
                                if let (Some(a), Some(b)) = (e1_pp0, e2_pp0) {
                                    let d = if b >= a { b - a } else { (u32::MAX as u64 - a) + b + 1 };
                                    r.rapl_cores_w = Some(d as f64 * e_unit / dt);
                                }
                                if let (Some(a), Some(b)) = (e1_pp1, e2_pp1) {
                                    let d = if b >= a { b - a } else { (u32::MAX as u64 - a) + b + 1 };
                                    r.rapl_uncore_w = Some(d as f64 * e_unit / dt);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        r.status = format!("IntelMSR load err 0x{:08X}", e as u32);
                    }
                }
            }
        }
    } else if let Some(amd_blob) = load_verified_module("AMDFamily17.bin", HASH_AMD_FAM17, EMBED_AMD_FAM17) {
        // 1B. AMD Ryzen Family 17h-1Ah (Tctl/Tdie + RAPL)
        if let Ok(conn) = PawnConn::open(lib.as_ref()) {
            if conn.load_blob(&amd_blob).is_ok() {
                r.active = true;
                loaded_parts.push("AMDFamily17");
                let mtx_name = to_wide(r"Global\Access_PCI");
                let mtx = unsafe { CreateMutexW(ptr::null_mut(), 0, mtx_name.as_ptr()) };
                let locked = if !mtx.is_null() {
                    unsafe { WaitForSingleObject(mtx, 200) == WAIT_OBJECT_0 }
                } else {
                    true
                };
                if locked {
                    let mut out = [0u64; 1];
                    if conn.exec("ioctl_read_smn", &[0x0005_9800], &mut out) {
                        let raw = out[0] as u32;
                        let mut temp = ((raw >> 21) as f64) * 0.125;
                        if (raw & (1 << 19)) != 0 {
                            temp -= 49.0;
                        }
                        if temp > -20.0 && temp < 140.0 {
                            r.package_temp_c = Some(temp);
                        }
                    }
                    if !mtx.is_null() {
                        unsafe { ReleaseMutex(mtx) };
                    }
                }
                if !mtx.is_null() {
                    unsafe { CloseHandle(mtx) };
                }
                let read_msr = |msr: u64| -> Option<u64> {
                    let mut out = [0u64; 1];
                    if conn.exec("ioctl_read_msr", &[msr], &mut out) { Some(out[0]) } else { None }
                };
                if let Some(unit_raw) = read_msr(0xC001_0299) {
                    let e_unit = 0.5_f64.powi(((unit_raw >> 8) & 0x1F) as i32);
                    let p1 = read_msr(0xC001_029B).map(|v| v & 0xFFFF_FFFF);
                    let t1 = Instant::now();
                    std::thread::sleep(Duration::from_millis(150));
                    let p2 = read_msr(0xC001_029B).map(|v| v & 0xFFFF_FFFF);
                    let dt = t1.elapsed().as_secs_f64();
                    if dt > 0.0 {
                        if let (Some(a), Some(b)) = (p1, p2) {
                            let d = if b >= a { b - a } else { (u32::MAX as u64 - a) + b + 1 };
                            r.rapl_pkg_w = Some(d as f64 * e_unit / dt);
                        }
                    }
                }
            }
        }
    }

    // 2. Intel PCH Thermal (read-only MMIO)
    if !is_amd {
        if let Some(pch_blob) = load_verified_module("IntelPCHThermal.bin", HASH_INTEL_PCH, EMBED_INTEL_PCH) {
            if let Ok(conn) = PawnConn::open(lib.as_ref()) {
                if conn.load_blob(&pch_blob).is_ok() {
                    loaded_parts.push("IntelPCH");
                    let mut out = [0u64; 2];
                    if conn.exec("ioctl_read_raw", &[], &mut out) {
                        let raw = out[0];
                        let route = out[1];
                        if route == 1 {
                            let c = (raw & 0x1FF) as f64 * 0.5 - 50.0;
                            if c > 0.0 && c < 130.0 {
                                r.pch_temp_c = Some(c);
                            }
                        } else if route == 2 && (raw & (1 << 31)) != 0 {
                            let c = (raw & 0xFF) as f64;
                            if c > 0.0 && c < 130.0 {
                                r.pch_temp_c = Some(c);
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. ThinkPad EC Read-Only (LpcThinkpadEC 0x1600/0x1604 first; falls back to LpcACPIEC 0x62/0x66 if no EC bytes respond; NEVER writes 0x81)
    if read_thinkpad_ec {
        let probe_ec_bank = |conn: &PawnConn, dport: u64, cport: u64, r: &mut PawnIoReadings| -> bool {
            let mut got_any = false;
            let mtx_name = to_wide(r"Global\Access_EC");
            let mtx = unsafe { CreateMutexW(ptr::null_mut(), 0, mtx_name.as_ptr()) };
            let locked = if !mtx.is_null() {
                unsafe { WaitForSingleObject(mtx, 200) == WAIT_OBJECT_0 }
            } else {
                true
            };
            if locked {
                if let (Some(lo), Some(hi)) = (
                    read_ec_byte_readonly(conn, dport, cport, 0x84),
                    read_ec_byte_readonly(conn, dport, cport, 0x85),
                ) {
                    let rpm = ((hi as u16) << 8) | (lo as u16);
                    if rpm < 15_000 && !(lo == 0xFF && hi == 0xFF) {
                        r.tp_fan_rpm = Some(rpm);
                        got_any = true;
                    }
                }
                if let Some(ctl) = read_ec_byte_readonly(conn, dport, cport, 0x2F) {
                    if ctl != 0xFF {
                        got_any = true;
                        r.tp_fan_mode = Some(match ctl {
                            0x80 => "BIOS auto (0x80)".to_string(),
                            0x40 => "Disengaged (0x40)".to_string(),
                            lvl if lvl <= 7 => format!("Level {lvl}"),
                            other => format!("0x{other:02X}"),
                        });
                    }
                }
                for (off, label) in [
                    (0x78, "EC CPU"),
                    (0x79, "EC APS"),
                    (0x7A, "EC CRD"),
                    (0x7B, "EC GPU"),
                    (0x7E, "EC BAT"),
                    (0xC0, "EC BUS"),
                    (0xC1, "EC PCI"),
                    (0xC2, "EC PWR"),
                ] {
                    if let Some(b) = read_ec_byte_readonly(conn, dport, cport, off) {
                        let t = (b as i8) as i32;
                        if t > 0 && t < 125 {
                            r.tp_ec_temps.push((label.to_string(), t));
                            got_any = true;
                        }
                    }
                }
                if !mtx.is_null() {
                    unsafe { ReleaseMutex(mtx) };
                }
            }
            if !mtx.is_null() {
                unsafe { CloseHandle(mtx) };
            }
            got_any
        };

        let mut ec_done = false;
        if let Some(tp_blob) = load_verified_module("LpcThinkpadEC.bin", HASH_THINKPAD_EC, EMBED_THINKPAD_EC) {
            if let Ok(conn) = PawnConn::open(lib.as_ref()) {
                if conn.load_blob(&tp_blob).is_ok() && probe_ec_bank(&conn, 0x1600, 0x1604, &mut r) {
                    loaded_parts.push("ThinkPadEC");
                    ec_done = true;
                }
            }
        }
        if !ec_done {
            if let Some(acpi_blob) = load_verified_module("LpcACPIEC.bin", HASH_ACPI_EC, EMBED_ACPI_EC) {
                if let Ok(conn) = PawnConn::open(lib.as_ref()) {
                    if conn.load_blob(&acpi_blob).is_ok() && probe_ec_bank(&conn, 0x62, 0x66, &mut r) {
                        loaded_parts.push("ACPIEC");
                    }
                }
            }
        }
    }

    if !loaded_parts.is_empty() {
        r.status = format!("PawnIO ({})", loaded_parts.join(" + "));
    } else if r.status.is_empty() {
        r.status = "PawnIO open, no compatible silicon module".to_string();
    }
    r
}

#[derive(Clone, Debug, Default)]
pub struct WindowsTelemetry {
    pub sys_manufacturer: String,
    pub sys_model: String,
    pub sys_family: String,
    pub bios_version: String,
    pub cpu_name: String,
    pub gpu_name: String,
    pub gpu_driver: String,
    pub topology: CoreTopology,
    pub ppi: Vec<PROCESSOR_POWER_INFORMATION>,
    pub pdh: PdhSample,
    pub drives: Vec<DriveReading>,
    pub battery: BatteryReading,
    pub mem_total_gb: f64,
    pub mem_used_gb: f64,
    pub commit_used_gb: f64,
    pub commit_limit_gb: f64,
    pub proc_count: u32,
    pub thread_count: u32,
    pub uptime_secs: u64,
    pub power_plan: String,
    pub boost_mode: Option<u32>,
    pub max_cpu_pct: Option<u32>,
    pub c_drive_free_gb: f64,
    pub lenovo_odometer: Option<LenovoOdometer>,
    pub ram_spec: String,
    pub display_mode: String,
    pub brightness_pct: Option<i64>,
    pub has_wwan_gnss: bool,
    pub battery_temp_c: Option<f64>,
    pub wmi_probes: Vec<(String, f64)>,
    pub hardware_badges: Vec<String>,
    pub external: ExternalSensors,
    pub pawnio: PawnIoReadings,
}

#[derive(Clone, Debug, Default)]
pub struct LenovoOdometer {
    pub thermal_events: Option<i64>,
    pub shock_events: Option<i64>,
    pub cpu_uptime_hrs: Option<i64>,
    pub battery_cycles: String,
    pub ssd_rw_count: String,
}

/// Collect a complete snapshot of all Windows hardware sensors.
pub fn collect(pdh_mon: &mut Option<PdhMonitor>, is_l15: bool, allow_ec: bool) -> WindowsTelemetry {
    let mut t = WindowsTelemetry::default();
    t.topology = detect_cpu_topology();

    // System / CPU / GPU identity via WMI COM
    if let Some(cs) = win_wmi::query(
        r"root\CIMV2",
        "SELECT Manufacturer, Model, SystemFamily FROM Win32_ComputerSystem",
        &["Manufacturer", "Model", "SystemFamily"],
    )
    .into_iter()
    .next()
    {
        t.sys_manufacturer = cs.get("Manufacturer").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
        t.sys_model = cs.get("Model").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
        t.sys_family = cs.get("SystemFamily").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    }
    if let Some(bios) = win_wmi::query(r"root\CIMV2", "SELECT SMBIOSBIOSVersion FROM Win32_BIOS", &["SMBIOSBIOSVersion"])
        .into_iter()
        .next()
    {
        t.bios_version = bios.get("SMBIOSBIOSVersion").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    }
    if let Some(cpu) = win_wmi::query(r"root\CIMV2", "SELECT Name FROM Win32_Processor", &["Name"])
        .into_iter()
        .next()
    {
        t.cpu_name = cpu.get("Name").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    }
    if let Some(gpu) = win_wmi::query(
        r"root\CIMV2",
        "SELECT Name, DriverVersion, CurrentHorizontalResolution, CurrentVerticalResolution, CurrentRefreshRate FROM Win32_VideoController",
        &["Name", "DriverVersion", "CurrentHorizontalResolution", "CurrentVerticalResolution", "CurrentRefreshRate"],
    )
    .into_iter()
    .next()
    {
        t.gpu_name = gpu.get("Name").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
        t.gpu_driver = gpu.get("DriverVersion").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
        if let (Some(w), Some(h), Some(hz)) = (
            gpu.get("CurrentHorizontalResolution").and_then(|v| v.as_i64()),
            gpu.get("CurrentVerticalResolution").and_then(|v| v.as_i64()),
            gpu.get("CurrentRefreshRate").and_then(|v| v.as_i64()),
        ) {
            t.display_mode = format!("{w}x{h} @ {hz}Hz");
        }
    }
    if let Some(br) = win_wmi::query(r"root\WMI", "SELECT CurrentBrightness FROM WmiMonitorBrightness", &["CurrentBrightness"])
        .into_iter()
        .next()
    {
        t.brightness_pct = br.get("CurrentBrightness").and_then(|v| v.as_i64());
    }
    let dimms = win_wmi::query(
        r"root\CIMV2",
        "SELECT Capacity, Manufacturer, Speed FROM Win32_PhysicalMemory",
        &["Capacity", "Manufacturer", "Speed"],
    );
    if !dimms.is_empty() {
        let count = dimms.len();
        let per_gb = dimms.first().and_then(|d| d.get("Capacity")).and_then(|v| v.as_i64()).unwrap_or(0) / 1_073_741_824;
        let mfg = dimms.first().and_then(|d| d.get("Manufacturer")).and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
        let spd = dimms.first().and_then(|d| d.get("Speed")).and_then(|v| v.as_i64()).unwrap_or(0);
        t.ram_spec = format!("{count}x{per_gb} GB {mfg} DDR4-{spd}").trim().to_string();
    }
    let pnp = win_wmi::query(
        r"root\CIMV2",
        "SELECT Name, PNPClass, DeviceID FROM Win32_PnPEntity WHERE Status='OK'",
        &["Name", "PNPClass", "DeviceID"],
    );
    let mut ipf_sen_count = 0usize;
    let mut has_pmt = false;
    let mut has_its = false;
    let mut has_tpm = false;
    let mut has_cam = false;
    let mut has_ir_cam = false;
    let mut has_hello = false;
    let mut has_bt = false;
    let mut has_wwan = false;
    let mut has_gnss = false;
    let mut has_wifi = false;
    let mut has_eth = false;
    let mut has_reader = false;
    let mut has_audio = false;
    for row in &pnp {
        let name = row.get("Name").and_then(|v| v.as_str()).unwrap_or("");
        let cls = row.get("PNPClass").and_then(|v| v.as_str()).unwrap_or("");
        let did = row.get("DeviceID").and_then(|v| v.as_str()).unwrap_or("");
        if did.contains("INTC1046") {
            ipf_sen_count += 1;
        } else if did.contains("DEV_467D") || name.contains("Platform Monitoring Technology") {
            has_pmt = true;
        } else if did.contains("LEN0100") || name.contains("Intelligent Thermal Solution") {
            has_its = true;
        } else if cls == "SecurityDevices" || name.contains("Trusted Platform Module") {
            has_tpm = true;
        } else if name.contains("IR Camera") {
            has_ir_cam = true;
        } else if cls == "Camera" {
            has_cam = true;
        } else if cls == "Biometric" || name.contains("Windows Hello") {
            has_hello = true;
        } else if cls == "Bluetooth" {
            has_bt = true;
        } else if cls == "Sensor" && name.contains("GNSS") {
            t.has_wwan_gnss = true;
            has_gnss = true;
        } else if name.contains("Quectel") || name.contains("Mobile Broadband") {
            has_wwan = true;
        } else if cls == "Net" && (name.contains("Wi-Fi") || name.contains("Wireless")) {
            has_wifi = true;
        } else if cls == "Net" && name.contains("Ethernet") && !name.contains("Miniport") {
            has_eth = true;
        } else if cls == "MTD" || name.contains("card reader") {
            has_reader = true;
        } else if cls == "MEDIA" || name.contains("Realtek") {
            has_audio = true;
        }
    }
    if ipf_sen_count > 0 {
        t.hardware_badges.push(format!("{ipf_sen_count}x IPF EC"));
    }
    if has_pmt {
        t.hardware_badges.push("Intel PMT".to_string());
    }
    if has_its {
        t.hardware_badges.push("Lenovo ITS".to_string());
    }
    if has_tpm {
        t.hardware_badges.push("TPM 2.0".to_string());
    }
    if has_wifi {
        t.hardware_badges.push("Wi-Fi".to_string());
    }
    if has_eth {
        t.hardware_badges.push("GbE".to_string());
    }
    if has_bt {
        t.hardware_badges.push("BT".to_string());
    }
    if has_wwan || has_gnss {
        t.hardware_badges.push(if has_gnss { "LTE/GNSS".to_string() } else { "LTE".to_string() });
    }
    if has_ir_cam && has_cam {
        t.hardware_badges.push("FHD+IR Cam".to_string());
    } else if has_cam {
        t.hardware_badges.push("Camera".to_string());
    }
    if has_hello {
        t.hardware_badges.push("Hello".to_string());
    }
    if has_reader {
        t.hardware_badges.push("SD Reader".to_string());
    }
    if has_audio {
        t.hardware_badges.push("Audio".to_string());
    }

    // CallNtPowerInformation(ProcessorInformation = 11)
    let n_cpu = t.topology.logical_cpus;
    let mut ppi = vec![PROCESSOR_POWER_INFORMATION::default(); n_cpu];
    let rc = unsafe {
        CallNtPowerInformation(
            11,
            ptr::null_mut(),
            0,
            ppi.as_mut_ptr() as *mut c_void,
            (n_cpu * std::mem::size_of::<PROCESSOR_POWER_INFORMATION>()) as ULONG,
        )
    };
    if rc == 0 {
        t.ppi = ppi;
    }

    if let Some(mon) = pdh_mon.as_mut() {
        t.pdh = mon.collect();
    }
    // Always merge any elevated ACPI/WMI thermal zones not already surfaced by PDH
    for row in win_wmi::query(
        r"root\WMI",
        "SELECT InstanceName, CurrentTemperature FROM MSAcpi_ThermalZoneTemperature",
        &["InstanceName", "CurrentTemperature"],
    ) {
        if let Some(dk) = row.get("CurrentTemperature").and_then(|v| v.as_f64()) {
            let tc = dk / 10.0 - 273.15;
            if tc > -20.0 && tc < 150.0 {
                let iname = row.get("InstanceName").and_then(|v| v.as_str()).unwrap_or("ACPI").to_string();
                let dup = t.pdh.thermal_zones.iter().any(|z| {
                    let zn = z.name.trim_start_matches(r"\_TZ.").trim_end_matches("_0");
                    iname.contains(zn) || zn == iname
                });
                if !dup {
                    t.pdh.thermal_zones.push(ThermalZoneReading {
                        name: iname,
                        temp_c: tc,
                        passive_pct: None,
                        throttle_reasons: None,
                    });
                }
            }
        }
    }
    if let Some(bt) = win_wmi::query(r"root\WMI", "SELECT Temperature FROM BatteryTemperature", &["Temperature"])
        .into_iter()
        .next()
    {
        if let Some(dk) = bt.get("Temperature").and_then(|v| v.as_f64()) {
            let tc = dk / 10.0 - 273.15;
            if tc > 0.0 && tc < 100.0 {
                t.battery_temp_c = Some(tc);
            }
        }
    }
    for row in win_wmi::query(r"root\CIMV2", "SELECT Name, CurrentReading FROM Win32_TemperatureProbe", &["Name", "CurrentReading"]) {
        if let Some(raw) = row.get("CurrentReading").and_then(|v| v.as_f64()) {
            let tc = raw / 10.0;
            if tc > 0.0 && tc < 120.0 {
                let label = row.get("Name").and_then(|v| v.as_str()).unwrap_or("SMBIOS Probe").trim().to_string();
                t.wmi_probes.push((label, tc));
            }
        }
    }

    t.drives = probe_drives();
    t.battery = probe_battery();

    let mut ms = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as DWORD,
        dwMemoryLoad: 0,
        ullTotalPhys: 0,
        ullAvailPhys: 0,
        ullTotalPageFile: 0,
        ullAvailPageFile: 0,
        ullTotalVirtual: 0,
        ullAvailVirtual: 0,
        ullAvailExtendedVirtual: 0,
    };
    if unsafe { GlobalMemoryStatusEx(&mut ms) } != 0 {
        t.mem_total_gb = ms.ullTotalPhys as f64 / 1_073_741_824.0;
        t.mem_used_gb = (ms.ullTotalPhys.saturating_sub(ms.ullAvailPhys)) as f64 / 1_073_741_824.0;
    }
    let mut pi: PERFORMANCE_INFORMATION = unsafe { std::mem::zeroed() };
    pi.cb = std::mem::size_of::<PERFORMANCE_INFORMATION>() as DWORD;
    if unsafe { K32GetPerformanceInfo(&mut pi, pi.cb) } != 0 {
        let ps = pi.PageSize as f64;
        t.commit_used_gb = (pi.CommitTotal as f64 * ps) / 1_073_741_824.0;
        t.commit_limit_gb = (pi.CommitLimit as f64 * ps) / 1_073_741_824.0;
        t.proc_count = pi.ProcessCount;
        t.thread_count = pi.ThreadCount;
    }
    t.uptime_secs = unsafe { GetTickCount64() } / 1000;

    let (plan, boost, max_p) = active_power_plan();
    t.power_plan = plan;
    t.boost_mode = boost;
    t.max_cpu_pct = max_p;

    let root_c = to_wide(r"C:\");
    let mut free_avail = 0u64;
    let mut total_b = 0u64;
    let mut total_free = 0u64;
    if unsafe { GetDiskFreeSpaceExW(root_c.as_ptr(), &mut free_avail, &mut total_b, &mut total_free) } != 0 {
        t.c_drive_free_gb = free_avail as f64 / 1_073_741_824.0;
    }

    if is_l15 || t.sys_manufacturer.eq_ignore_ascii_case("LENOVO") {
        if let Some(odo) = win_wmi::query(
            r"root\Lenovo",
            "SELECT Thermal_events, Shock_events, CPU_Uptime, Battery_cycles, SSD_Read_Write_count FROM Lenovo_Odometer",
            &["Thermal_events", "Shock_events", "CPU_Uptime", "Battery_cycles", "SSD_Read_Write_count"],
        )
        .into_iter()
        .next()
        {
            t.lenovo_odometer = Some(LenovoOdometer {
                thermal_events: odo.get("Thermal_events").and_then(|v| v.as_i64()),
                shock_events: odo.get("Shock_events").and_then(|v| v.as_i64()),
                cpu_uptime_hrs: odo.get("CPU_Uptime").and_then(|v| v.as_i64()),
                battery_cycles: odo.get("Battery_cycles").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                ssd_rw_count: odo.get("SSD_Read_Write_count").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            });
        }
    }

    read_hwinfo_registry(&mut t.external);
    read_lhm_wmi(&mut t.external);
    t.external.nvml = read_nvml();
    t.pawnio = probe_pawnio(&t.topology, &t.cpu_name, is_l15 && allow_ec);
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvme_smart_log_parser_decodes_real_offsets() {
        let mut buf = [0u8; 512];
        buf[0] = 0; // critical_warning
        buf[1..3].copy_from_slice(&313u16.to_le_bytes()); // 39.85 C
        buf[3] = 100; // avail spare
        buf[4] = 5;   // threshold
        buf[5] = 2;   // 2% used -> 98% life
        buf[48..56].copy_from_slice(&36_613_380u64.to_le_bytes()); // data units written
        buf[112..120].copy_from_slice(&499u64.to_le_bytes()); // cycles
        buf[128..136].copy_from_slice(&4057u64.to_le_bytes()); // power-on hours
        buf[200..202].copy_from_slice(&313u16.to_le_bytes()); // sensor 1

        let mut drv = DriveReading::default();
        parse_nvme_smart_log(&buf, &mut drv);
        assert_eq!(drv.life_pct, Some(98));
        assert_eq!(drv.avail_spare_pct, Some(100));
        assert_eq!(drv.power_on_hours, Some(4057));
        assert!((drv.temp_c.unwrap() - 39.85).abs() < 0.01);
        assert!((drv.written_gb.unwrap() - 18746.05).abs() < 1.0);
    }
}
