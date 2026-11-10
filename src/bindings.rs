#[inline]
pub unsafe fn ClientToScreen(hwnd: HWND, lppoint: *mut POINT) -> windows_core::BOOL {
    windows_core::link!("user32.dll" "system" fn ClientToScreen(hwnd : HWND, lppoint : *mut POINT) -> windows_core::BOOL);
    unsafe { ClientToScreen(hwnd, lppoint as _) }
}
#[inline]
pub unsafe fn CreateRectRgn(x1: i32, y1: i32, x2: i32, y2: i32) -> HRGN {
    windows_core::link!("gdi32.dll" "system" fn CreateRectRgn(x1 : i32, y1 : i32, x2 : i32, y2 : i32) -> HRGN);
    unsafe { CreateRectRgn(x1, y1, x2, y2) }
}
#[inline]
pub unsafe fn CreateWindowExW<P1, P2>(
    dwexstyle: u32,
    lpclassname: P1,
    lpwindowname: P2,
    dwstyle: u32,
    x: i32,
    y: i32,
    nwidth: i32,
    nheight: i32,
    hwndparent: Option<HWND>,
    hmenu: Option<HMENU>,
    hinstance: Option<HINSTANCE>,
    lpparam: Option<*const core::ffi::c_void>,
) -> HWND
where
    P1: windows_core::Param<windows_core::PCWSTR>,
    P2: windows_core::Param<windows_core::PCWSTR>,
{
    windows_core::link!("user32.dll" "system" fn CreateWindowExW(dwexstyle : u32, lpclassname : windows_core::PCWSTR, lpwindowname : windows_core::PCWSTR, dwstyle : u32, x : i32, y : i32, nwidth : i32, nheight : i32, hwndparent : HWND, hmenu : HMENU, hinstance : HINSTANCE, lpparam : *const core::ffi::c_void) -> HWND);
    unsafe {
        CreateWindowExW(
            dwexstyle,
            lpclassname.param().abi(),
            lpwindowname.param().abi(),
            dwstyle,
            x,
            y,
            nwidth,
            nheight,
            hwndparent.unwrap_or(core::mem::zeroed()) as _,
            hmenu.unwrap_or(core::mem::zeroed()) as _,
            hinstance.unwrap_or(core::mem::zeroed()) as _,
            lpparam.unwrap_or(core::mem::zeroed()) as _,
        )
    }
}
#[inline]
pub unsafe fn DefWindowProcW(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    windows_core::link!("user32.dll" "system" fn DefWindowProcW(hwnd : HWND, msg : u32, wparam : WPARAM, lparam : LPARAM) -> LRESULT);
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
#[inline]
pub unsafe fn DragAcceptFiles(hwnd: HWND, faccept: bool) {
    windows_core::link!("shell32.dll" "system" fn DragAcceptFiles(hwnd : HWND, faccept : windows_core::BOOL));
    unsafe { DragAcceptFiles(hwnd, faccept.into()) }
}
#[inline]
pub unsafe fn DragFinish(hdrop: HDROP) {
    windows_core::link!("shell32.dll" "system" fn DragFinish(hdrop : HDROP));
    unsafe { DragFinish(hdrop) }
}
#[inline]
pub unsafe fn DragQueryFileW(
    hdrop: HDROP,
    ifile: u32,
    lpszfile: Option<windows_core::PWSTR>,
    cch: u32,
) -> u32 {
    windows_core::link!("shell32.dll" "system" fn DragQueryFileW(hdrop : HDROP, ifile : u32, lpszfile : windows_core::PWSTR, cch : u32) -> u32);
    unsafe {
        DragQueryFileW(
            hdrop,
            ifile,
            lpszfile.unwrap_or(core::mem::zeroed()) as _,
            cch,
        )
    }
}
#[inline]
pub unsafe fn EnumThreadWindows(
    dwthreadid: u32,
    lpfn: WNDENUMPROC,
    lparam: LPARAM,
) -> windows_core::BOOL {
    windows_core::link!("user32.dll" "system" fn EnumThreadWindows(dwthreadid : u32, lpfn : WNDENUMPROC, lparam : LPARAM) -> windows_core::BOOL);
    unsafe { EnumThreadWindows(dwthreadid, lpfn, lparam) }
}
#[inline]
pub unsafe fn FindWindowExW<P2, P3>(
    hwndparent: Option<HWND>,
    hwndchildafter: Option<HWND>,
    lpszclass: P2,
    lpszwindow: P3,
) -> HWND
where
    P2: windows_core::Param<windows_core::PCWSTR>,
    P3: windows_core::Param<windows_core::PCWSTR>,
{
    windows_core::link!("user32.dll" "system" fn FindWindowExW(hwndparent : HWND, hwndchildafter : HWND, lpszclass : windows_core::PCWSTR, lpszwindow : windows_core::PCWSTR) -> HWND);
    unsafe {
        FindWindowExW(
            hwndparent.unwrap_or(core::mem::zeroed()) as _,
            hwndchildafter.unwrap_or(core::mem::zeroed()) as _,
            lpszclass.param().abi(),
            lpszwindow.param().abi(),
        )
    }
}
#[inline]
pub unsafe fn GetAncestor(hwnd: HWND, gaflags: u32) -> HWND {
    windows_core::link!("user32.dll" "system" fn GetAncestor(hwnd : HWND, gaflags : u32) -> HWND);
    unsafe { GetAncestor(hwnd, gaflags) }
}
#[inline]
pub unsafe fn GetClassNameW(hwnd: HWND, lpclassname: windows_core::PWSTR, nmaxcount: i32) -> i32 {
    windows_core::link!("user32.dll" "system" fn GetClassNameW(hwnd : HWND, lpclassname : windows_core::PWSTR, nmaxcount : i32) -> i32);
    unsafe { GetClassNameW(hwnd, lpclassname, nmaxcount) }
}
#[inline]
pub unsafe fn GetClientRect(hwnd: HWND, lprect: *mut RECT) -> windows_core::BOOL {
    windows_core::link!("user32.dll" "system" fn GetClientRect(hwnd : HWND, lprect : *mut RECT) -> windows_core::BOOL);
    unsafe { GetClientRect(hwnd, lprect as _) }
}
#[inline]
pub unsafe fn GetCurrentThreadId() -> u32 {
    windows_core::link!("kernel32.dll" "system" fn GetCurrentThreadId() -> u32);
    unsafe { GetCurrentThreadId() }
}
#[inline]
pub unsafe fn GetCursorPos(lppoint: *mut POINT) -> windows_core::BOOL {
    windows_core::link!("user32.dll" "system" fn GetCursorPos(lppoint : *mut POINT) -> windows_core::BOOL);
    unsafe { GetCursorPos(lppoint as _) }
}
#[inline]
pub unsafe fn GetDpiForWindow(hwnd: HWND) -> u32 {
    windows_core::link!("user32.dll" "system" fn GetDpiForWindow(hwnd : HWND) -> u32);
    unsafe { GetDpiForWindow(hwnd) }
}
#[inline]
pub unsafe fn GetForegroundWindow() -> HWND {
    windows_core::link!("user32.dll" "system" fn GetForegroundWindow() -> HWND);
    unsafe { GetForegroundWindow() }
}
#[inline]
pub unsafe fn GetModuleHandleW<P0>(lpmodulename: P0) -> HMODULE
where
    P0: windows_core::Param<windows_core::PCWSTR>,
{
    windows_core::link!("kernel32.dll" "system" fn GetModuleHandleW(lpmodulename : windows_core::PCWSTR) -> HMODULE);
    unsafe { GetModuleHandleW(lpmodulename.param().abi()) }
}
#[inline]
pub unsafe fn GetMonitorInfoW(hmonitor: HMONITOR, lpmi: *mut MONITORINFO) -> windows_core::BOOL {
    windows_core::link!("user32.dll" "system" fn GetMonitorInfoW(hmonitor : HMONITOR, lpmi : *mut MONITORINFO) -> windows_core::BOOL);
    unsafe { GetMonitorInfoW(hmonitor, lpmi as _) }
}
#[inline]
pub unsafe fn GetOpenFileNameW(param0: *mut OPENFILENAMEW) -> windows_core::BOOL {
    windows_core::link!("comdlg32.dll" "system" fn GetOpenFileNameW(param0 : *mut OPENFILENAMEW) -> windows_core::BOOL);
    unsafe { GetOpenFileNameW(param0 as _) }
}
#[inline]
pub unsafe fn GetParent(hwnd: HWND) -> HWND {
    windows_core::link!("user32.dll" "system" fn GetParent(hwnd : HWND) -> HWND);
    unsafe { GetParent(hwnd) }
}
#[inline]
pub unsafe fn GetStockObject(i: i32) -> HGDIOBJ {
    windows_core::link!("gdi32.dll" "system" fn GetStockObject(i : i32) -> HGDIOBJ);
    unsafe { GetStockObject(i) }
}
#[inline]
pub unsafe fn GetSystemMetrics(nindex: i32) -> i32 {
    windows_core::link!("user32.dll" "system" fn GetSystemMetrics(nindex : i32) -> i32);
    unsafe { GetSystemMetrics(nindex) }
}
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "arm64ec",
    target_arch = "x86_64"
))]
#[inline]
pub unsafe fn GetWindowLongPtrW(hwnd: HWND, nindex: i32) -> isize {
    windows_core::link!("user32.dll" "system" fn GetWindowLongPtrW(hwnd : HWND, nindex : i32) -> isize);
    unsafe { GetWindowLongPtrW(hwnd, nindex) }
}
#[cfg(target_pointer_width = "32")]
pub use GetWindowLongW as GetWindowLongPtrW;
#[inline]
pub unsafe fn GetWindowLongW(hwnd: HWND, nindex: i32) -> i32 {
    windows_core::link!("user32.dll" "system" fn GetWindowLongW(hwnd : HWND, nindex : i32) -> i32);
    unsafe { GetWindowLongW(hwnd, nindex) }
}
#[inline]
pub unsafe fn GetWindowPlacement(hwnd: HWND, lpwndpl: *mut WINDOWPLACEMENT) -> windows_core::BOOL {
    windows_core::link!("user32.dll" "system" fn GetWindowPlacement(hwnd : HWND, lpwndpl : *mut WINDOWPLACEMENT) -> windows_core::BOOL);
    unsafe { GetWindowPlacement(hwnd, lpwndpl as _) }
}
#[inline]
pub unsafe fn GetWindowRect(hwnd: HWND, lprect: *mut RECT) -> windows_core::BOOL {
    windows_core::link!("user32.dll" "system" fn GetWindowRect(hwnd : HWND, lprect : *mut RECT) -> windows_core::BOOL);
    unsafe { GetWindowRect(hwnd, lprect as _) }
}
#[inline]
pub unsafe fn LoadCursorW<P1>(hinstance: Option<HINSTANCE>, lpcursorname: P1) -> HCURSOR
where
    P1: windows_core::Param<windows_core::PCWSTR>,
{
    windows_core::link!("user32.dll" "system" fn LoadCursorW(hinstance : HINSTANCE, lpcursorname : windows_core::PCWSTR) -> HCURSOR);
    unsafe {
        LoadCursorW(
            hinstance.unwrap_or(core::mem::zeroed()) as _,
            lpcursorname.param().abi(),
        )
    }
}
#[inline]
pub unsafe fn LoadImageW<P1>(
    hinst: Option<HINSTANCE>,
    name: P1,
    r#type: u32,
    cx: i32,
    cy: i32,
    fuload: u32,
) -> HANDLE
where
    P1: windows_core::Param<windows_core::PCWSTR>,
{
    windows_core::link!("user32.dll" "system" fn LoadImageW(hinst : HINSTANCE, name : windows_core::PCWSTR, r#type : u32, cx : i32, cy : i32, fuload : u32) -> HANDLE);
    unsafe {
        LoadImageW(
            hinst.unwrap_or(core::mem::zeroed()) as _,
            name.param().abi(),
            r#type,
            cx,
            cy,
            fuload,
        )
    }
}
#[inline]
pub unsafe fn MonitorFromWindow(hwnd: HWND, dwflags: u32) -> HMONITOR {
    windows_core::link!("user32.dll" "system" fn MonitorFromWindow(hwnd : HWND, dwflags : u32) -> HMONITOR);
    unsafe { MonitorFromWindow(hwnd, dwflags) }
}
#[inline]
pub unsafe fn RegisterClassW(lpwndclass: *const WNDCLASSW) -> ATOM {
    windows_core::link!("user32.dll" "system" fn RegisterClassW(lpwndclass : *const WNDCLASSW) -> ATOM);
    unsafe { RegisterClassW(lpwndclass) }
}
#[inline]
pub unsafe fn ReleaseCapture() -> windows_core::BOOL {
    windows_core::link!("user32.dll" "system" fn ReleaseCapture() -> windows_core::BOOL);
    unsafe { ReleaseCapture() }
}
#[inline]
pub unsafe fn SendMessageW(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    windows_core::link!("user32.dll" "system" fn SendMessageW(hwnd : HWND, msg : u32, wparam : WPARAM, lparam : LPARAM) -> LRESULT);
    unsafe { SendMessageW(hwnd, msg, wparam, lparam) }
}
#[inline]
pub unsafe fn SetFocus(hwnd: Option<HWND>) -> HWND {
    windows_core::link!("user32.dll" "system" fn SetFocus(hwnd : HWND) -> HWND);
    unsafe { SetFocus(hwnd.unwrap_or(core::mem::zeroed()) as _) }
}
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "arm64ec",
    target_arch = "x86_64"
))]
#[inline]
pub unsafe fn SetWindowLongPtrW(hwnd: HWND, nindex: i32, dwnewlong: isize) -> isize {
    windows_core::link!("user32.dll" "system" fn SetWindowLongPtrW(hwnd : HWND, nindex : i32, dwnewlong : isize) -> isize);
    unsafe { SetWindowLongPtrW(hwnd, nindex, dwnewlong) }
}
#[cfg(target_pointer_width = "32")]
pub use SetWindowLongW as SetWindowLongPtrW;
#[inline]
pub unsafe fn SetWindowLongW(hwnd: HWND, nindex: i32, dwnewlong: i32) -> i32 {
    windows_core::link!("user32.dll" "system" fn SetWindowLongW(hwnd : HWND, nindex : i32, dwnewlong : i32) -> i32);
    unsafe { SetWindowLongW(hwnd, nindex, dwnewlong) }
}
#[inline]
pub unsafe fn SetWindowPlacement(
    hwnd: HWND,
    lpwndpl: *const WINDOWPLACEMENT,
) -> windows_core::BOOL {
    windows_core::link!("user32.dll" "system" fn SetWindowPlacement(hwnd : HWND, lpwndpl : *const WINDOWPLACEMENT) -> windows_core::BOOL);
    unsafe { SetWindowPlacement(hwnd, lpwndpl) }
}
#[inline]
pub unsafe fn SetWindowPos(
    hwnd: HWND,
    hwndinsertafter: Option<HWND>,
    x: i32,
    y: i32,
    cx: i32,
    cy: i32,
    uflags: u32,
) -> windows_core::BOOL {
    windows_core::link!("user32.dll" "system" fn SetWindowPos(hwnd : HWND, hwndinsertafter : HWND, x : i32, y : i32, cx : i32, cy : i32, uflags : u32) -> windows_core::BOOL);
    unsafe {
        SetWindowPos(
            hwnd,
            hwndinsertafter.unwrap_or(core::mem::zeroed()) as _,
            x,
            y,
            cx,
            cy,
            uflags,
        )
    }
}
#[inline]
pub unsafe fn SetWindowRgn(hwnd: HWND, hrgn: Option<HRGN>, bredraw: bool) -> i32 {
    windows_core::link!("user32.dll" "system" fn SetWindowRgn(hwnd : HWND, hrgn : HRGN, bredraw : windows_core::BOOL) -> i32);
    unsafe {
        SetWindowRgn(
            hwnd,
            hrgn.unwrap_or(core::mem::zeroed()) as _,
            bredraw.into(),
        )
    }
}
#[inline]
pub unsafe fn WindowFromPoint(point: POINT) -> HWND {
    windows_core::link!("user32.dll" "system" fn WindowFromPoint(point : POINT) -> HWND);
    unsafe { WindowFromPoint(point) }
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ATOM(pub u16);
pub const BLACK_BRUSH: i32 = 4;
pub const CS_DBLCLKS: i32 = 8;
pub const GA_ROOTOWNER: i32 = 3;
pub const GWLP_USERDATA: i32 = -21;
pub const GWL_STYLE: i32 = -16;
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HANDLE(pub *mut core::ffi::c_void);
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HBRUSH(pub *mut core::ffi::c_void);
pub type HCURSOR = HICON;
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HDROP(pub *mut core::ffi::c_void);
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HGDIOBJ(pub *mut core::ffi::c_void);
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HICON(pub *mut core::ffi::c_void);
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HINSTANCE(pub *mut core::ffi::c_void);
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HMENU(pub *mut core::ffi::c_void);
pub type HMODULE = HINSTANCE;
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HMONITOR(pub *mut core::ffi::c_void);
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HRGN(pub *mut core::ffi::c_void);
pub const HTCAPTION: i32 = 2;
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HWND(pub *mut core::ffi::c_void);
pub const HWND_NOTOPMOST: HWND = HWND(-2 as _);
pub const HWND_TOP: HWND = HWND(0 as _);
pub const HWND_TOPMOST: HWND = HWND(-1 as _);
pub const ICON_BIG: i32 = 1;
pub const ICON_SMALL: i32 = 0;
pub const IDC_ARROW: windows_core::PCWSTR = windows_core::PCWSTR(32512 as _);
pub const IMAGE_ICON: i32 = 1;
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct LPARAM(pub isize);
pub type LPOFNHOOKPROC = Option<
    unsafe extern "system" fn(param0: HWND, param1: u32, param2: WPARAM, param3: LPARAM) -> usize,
>;
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct LRESULT(pub isize);
pub const LR_DEFAULTCOLOR: i32 = 0;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MONITORINFO {
    pub cbSize: u32,
    pub rcMonitor: RECT,
    pub rcWork: RECT,
    pub dwFlags: u32,
}
pub const MONITOR_DEFAULTTONEAREST: i32 = 2;
pub const OFN_FILEMUSTEXIST: i32 = 4096;
pub const OFN_PATHMUSTEXIST: i32 = 2048;
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
#[derive(Clone, Copy, Default)]
pub struct OPENFILENAMEW {
    pub lStructSize: u32,
    pub hwndOwner: HWND,
    pub hInstance: HINSTANCE,
    pub lpstrFilter: windows_core::PCWSTR,
    pub lpstrCustomFilter: windows_core::PWSTR,
    pub nMaxCustFilter: u32,
    pub nFilterIndex: u32,
    pub lpstrFile: windows_core::PWSTR,
    pub nMaxFile: u32,
    pub lpstrFileTitle: windows_core::PWSTR,
    pub nMaxFileTitle: u32,
    pub lpstrInitialDir: windows_core::PCWSTR,
    pub lpstrTitle: windows_core::PCWSTR,
    pub Flags: u32,
    pub nFileOffset: u16,
    pub nFileExtension: u16,
    pub lpstrDefExt: windows_core::PCWSTR,
    pub lCustData: LPARAM,
    pub lpfnHook: LPOFNHOOKPROC,
    pub lpTemplateName: windows_core::PCWSTR,
    pub pvReserved: *mut core::ffi::c_void,
    pub dwReserved: u32,
    pub FlagsEx: u32,
}
#[repr(C)]
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "arm64ec",
    target_arch = "x86_64"
))]
#[derive(Clone, Copy, Debug, Default)]
pub struct OPENFILENAMEW {
    pub lStructSize: u32,
    pub hwndOwner: HWND,
    pub hInstance: HINSTANCE,
    pub lpstrFilter: windows_core::PCWSTR,
    pub lpstrCustomFilter: windows_core::PWSTR,
    pub nMaxCustFilter: u32,
    pub nFilterIndex: u32,
    pub lpstrFile: windows_core::PWSTR,
    pub nMaxFile: u32,
    pub lpstrFileTitle: windows_core::PWSTR,
    pub nMaxFileTitle: u32,
    pub lpstrInitialDir: windows_core::PCWSTR,
    pub lpstrTitle: windows_core::PCWSTR,
    pub Flags: u32,
    pub nFileOffset: u16,
    pub nFileExtension: u16,
    pub lpstrDefExt: windows_core::PCWSTR,
    pub lCustData: LPARAM,
    pub lpfnHook: LPOFNHOOKPROC,
    pub lpTemplateName: windows_core::PCWSTR,
    pub pvReserved: *mut core::ffi::c_void,
    pub dwReserved: u32,
    pub FlagsEx: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
pub const SM_CXDRAG: i32 = 68;
pub const SM_CXICON: i32 = 11;
pub const SM_CXSMICON: i32 = 49;
pub const SM_CYDRAG: i32 = 69;
pub const SWP_FRAMECHANGED: i32 = 32;
pub const SWP_NOACTIVATE: i32 = 16;
pub const SWP_NOOWNERZORDER: i32 = 512;
pub const VK_DOWN: i32 = 40;
pub const VK_ESCAPE: i32 = 27;
pub const VK_LEFT: i32 = 37;
pub const VK_MEDIA_NEXT_TRACK: i32 = 176;
pub const VK_MEDIA_PREV_TRACK: i32 = 177;
pub const VK_NEXT: i32 = 34;
pub const VK_PRIOR: i32 = 33;
pub const VK_RIGHT: i32 = 39;
pub const VK_SPACE: i32 = 32;
pub const VK_UP: i32 = 38;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WINDOWPLACEMENT {
    pub length: u32,
    pub flags: u32,
    pub showCmd: u32,
    pub ptMinPosition: POINT,
    pub ptMaxPosition: POINT,
    pub rcNormalPosition: RECT,
}
pub const WM_DROPFILES: i32 = 563;
pub const WM_KEYDOWN: i32 = 256;
pub const WM_LBUTTONDBLCLK: i32 = 515;
pub const WM_LBUTTONDOWN: i32 = 513;
pub const WM_LBUTTONUP: i32 = 514;
pub const WM_MOUSEMOVE: i32 = 512;
pub const WM_NCDESTROY: i32 = 130;
pub const WM_NCLBUTTONDOWN: i32 = 161;
pub const WM_SETICON: i32 = 128;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct WNDCLASSW {
    pub style: u32,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: windows_core::PCWSTR,
    pub lpszClassName: windows_core::PCWSTR,
}
pub type WNDENUMPROC =
    Option<unsafe extern "system" fn(param0: HWND, param1: LPARAM) -> windows_core::BOOL>;
pub type WNDPROC = Option<
    unsafe extern "system" fn(param0: HWND, param1: u32, param2: WPARAM, param3: LPARAM) -> LRESULT,
>;
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct WPARAM(pub usize);
pub const WS_CAPTION: i32 = 12582912;
pub const WS_CHILD: i32 = 1073741824;
pub const WS_CLIPSIBLINGS: i32 = 67108864;
pub const WS_MAXIMIZE: i32 = 16777216;
pub const WS_OVERLAPPEDWINDOW: i32 = 13565952;
pub const WS_VISIBLE: i32 = 268435456;
