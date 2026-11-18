//! Minimal Win32 FFI bindings (zero external crates, zero TPFanControl code).

#![cfg(target_os = "windows")]
#![allow(non_snake_case, non_camel_case_types, dead_code)]

use std::ffi::c_void;

pub type BOOL = i32;
pub type BYTE = u8;
pub type WORD = u16;
pub type DWORD = u32;
pub type ULONG = u32;
pub type LONG = i32;
pub type HRESULT = i32;
pub type NTSTATUS = i32;
pub type HANDLE = *mut c_void;
pub type HDEVINFO = *mut c_void;
pub type HKEY = *mut c_void;
pub type HMODULE = *mut c_void;
pub type PDH_HQUERY = *mut c_void;
pub type PDH_HCOUNTER = *mut c_void;
pub type LPVOID = *mut c_void;
pub type LPCVOID = *const c_void;
pub type LPCWSTR = *const u16;
pub type LPWSTR = *mut u16;
pub type LPCSTR = *const i8;
pub type BSTR = *mut u16;

pub const INVALID_HANDLE_VALUE: HANDLE = (-1isize) as HANDLE;
pub const STD_INPUT_HANDLE: DWORD = (-10i32) as DWORD;
pub const STD_OUTPUT_HANDLE: DWORD = (-11i32) as DWORD;
pub const ENABLE_VIRTUAL_TERMINAL_PROCESSING: DWORD = 0x0004;
pub const CP_UTF8: DWORD = 65001;

pub const GENERIC_READ: DWORD = 0x8000_0000;
pub const GENERIC_WRITE: DWORD = 0x4000_0000;
pub const FILE_SHARE_READ: DWORD = 0x0000_0001;
pub const FILE_SHARE_WRITE: DWORD = 0x0000_0002;
pub const OPEN_EXISTING: DWORD = 3;

pub const IOCTL_STORAGE_QUERY_PROPERTY: DWORD = 0x002D_1400;
pub const IOCTL_BATTERY_QUERY_TAG: DWORD = 0x0029_4040;
pub const IOCTL_BATTERY_QUERY_INFORMATION: DWORD = 0x0029_4044;
pub const IOCTL_BATTERY_QUERY_STATUS: DWORD = 0x0029_404C;

pub const DIGCF_PRESENT: DWORD = 0x0000_0002;
pub const DIGCF_DEVICEINTERFACE: DWORD = 0x0000_0010;

pub const HKEY_CURRENT_USER: HKEY = 0x8000_0001usize as HKEY;
pub const HKEY_LOCAL_MACHINE: HKEY = 0x8000_0002usize as HKEY;
pub const KEY_READ: DWORD = 0x0002_0019;

pub const PDH_FMT_DOUBLE: DWORD = 0x0000_0200;
pub const PDH_MORE_DATA: u32 = 0x8000_07D2;

pub const WAIT_OBJECT_0: DWORD = 0;
pub const WINDOW_BUFFER_SIZE_EVENT: WORD = 0x0004;
pub const KEY_EVENT: WORD = 0x0001;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct GUID {
    pub Data1: u32,
    pub Data2: u16,
    pub Data3: u16,
    pub Data4: [u8; 8],
}

pub const GUID_DEVINTERFACE_BATTERY: GUID = GUID {
    Data1: 0x72631e54,
    Data2: 0x78a4,
    Data3: 0x11d0,
    Data4: [0xbc, 0xf7, 0x00, 0xaa, 0x00, 0xb7, 0xb3, 0x2a],
};

pub const GUID_PROCESSOR_SETTINGS_SUBGROUP: GUID = GUID {
    Data1: 0x54533251,
    Data2: 0x82be,
    Data3: 0x4824,
    Data4: [0x96, 0xc1, 0x47, 0xb6, 0x0b, 0x74, 0x0d, 0x00],
};

pub const GUID_PROCESSOR_PERF_BOOST_MODE: GUID = GUID {
    Data1: 0xbe337238,
    Data2: 0x0d82,
    Data3: 0x4146,
    Data4: [0xa9, 0x60, 0x4f, 0x37, 0x49, 0xd4, 0x70, 0xc7],
};

pub const GUID_PROCESSOR_THROTTLE_MAXIMUM: GUID = GUID {
    Data1: 0xbc5038f7,
    Data2: 0x23e0,
    Data3: 0x4960,
    Data4: [0x96, 0xda, 0x33, 0xab, 0xaf, 0x59, 0x35, 0xec],
};

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct FILETIME {
    pub dwLowDateTime: DWORD,
    pub dwHighDateTime: DWORD,
}

impl FILETIME {
    pub fn as_u64(self) -> u64 {
        ((self.dwHighDateTime as u64) << 32) | (self.dwLowDateTime as u64)
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct COORD {
    pub X: i16,
    pub Y: i16,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct SMALL_RECT {
    pub Left: i16,
    pub Top: i16,
    pub Right: i16,
    pub Bottom: i16,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct CONSOLE_SCREEN_BUFFER_INFO {
    pub dwSize: COORD,
    pub dwCursorPosition: COORD,
    pub wAttributes: WORD,
    pub srWindow: SMALL_RECT,
    pub dwMaximumWindowSize: COORD,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct KEY_EVENT_RECORD {
    pub bKeyDown: BOOL,
    pub wRepeatCount: WORD,
    pub wVirtualKeyCode: WORD,
    pub wVirtualScanCode: WORD,
    pub uChar: u16,
    pub dwControlKeyState: DWORD,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union INPUT_RECORD_EVENT {
    pub KeyEvent: KEY_EVENT_RECORD,
    pub WindowBufferSizeEvent: COORD,
    pub _pad: [u32; 4],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct INPUT_RECORD {
    pub EventType: WORD,
    pub Event: INPUT_RECORD_EVENT,
}

#[repr(C)]
pub struct MEMORYSTATUSEX {
    pub dwLength: DWORD,
    pub dwMemoryLoad: DWORD,
    pub ullTotalPhys: u64,
    pub ullAvailPhys: u64,
    pub ullTotalPageFile: u64,
    pub ullAvailPageFile: u64,
    pub ullTotalVirtual: u64,
    pub ullAvailVirtual: u64,
    pub ullAvailExtendedVirtual: u64,
}

#[repr(C)]
pub struct PERFORMANCE_INFORMATION {
    pub cb: DWORD,
    pub CommitTotal: usize,
    pub CommitLimit: usize,
    pub CommitPeak: usize,
    pub PhysicalTotal: usize,
    pub PhysicalAvailable: usize,
    pub SystemCache: usize,
    pub KernelTotal: usize,
    pub KernelPaged: usize,
    pub KernelNonpaged: usize,
    pub PageSize: usize,
    pub HandleCount: DWORD,
    pub ProcessCount: DWORD,
    pub ThreadCount: DWORD,
}

#[repr(C)]
#[derive(Default)]
pub struct SYSTEM_POWER_STATUS {
    pub ACLineStatus: BYTE,
    pub BatteryFlag: BYTE,
    pub BatteryLifePercent: BYTE,
    pub SystemStatusFlag: BYTE,
    pub BatteryLifeTime: DWORD,
    pub BatteryFullLifeTime: DWORD,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct PROCESSOR_POWER_INFORMATION {
    pub Number: ULONG,
    pub MaxMhz: ULONG,
    pub CurrentMhz: ULONG,
    pub MhzLimit: ULONG,
    pub MaxIdleState: ULONG,
    pub CurrentIdleState: ULONG,
}

#[repr(C)]
pub struct SP_DEVICE_INTERFACE_DATA {
    pub cbSize: DWORD,
    pub InterfaceClassGuid: GUID,
    pub Flags: DWORD,
    pub Reserved: usize,
}

#[repr(C)]
pub struct STORAGE_PROPERTY_QUERY {
    pub PropertyId: DWORD,
    pub QueryType: DWORD,
    pub AdditionalParameters: [BYTE; 1],
}

#[repr(C)]
#[derive(Default)]
pub struct STORAGE_PROTOCOL_SPECIFIC_DATA {
    pub ProtocolType: DWORD,
    pub DataType: DWORD,
    pub ProtocolDataRequestValue: DWORD,
    pub ProtocolDataRequestSubValue: DWORD,
    pub ProtocolDataOffset: DWORD,
    pub ProtocolDataLength: DWORD,
    pub FixedProtocolReturnData: DWORD,
    pub ProtocolDataRequestSubValue2: DWORD,
    pub ProtocolDataRequestSubValue3: DWORD,
    pub ProtocolDataRequestSubValue4: DWORD,
}

#[repr(C)]
pub struct STORAGE_PROPERTY_QUERY_PROTOCOL {
    pub PropertyId: DWORD,
    pub QueryType: DWORD,
    pub ProtocolSpecific: STORAGE_PROTOCOL_SPECIFIC_DATA,
    pub Buffer: [BYTE; 512],
}

#[repr(C)]
pub struct BATTERY_QUERY_INFORMATION {
    pub BatteryTag: ULONG,
    pub InformationLevel: DWORD,
    pub AtRate: LONG,
}

#[repr(C)]
#[derive(Default)]
pub struct BATTERY_INFORMATION {
    pub Capabilities: ULONG,
    pub Technology: BYTE,
    pub Reserved: [BYTE; 3],
    pub Chemistry: [u8; 4],
    pub DesignedCapacity: ULONG,
    pub FullChargedCapacity: ULONG,
    pub DefaultAlert1: ULONG,
    pub DefaultAlert2: ULONG,
    pub CriticalBias: ULONG,
    pub CycleCount: ULONG,
}

#[repr(C)]
pub struct BATTERY_WAIT_STATUS {
    pub BatteryTag: ULONG,
    pub Timeout: ULONG,
    pub PowerState: ULONG,
    pub LowCapacity: ULONG,
    pub HighCapacity: ULONG,
}

#[repr(C)]
#[derive(Default)]
pub struct BATTERY_STATUS {
    pub PowerState: ULONG,
    pub Capacity: ULONG,
    pub Voltage: ULONG,
    pub Rate: LONG,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union PDH_FMT_COUNTERVALUE_U {
    pub longValue: LONG,
    pub doubleValue: f64,
    pub largeValue: i64,
    pub AnsiStringValue: *mut i8,
    pub WideStringValue: *mut u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PDH_FMT_COUNTERVALUE {
    pub CStatus: DWORD,
    pub value: PDH_FMT_COUNTERVALUE_U,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PDH_FMT_COUNTERVALUE_ITEM_W {
    pub szName: LPWSTR,
    pub FmtValue: PDH_FMT_COUNTERVALUE,
}

#[link(name = "kernel32")]
extern "system" {
    pub fn GetStdHandle(nStdHandle: DWORD) -> HANDLE;
    pub fn GetConsoleMode(hConsoleHandle: HANDLE, lpMode: *mut DWORD) -> BOOL;
    pub fn SetConsoleMode(hConsoleHandle: HANDLE, dwMode: DWORD) -> BOOL;
    pub fn SetConsoleOutputCP(wCodePageID: DWORD) -> BOOL;
    pub fn GetConsoleScreenBufferInfo(
        hConsoleOutput: HANDLE,
        lpConsoleScreenBufferInfo: *mut CONSOLE_SCREEN_BUFFER_INFO,
    ) -> BOOL;
    pub fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: DWORD) -> DWORD;
    pub fn PeekConsoleInputW(
        hConsoleInput: HANDLE,
        lpBuffer: *mut INPUT_RECORD,
        nLength: DWORD,
        lpNumberOfEventsRead: *mut DWORD,
    ) -> BOOL;
    pub fn ReadConsoleInputW(
        hConsoleInput: HANDLE,
        lpBuffer: *mut INPUT_RECORD,
        nLength: DWORD,
        lpNumberOfEventsRead: *mut DWORD,
    ) -> BOOL;
    pub fn CreateFileW(
        lpFileName: LPCWSTR,
        dwDesiredAccess: DWORD,
        dwShareMode: DWORD,
        lpSecurityAttributes: *mut c_void,
        dwCreationDisposition: DWORD,
        dwFlagsAndAttributes: DWORD,
        hTemplateFile: HANDLE,
    ) -> HANDLE;
    pub fn CloseHandle(hObject: HANDLE) -> BOOL;
    pub fn DeviceIoControl(
        hDevice: HANDLE,
        dwIoControlCode: DWORD,
        lpInBuffer: LPCVOID,
        nInBufferSize: DWORD,
        lpOutBuffer: LPVOID,
        nOutBufferSize: DWORD,
        lpBytesReturned: *mut DWORD,
        lpOverlapped: *mut c_void,
    ) -> BOOL;
    pub fn GetLastError() -> DWORD;
    pub fn GlobalMemoryStatusEx(lpBuffer: *mut MEMORYSTATUSEX) -> BOOL;
    pub fn K32GetPerformanceInfo(pPerformanceInformation: *mut PERFORMANCE_INFORMATION, cb: DWORD) -> BOOL;
    pub fn GetSystemTimes(
        lpIdleTime: *mut FILETIME,
        lpKernelTime: *mut FILETIME,
        lpUserTime: *mut FILETIME,
    ) -> BOOL;
    pub fn GetSystemPowerStatus(lpSystemPowerStatus: *mut SYSTEM_POWER_STATUS) -> BOOL;
    pub fn GetTickCount64() -> u64;
    pub fn GetActiveProcessorCount(GroupNumber: WORD) -> DWORD;
    pub fn GetDiskFreeSpaceExW(
        lpDirectoryName: LPCWSTR,
        lpFreeBytesAvailableToCaller: *mut u64,
        lpTotalNumberOfBytes: *mut u64,
        lpTotalNumberOfFreeBytes: *mut u64,
    ) -> BOOL;
    pub fn GetLogicalProcessorInformationEx(
        RelationshipType: DWORD,
        Buffer: *mut c_void,
        ReturnedLength: *mut DWORD,
    ) -> BOOL;
    pub fn GetCurrentThread() -> HANDLE;
    pub fn GetCurrentProcess() -> HANDLE;
    pub fn SetThreadAffinityMask(hThread: HANDLE, dwThreadAffinityMask: usize) -> usize;
    pub fn LoadLibraryW(lpLibFileName: LPCWSTR) -> HMODULE;
    pub fn GetProcAddress(hModule: HMODULE, lpProcName: LPCSTR) -> *const c_void;
    pub fn FreeLibrary(hLibModule: HMODULE) -> BOOL;
    pub fn CreateMutexW(
        lpMutexAttributes: *mut c_void,
        bInitialOwner: BOOL,
        lpName: LPCWSTR,
    ) -> HANDLE;
    pub fn ReleaseMutex(hMutex: HANDLE) -> BOOL;
    pub fn LocalFree(hMem: *mut c_void) -> *mut c_void;
    pub fn SetConsoleCtrlHandler(HandlerRoutine: Option<unsafe extern "system" fn(DWORD) -> BOOL>, Add: BOOL) -> BOOL;
}

#[link(name = "powrprof")]
extern "system" {
    pub fn CallNtPowerInformation(
        InformationLevel: DWORD,
        InputBuffer: *mut c_void,
        InputBufferLength: ULONG,
        OutputBuffer: *mut c_void,
        OutputBufferLength: ULONG,
    ) -> NTSTATUS;
    pub fn PowerGetActiveScheme(UserRootPowerKey: HKEY, ActivePolicyGuid: *mut *mut GUID) -> DWORD;
    pub fn PowerReadFriendlyName(
        RootPowerKey: HKEY,
        SchemeGuid: *const GUID,
        SubGroupOfPowerSettingsGuid: *const GUID,
        PowerSettingGuid: *const GUID,
        Buffer: *mut u8,
        BufferSize: *mut DWORD,
    ) -> DWORD;
    pub fn PowerReadACValueIndex(
        RootPowerKey: HKEY,
        SchemeGuid: *const GUID,
        SubGroupOfPowerSettingsGuid: *const GUID,
        PowerSettingGuid: *const GUID,
        AcValueIndex: *mut DWORD,
    ) -> DWORD;
    pub fn PowerReadDCValueIndex(
        RootPowerKey: HKEY,
        SchemeGuid: *const GUID,
        SubGroupOfPowerSettingsGuid: *const GUID,
        PowerSettingGuid: *const GUID,
        DcValueIndex: *mut DWORD,
    ) -> DWORD;
}

#[link(name = "setupapi")]
extern "system" {
    pub fn SetupDiGetClassDevsW(
        ClassGuid: *const GUID,
        Enumerator: LPCWSTR,
        hwndParent: HANDLE,
        Flags: DWORD,
    ) -> HDEVINFO;
    pub fn SetupDiEnumDeviceInterfaces(
        DeviceInfoSet: HDEVINFO,
        DeviceInfoData: *mut c_void,
        InterfaceClassGuid: *const GUID,
        MemberIndex: DWORD,
        DeviceInterfaceData: *mut SP_DEVICE_INTERFACE_DATA,
    ) -> BOOL;
    pub fn SetupDiGetDeviceInterfaceDetailW(
        DeviceInfoSet: HDEVINFO,
        DeviceInterfaceData: *mut SP_DEVICE_INTERFACE_DATA,
        DeviceInterfaceDetailData: *mut c_void,
        DeviceInterfaceDetailDataSize: DWORD,
        RequiredSize: *mut DWORD,
        DeviceInfoData: *mut c_void,
    ) -> BOOL;
    pub fn SetupDiDestroyDeviceInfoList(DeviceInfoSet: HDEVINFO) -> BOOL;
}

#[link(name = "pdh")]
extern "system" {
    pub fn PdhOpenQueryW(szDataSource: LPCWSTR, dwUserData: usize, phQuery: *mut PDH_HQUERY) -> DWORD;
    pub fn PdhAddEnglishCounterW(
        hQuery: PDH_HQUERY,
        szFullCounterPath: LPCWSTR,
        dwUserData: usize,
        phCounter: *mut PDH_HCOUNTER,
    ) -> DWORD;
    pub fn PdhCollectQueryData(hQuery: PDH_HQUERY) -> DWORD;
    pub fn PdhGetFormattedCounterValue(
        hCounter: PDH_HCOUNTER,
        dwFormat: DWORD,
        lpdwType: *mut DWORD,
        pValue: *mut PDH_FMT_COUNTERVALUE,
    ) -> DWORD;
    pub fn PdhGetFormattedCounterArrayW(
        hCounter: PDH_HCOUNTER,
        dwFormat: DWORD,
        lpdwBufferSize: *mut DWORD,
        lpdwItemCount: *mut DWORD,
        ItemBuffer: *mut PDH_FMT_COUNTERVALUE_ITEM_W,
    ) -> DWORD;
    pub fn PdhCloseQuery(hQuery: PDH_HQUERY) -> DWORD;
}

#[link(name = "advapi32")]
extern "system" {
    pub fn RegOpenKeyExW(
        hKey: HKEY,
        lpSubKey: LPCWSTR,
        ulOptions: DWORD,
        samDesired: DWORD,
        phkResult: *mut HKEY,
    ) -> DWORD;
    pub fn RegEnumValueW(
        hKey: HKEY,
        dwIndex: DWORD,
        lpValueName: LPWSTR,
        lpcchValueName: *mut DWORD,
        lpReserved: *mut DWORD,
        lpType: *mut DWORD,
        lpData: *mut BYTE,
        lpcbData: *mut DWORD,
    ) -> DWORD;
    pub fn RegCloseKey(hKey: HKEY) -> DWORD;
    pub fn OpenSCManagerW(lpMachineName: LPCWSTR, lpDatabaseName: LPCWSTR, dwDesiredAccess: DWORD) -> HANDLE;
    pub fn OpenServiceW(hSCManager: HANDLE, lpServiceName: LPCWSTR, dwDesiredAccess: DWORD) -> HANDLE;
    pub fn StartServiceW(hService: HANDLE, dwNumServiceArgs: DWORD, lpServiceArgVectors: *const LPCWSTR) -> BOOL;
    pub fn CloseServiceHandle(hSCObject: HANDLE) -> BOOL;
    pub fn OpenProcessToken(ProcessHandle: HANDLE, DesiredAccess: DWORD, TokenHandle: *mut HANDLE) -> BOOL;
    pub fn GetTokenInformation(
        TokenHandle: HANDLE,
        TokenInformationClass: DWORD,
        TokenInformation: LPVOID,
        TokenInformationLength: DWORD,
        ReturnLength: *mut DWORD,
    ) -> BOOL;
}

#[link(name = "shell32")]
extern "system" {
    pub fn ShellExecuteW(
        hwnd: HANDLE,
        lpOperation: LPCWSTR,
        lpFile: LPCWSTR,
        lpParameters: LPCWSTR,
        lpDirectory: LPCWSTR,
        nShowCmd: i32,
    ) -> HANDLE;
}

/// Convert Rust string to NUL-terminated UTF-16 Vec.
pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Read NUL-terminated UTF-16 pointer into Rust String.
pub unsafe fn from_wide_ptr(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
}
