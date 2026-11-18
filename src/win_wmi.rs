//! Zero-dependency COM WMI reader (`IWbemLocator` / `IWbemServices` / `IWbemClassObject`).
//! Sub-5ms WMI queries with zero PowerShell subprocesses.

#![cfg(target_os = "windows")]
#![allow(non_snake_case, non_camel_case_types, dead_code)]

use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr;

use crate::win_ffi::{to_wide, BSTR, GUID, HRESULT, LPCWSTR};

const COINIT_MULTITHREADED: u32 = 0x0;
const CLSCTX_INPROC_SERVER: u32 = 0x1;
const RPC_C_AUTHN_LEVEL_DEFAULT: u32 = 0;
const RPC_C_IMP_LEVEL_IMPERSONATE: u32 = 3;
const EOAC_NONE: u32 = 0;
const RPC_E_CHANGED_MODE: HRESULT = 0x8001_0106u32 as i32;
const WBEM_FLAG_FORWARD_ONLY: i32 = 0x20;
const WBEM_FLAG_RETURN_IMMEDIATELY: i32 = 0x10;
const WBEM_INFINITE: i32 = -1;

const CLSID_WBEM_LOCATOR: GUID = GUID {
    Data1: 0x4590_f811,
    Data2: 0x1d3a,
    Data3: 0x11d0,
    Data4: [0x89, 0x1f, 0x00, 0xaa, 0x00, 0x4b, 0x2e, 0x24],
};

const IID_IWBEM_LOCATOR: GUID = GUID {
    Data1: 0xdc12_a687,
    Data2: 0x737f,
    Data3: 0x11cf,
    Data4: [0x88, 0x4d, 0x00, 0xaa, 0x00, 0x4b, 0x2e, 0x24],
};

#[repr(C)]
struct VARIANT {
    vt: u16,
    wReserved1: u16,
    wReserved2: u16,
    wReserved3: u16,
    val: [u64; 2],
}

impl Default for VARIANT {
    fn default() -> Self {
        Self {
            vt: 0,
            wReserved1: 0,
            wReserved2: 0,
            wReserved3: 0,
            val: [0, 0],
        }
    }
}

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(pvReserved: *mut c_void, dwCoInit: u32) -> HRESULT;
    fn CoInitializeSecurity(
        pSecDesc: *mut c_void,
        cAuthSvc: i32,
        asAuthSvc: *mut c_void,
        pReserved1: *mut c_void,
        dwAuthnLevel: u32,
        dwImpLevel: u32,
        pAuthList: *mut c_void,
        dwCapabilities: u32,
        pReserved3: *mut c_void,
    ) -> HRESULT;
    fn CoCreateInstance(
        rclsid: *const GUID,
        pUnkOuter: *mut c_void,
        dwClsContext: u32,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT;
    fn CoSetProxyBlanket(
        pProxy: *mut c_void,
        dwAuthnSvc: u32,
        dwAuthzSvc: u32,
        pServerPrincName: *mut u16,
        dwAuthnLevel: u32,
        dwImpLevel: u32,
        pAuthInfo: *mut c_void,
        dwCapabilities: u32,
    ) -> HRESULT;
}

#[link(name = "oleaut32")]
extern "system" {
    fn SysAllocString(psz: LPCWSTR) -> BSTR;
    fn SysFreeString(bstrString: BSTR);
    fn VariantClear(pvarg: *mut VARIANT) -> HRESULT;
}

#[derive(Clone, Debug)]
pub enum WmiVal {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl WmiVal {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            WmiVal::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            WmiVal::Int(v) => Some(*v),
            WmiVal::Float(f) => Some(*f as i64),
            WmiVal::Str(s) => s.trim().parse::<i64>().ok(),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            WmiVal::Float(f) => Some(*f),
            WmiVal::Int(v) => Some(*v as f64),
            WmiVal::Str(s) => s.trim().parse::<f64>().ok(),
            _ => None,
        }
    }
}

unsafe fn vtbl(obj: *mut c_void) -> *const *const c_void {
    *(obj as *const *const *const c_void)
}

unsafe fn com_release(obj: *mut c_void) {
    if !obj.is_null() {
        let f: extern "system" fn(*mut c_void) -> u32 = std::mem::transmute(*vtbl(obj).add(2));
        f(obj);
    }
}

struct BstrGuard(BSTR);
impl BstrGuard {
    fn new(s: &str) -> Self {
        let w = to_wide(s);
        Self(unsafe { SysAllocString(w.as_ptr()) })
    }
}
impl Drop for BstrGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { SysFreeString(self.0) };
        }
    }
}

/// Initialize COM for this thread (safe to call repeatedly).
pub fn init_com() {
    unsafe {
        let hr = CoInitializeEx(ptr::null_mut(), COINIT_MULTITHREADED);
        if hr >= 0 || hr == RPC_E_CHANGED_MODE {
            let _ = CoInitializeSecurity(
                ptr::null_mut(),
                -1,
                ptr::null_mut(),
                ptr::null_mut(),
                RPC_C_AUTHN_LEVEL_DEFAULT,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                ptr::null_mut(),
                EOAC_NONE,
                ptr::null_mut(),
            );
        }
    }
}

unsafe fn decode_variant(v: &VARIANT) -> Option<WmiVal> {
    match v.vt {
        8 => {
            // VT_BSTR
            let p = v.val[0] as *const u16;
            if p.is_null() {
                None
            } else {
                Some(WmiVal::Str(crate::win_ffi::from_wide_ptr(p)))
            }
        }
        2 => Some(WmiVal::Int((v.val[0] as i16) as i64)),       // VT_I2
        3 => Some(WmiVal::Int((v.val[0] as i32) as i64)),       // VT_I4
        17 => Some(WmiVal::Int((v.val[0] as u8) as i64)),       // VT_UI1
        18 => Some(WmiVal::Int((v.val[0] as u16) as i64)),      // VT_UI2
        19 => Some(WmiVal::Int((v.val[0] as u32) as i64)),      // VT_UI4
        20 => Some(WmiVal::Int(v.val[0] as i64)),               // VT_I8
        21 => Some(WmiVal::Int(v.val[0] as i64)),               // VT_UI8
        4 => Some(WmiVal::Float(f32::from_bits(v.val[0] as u32) as f64)), // VT_R4
        5 => Some(WmiVal::Float(f64::from_bits(v.val[0]))),     // VT_R8
        11 => Some(WmiVal::Bool((v.val[0] as i16) != 0)),       // VT_BOOL
        _ => None,
    }
}

/// Run a WQL query against `namespace` (e.g. `r"root\CIMV2"`) and extract the requested `props`.
pub fn query(namespace: &str, wql: &str, props: &[&str]) -> Vec<HashMap<String, WmiVal>> {
    init_com();
    let mut rows = Vec::new();
    unsafe {
        let mut locator: *mut c_void = ptr::null_mut();
        let hr = CoCreateInstance(
            &CLSID_WBEM_LOCATOR,
            ptr::null_mut(),
            CLSCTX_INPROC_SERVER,
            &IID_IWBEM_LOCATOR,
            &mut locator,
        );
        if hr < 0 || locator.is_null() {
            return rows;
        }

        let ns = BstrGuard::new(namespace);
        let connect_server: extern "system" fn(
            *mut c_void,
            BSTR,
            BSTR,
            BSTR,
            BSTR,
            i32,
            BSTR,
            *mut c_void,
            *mut *mut c_void,
        ) -> HRESULT = std::mem::transmute(*vtbl(locator).add(3));

        let mut services: *mut c_void = ptr::null_mut();
        let hr = connect_server(
            locator,
            ns.0,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut services,
        );
        if hr < 0 || services.is_null() {
            com_release(locator);
            return rows;
        }

        // Security blanket for WMI calls
        let _ = CoSetProxyBlanket(
            services,
            10, // RPC_C_AUTHN_WINNT
            0,  // RPC_C_AUTHZ_NONE
            ptr::null_mut(),
            3, // RPC_C_AUTHN_LEVEL_CALL
            RPC_C_IMP_LEVEL_IMPERSONATE,
            ptr::null_mut(),
            EOAC_NONE,
        );

        let lang = BstrGuard::new("WQL");
        let qstr = BstrGuard::new(wql);
        let exec_query: extern "system" fn(
            *mut c_void,
            BSTR,
            BSTR,
            i32,
            *mut c_void,
            *mut *mut c_void,
        ) -> HRESULT = std::mem::transmute(*vtbl(services).add(20));

        let mut enum_obj: *mut c_void = ptr::null_mut();
        let hr = exec_query(
            services,
            lang.0,
            qstr.0,
            WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
            ptr::null_mut(),
            &mut enum_obj,
        );
        if hr < 0 || enum_obj.is_null() {
            com_release(services);
            com_release(locator);
            return rows;
        }

        let next_fn: extern "system" fn(*mut c_void, i32, u32, *mut *mut c_void, *mut u32) -> HRESULT =
            std::mem::transmute(*vtbl(enum_obj).add(4));

        loop {
            let mut obj: *mut c_void = ptr::null_mut();
            let mut returned: u32 = 0;
            let hr = next_fn(enum_obj, WBEM_INFINITE, 1, &mut obj, &mut returned);
            if hr < 0 || returned == 0 || obj.is_null() {
                break;
            }
            let get_fn: extern "system" fn(
                *mut c_void,
                LPCWSTR,
                i32,
                *mut VARIANT,
                *mut i32,
                *mut i32,
            ) -> HRESULT = std::mem::transmute(*vtbl(obj).add(4));

            let mut row = HashMap::new();
            for prop in props {
                let wp = to_wide(prop);
                let mut var = VARIANT::default();
                let hr_g = get_fn(obj, wp.as_ptr(), 0, &mut var, ptr::null_mut(), ptr::null_mut());
                if hr_g >= 0 {
                    if let Some(val) = decode_variant(&var) {
                        row.insert((*prop).to_string(), val);
                    }
                }
                let _ = VariantClear(&mut var);
            }
            rows.push(row);
            com_release(obj);
        }

        com_release(enum_obj);
        com_release(services);
        com_release(locator);
    }
    rows
}
