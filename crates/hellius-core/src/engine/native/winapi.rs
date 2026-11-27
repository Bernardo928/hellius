//! Conhecimento sobre APIs comuns (Win32 e libc) usado pelo decompilador:
//! quantos argumentos cada função recebe, os tipos, nomes de constantes
//! (`MB_OK`, `WM_CREATE`...) e o layout de structs passadas por ponteiro.
//!
//! Só serve para deixar o pseudocódigo legível. Nada aqui muda a análise.

/// Assinatura: `(nome base, tipo de retorno [nome sugerido], parâmetros)`.
/// Parâmetros separados por `|`, cada um `TIPO[:GRUPO] nome`. `...` = variádico.
/// Nomes Win32 ficam sem o sufixo A/W (vale para os dois).
static APIS: &[(&str, &str, &str)] = &[
    // ---- user32
    ("MessageBox", "int", "HWND hWnd|LPCSTR lpText|LPCSTR lpCaption|UINT:MB uType"),
    ("MessageBoxEx", "int", "HWND hWnd|LPCSTR lpText|LPCSTR lpCaption|UINT:MB uType|WORD wLanguageId"),
    ("CreateWindowEx", "HWND hWnd", "DWORD:WSEX dwExStyle|LPCSTR lpClassName|LPCSTR lpWindowName|DWORD:WS dwStyle|int:CW X|int:CW Y|int:CW nWidth|int:CW nHeight|HWND hWndParent|HMENU hMenu|HINSTANCE hInstance|LPVOID lpParam"),
    ("DefWindowProc", "LRESULT", "HWND hWnd|UINT:WM Msg|WPARAM wParam|LPARAM lParam"),
    ("CallWindowProc", "LRESULT", "WNDPROC lpPrevWndFunc|HWND hWnd|UINT:WM Msg|WPARAM wParam|LPARAM lParam"),
    ("DefDlgProc", "LRESULT", "HWND hDlg|UINT:WM Msg|WPARAM wParam|LPARAM lParam"),
    ("RegisterClass", "ATOM atom", "WNDCLASS* lpWndClass"),
    ("RegisterClassEx", "ATOM atom", "WNDCLASSEX* lpWndClass"),
    ("UnregisterClass", "BOOL", "LPCSTR lpClassName|HINSTANCE hInstance"),
    ("GetMessage", "BOOL", "MSG* lpMsg|HWND hWnd|UINT wMsgFilterMin|UINT wMsgFilterMax"),
    ("PeekMessage", "BOOL", "MSG* lpMsg|HWND hWnd|UINT wMsgFilterMin|UINT wMsgFilterMax|UINT:PM wRemoveMsg"),
    ("TranslateMessage", "BOOL", "MSG* lpMsg"),
    ("DispatchMessage", "LRESULT", "MSG* lpMsg"),
    ("IsDialogMessage", "BOOL", "HWND hDlg|MSG* lpMsg"),
    ("PostQuitMessage", "void", "int nExitCode"),
    ("SendMessage", "LRESULT", "HWND hWnd|UINT:WM Msg|WPARAM wParam|LPARAM lParam"),
    ("PostMessage", "BOOL", "HWND hWnd|UINT:WM Msg|WPARAM wParam|LPARAM lParam"),
    ("SendDlgItemMessage", "LRESULT", "HWND hDlg|int nIDDlgItem|UINT:WM Msg|WPARAM wParam|LPARAM lParam"),
    ("ShowWindow", "BOOL", "HWND hWnd|int:SW nCmdShow"),
    ("UpdateWindow", "BOOL", "HWND hWnd"),
    ("DestroyWindow", "BOOL", "HWND hWnd"),
    ("CloseWindow", "BOOL", "HWND hWnd"),
    ("EnableWindow", "BOOL", "HWND hWnd|BOOL bEnable"),
    ("IsWindow", "BOOL", "HWND hWnd"),
    ("SetFocus", "HWND", "HWND hWnd"),
    ("GetParent", "HWND", "HWND hWnd"),
    ("FindWindow", "HWND hWnd", "LPCSTR lpClassName|LPCSTR lpWindowName"),
    ("FindWindowEx", "HWND hWnd", "HWND hWndParent|HWND hWndChildAfter|LPCSTR lpszClass|LPCSTR lpszWindow"),
    ("GetForegroundWindow", "HWND hWnd", ""),
    ("SetForegroundWindow", "BOOL", "HWND hWnd"),
    ("GetDesktopWindow", "HWND hWnd", ""),
    ("MoveWindow", "BOOL", "HWND hWnd|int X|int Y|int nWidth|int nHeight|BOOL bRepaint"),
    ("SetWindowPos", "BOOL", "HWND hWnd|HWND hWndInsertAfter|int X|int Y|int cx|int cy|UINT uFlags"),
    ("GetWindowRect", "BOOL", "HWND hWnd|RECT* lpRect"),
    ("GetClientRect", "BOOL", "HWND hWnd|RECT* lpRect"),
    ("InvalidateRect", "BOOL", "HWND hWnd|RECT* lpRect|BOOL bErase"),
    ("SetWindowText", "BOOL", "HWND hWnd|LPCSTR lpString"),
    ("GetWindowText", "int", "HWND hWnd|LPSTR lpString|int nMaxCount"),
    ("GetWindowTextLength", "int", "HWND hWnd"),
    ("SetWindowLong", "LONG", "HWND hWnd|int nIndex|LONG dwNewLong"),
    ("GetWindowLong", "LONG", "HWND hWnd|int nIndex"),
    ("SetWindowLongPtr", "LONG_PTR", "HWND hWnd|int nIndex|LONG_PTR dwNewLong"),
    ("GetWindowLongPtr", "LONG_PTR", "HWND hWnd|int nIndex"),
    ("GetDlgItem", "HWND hItem", "HWND hDlg|int nIDDlgItem"),
    ("GetDlgItemText", "UINT", "HWND hDlg|int nIDDlgItem|LPSTR lpString|int cchMax"),
    ("SetDlgItemText", "BOOL", "HWND hDlg|int nIDDlgItem|LPCSTR lpString"),
    ("GetDlgItemInt", "UINT", "HWND hDlg|int nIDDlgItem|BOOL* lpTranslated|BOOL bSigned"),
    ("SetDlgItemInt", "BOOL", "HWND hDlg|int nIDDlgItem|UINT uValue|BOOL bSigned"),
    ("CheckDlgButton", "BOOL", "HWND hDlg|int nIDButton|UINT uCheck"),
    ("IsDlgButtonChecked", "UINT", "HWND hDlg|int nIDButton"),
    ("DialogBoxParam", "INT_PTR", "HINSTANCE hInstance|LPCSTR lpTemplateName|HWND hWndParent|DLGPROC lpDialogFunc|LPARAM dwInitParam"),
    ("CreateDialogParam", "HWND hDlg", "HINSTANCE hInstance|LPCSTR lpTemplateName|HWND hWndParent|DLGPROC lpDialogFunc|LPARAM dwInitParam"),
    ("EndDialog", "BOOL", "HWND hDlg|INT_PTR nResult"),
    ("CreateMenu", "HMENU hMenu", ""),
    ("CreatePopupMenu", "HMENU hMenu", ""),
    ("DestroyMenu", "BOOL", "HMENU hMenu"),
    ("AppendMenu", "BOOL", "HMENU hMenu|UINT:MF uFlags|UINT_PTR uIDNewItem|LPCSTR lpNewItem"),
    ("InsertMenu", "BOOL", "HMENU hMenu|UINT uPosition|UINT:MF uFlags|UINT_PTR uIDNewItem|LPCSTR lpNewItem"),
    ("ModifyMenu", "BOOL", "HMENU hMenu|UINT uPosition|UINT:MF uFlags|UINT_PTR uIDNewItem|LPCSTR lpNewItem"),
    ("CheckMenuItem", "DWORD", "HMENU hMenu|UINT uIDCheckItem|UINT:MF uCheck"),
    ("EnableMenuItem", "BOOL", "HMENU hMenu|UINT uIDEnableItem|UINT:MF uEnable"),
    ("SetMenu", "BOOL", "HWND hWnd|HMENU hMenu"),
    ("GetMenu", "HMENU hMenu", "HWND hWnd"),
    ("GetSubMenu", "HMENU hMenu", "HMENU hMenu|int nPos"),
    ("DrawMenuBar", "BOOL", "HWND hWnd"),
    ("TrackPopupMenu", "BOOL", "HMENU hMenu|UINT uFlags|int x|int y|int nReserved|HWND hWnd|RECT* prcRect"),
    ("LoadCursor", "HCURSOR hCursor", "HINSTANCE hInstance|LPCSTR:IDC lpCursorName"),
    ("LoadIcon", "HICON hIcon", "HINSTANCE hInstance|LPCSTR:IDI lpIconName"),
    ("LoadImage", "HANDLE hImage", "HINSTANCE hInst|LPCSTR name|UINT type|int cx|int cy|UINT fuLoad"),
    ("LoadMenu", "HMENU hMenu", "HINSTANCE hInstance|LPCSTR lpMenuName"),
    ("LoadString", "int", "HINSTANCE hInstance|UINT uID|LPSTR lpBuffer|int cchBufferMax"),
    ("SetCursor", "HCURSOR hCursor", "HCURSOR hCursor"),
    ("BeginPaint", "HDC hdc", "HWND hWnd|PAINTSTRUCT* lpPaint"),
    ("EndPaint", "BOOL", "HWND hWnd|PAINTSTRUCT* lpPaint"),
    ("GetDC", "HDC hdc", "HWND hWnd"),
    ("ReleaseDC", "int", "HWND hWnd|HDC hDC"),
    ("FillRect", "int", "HDC hDC|RECT* lprc|HBRUSH hbr"),
    ("DrawText", "int", "HDC hdc|LPCSTR lpchText|int cchText|RECT* lprc|UINT format"),
    ("SetTimer", "UINT_PTR", "HWND hWnd|UINT_PTR nIDEvent|UINT uElapse|TIMERPROC lpTimerFunc"),
    ("KillTimer", "BOOL", "HWND hWnd|UINT_PTR uIDEvent"),
    ("GetSystemMetrics", "int", "int nIndex"),
    ("GetAsyncKeyState", "SHORT", "int vKey"),
    ("GetKeyState", "SHORT", "int nVirtKey"),
    ("GetCursorPos", "BOOL", "POINT* lpPoint"),
    ("SetWindowsHookEx", "HHOOK hHook", "int idHook|HOOKPROC lpfn|HINSTANCE hmod|DWORD dwThreadId"),
    ("CallNextHookEx", "LRESULT", "HHOOK hhk|int nCode|WPARAM wParam|LPARAM lParam"),
    ("UnhookWindowsHookEx", "BOOL", "HHOOK hhk"),
    ("wsprintf", "int", "LPSTR buf|LPCSTR fmt|..."),
    ("OpenClipboard", "BOOL", "HWND hWndNewOwner"),
    ("CloseClipboard", "BOOL", ""),
    ("MessageBeep", "BOOL", "UINT:MB uType"),
    // ---- gdi32
    ("TextOut", "BOOL", "HDC hdc|int x|int y|LPCSTR lpString|int c"),
    ("SetTextColor", "COLORREF", "HDC hdc|COLORREF color"),
    ("SetBkColor", "COLORREF", "HDC hdc|COLORREF color"),
    ("SetBkMode", "int", "HDC hdc|int mode"),
    ("CreateSolidBrush", "HBRUSH hBrush", "COLORREF color"),
    ("CreatePen", "HPEN hPen", "int iStyle|int cWidth|COLORREF color"),
    ("CreateFont", "HFONT hFont", "int cHeight|int cWidth|int cEscapement|int cOrientation|int cWeight|DWORD bItalic|DWORD bUnderline|DWORD bStrikeOut|DWORD iCharSet|DWORD iOutPrecision|DWORD iClipPrecision|DWORD iQuality|DWORD iPitchAndFamily|LPCSTR pszFaceName"),
    ("SelectObject", "HGDIOBJ hOld", "HDC hdc|HGDIOBJ h"),
    ("DeleteObject", "BOOL", "HGDIOBJ ho"),
    ("GetStockObject", "HGDIOBJ hObj", "int i"),
    ("Rectangle", "BOOL", "HDC hdc|int left|int top|int right|int bottom"),
    ("Ellipse", "BOOL", "HDC hdc|int left|int top|int right|int bottom"),
    ("MoveToEx", "BOOL", "HDC hdc|int x|int y|POINT* lppt"),
    ("LineTo", "BOOL", "HDC hdc|int x|int y"),
    ("BitBlt", "BOOL", "HDC hdc|int x|int y|int cx|int cy|HDC hdcSrc|int x1|int y1|DWORD rop"),
    ("CreateCompatibleDC", "HDC hdcMem", "HDC hdc"),
    ("DeleteDC", "BOOL", "HDC hdc"),
    // ---- kernel32
    ("GetModuleHandle", "HMODULE hModule", "LPCSTR lpModuleName"),
    ("GetModuleFileName", "DWORD", "HMODULE hModule|LPSTR lpFilename|DWORD nSize"),
    ("LoadLibrary", "HMODULE hModule", "LPCSTR lpLibFileName"),
    ("LoadLibraryEx", "HMODULE hModule", "LPCSTR lpLibFileName|HANDLE hFile|DWORD dwFlags"),
    ("GetProcAddress", "FARPROC pfn", "HMODULE hModule|LPCSTR lpProcName"),
    ("FreeLibrary", "BOOL", "HMODULE hLibModule"),
    ("ExitProcess", "void", "UINT uExitCode"),
    ("TerminateProcess", "BOOL", "HANDLE hProcess|UINT uExitCode"),
    ("ExitThread", "void", "DWORD dwExitCode"),
    ("Sleep", "void", "DWORD dwMilliseconds"),
    ("GetLastError", "DWORD", ""),
    ("SetLastError", "void", "DWORD dwErrCode"),
    ("GetTickCount", "DWORD", ""),
    ("GetTickCount64", "ULONGLONG", ""),
    ("QueryPerformanceCounter", "BOOL", "LARGE_INTEGER* lpPerformanceCount"),
    ("GetCurrentProcess", "HANDLE hProcess", ""),
    ("GetCurrentProcessId", "DWORD", ""),
    ("GetCurrentThreadId", "DWORD", ""),
    ("OpenProcess", "HANDLE hProcess", "DWORD dwDesiredAccess|BOOL bInheritHandle|DWORD dwProcessId"),
    ("CreateFile", "HANDLE hFile", "LPCSTR lpFileName|DWORD:GENERIC dwDesiredAccess|DWORD:SHARE dwShareMode|LPVOID lpSecurityAttributes|DWORD:DISP dwCreationDisposition|DWORD dwFlagsAndAttributes|HANDLE hTemplateFile"),
    ("ReadFile", "BOOL", "HANDLE hFile|LPVOID lpBuffer|DWORD nNumberOfBytesToRead|LPDWORD lpNumberOfBytesRead|LPVOID lpOverlapped"),
    ("WriteFile", "BOOL", "HANDLE hFile|LPCVOID lpBuffer|DWORD nNumberOfBytesToWrite|LPDWORD lpNumberOfBytesWritten|LPVOID lpOverlapped"),
    ("CloseHandle", "BOOL", "HANDLE hObject"),
    ("DeleteFile", "BOOL", "LPCSTR lpFileName"),
    ("CopyFile", "BOOL", "LPCSTR lpExistingFileName|LPCSTR lpNewFileName|BOOL bFailIfExists"),
    ("MoveFile", "BOOL", "LPCSTR lpExistingFileName|LPCSTR lpNewFileName"),
    ("CreateDirectory", "BOOL", "LPCSTR lpPathName|LPVOID lpSecurityAttributes"),
    ("GetFileSize", "DWORD", "HANDLE hFile|LPDWORD lpFileSizeHigh"),
    ("SetFilePointer", "DWORD", "HANDLE hFile|LONG lDistanceToMove|PLONG lpDistanceToMoveHigh|DWORD dwMoveMethod"),
    ("FindFirstFile", "HANDLE hFind", "LPCSTR lpFileName|LPVOID lpFindFileData"),
    ("FindNextFile", "BOOL", "HANDLE hFindFile|LPVOID lpFindFileData"),
    ("FindClose", "BOOL", "HANDLE hFindFile"),
    ("GetTempPath", "DWORD", "DWORD nBufferLength|LPSTR lpBuffer"),
    ("GetCommandLine", "LPSTR lpCmdLine", ""),
    ("GetStartupInfo", "void", "LPVOID lpStartupInfo"),
    ("GetEnvironmentVariable", "DWORD", "LPCSTR lpName|LPSTR lpBuffer|DWORD nSize"),
    ("SetEnvironmentVariable", "BOOL", "LPCSTR lpName|LPCSTR lpValue"),
    ("OutputDebugString", "void", "LPCSTR lpOutputString"),
    ("IsDebuggerPresent", "BOOL", ""),
    ("VirtualAlloc", "LPVOID lpMem", "LPVOID lpAddress|SIZE_T dwSize|DWORD:MEM flAllocationType|DWORD:PAGE flProtect"),
    ("VirtualAllocEx", "LPVOID lpMem", "HANDLE hProcess|LPVOID lpAddress|SIZE_T dwSize|DWORD:MEM flAllocationType|DWORD:PAGE flProtect"),
    ("VirtualFree", "BOOL", "LPVOID lpAddress|SIZE_T dwSize|DWORD:MEM dwFreeType"),
    ("VirtualProtect", "BOOL", "LPVOID lpAddress|SIZE_T dwSize|DWORD:PAGE flNewProtect|PDWORD lpflOldProtect"),
    ("WriteProcessMemory", "BOOL", "HANDLE hProcess|LPVOID lpBaseAddress|LPCVOID lpBuffer|SIZE_T nSize|SIZE_T* lpNumberOfBytesWritten"),
    ("ReadProcessMemory", "BOOL", "HANDLE hProcess|LPCVOID lpBaseAddress|LPVOID lpBuffer|SIZE_T nSize|SIZE_T* lpNumberOfBytesRead"),
    ("CreateRemoteThread", "HANDLE hThread", "HANDLE hProcess|LPVOID lpThreadAttributes|SIZE_T dwStackSize|LPVOID lpStartAddress|LPVOID lpParameter|DWORD dwCreationFlags|LPDWORD lpThreadId"),
    ("CreateThread", "HANDLE hThread", "LPVOID lpThreadAttributes|SIZE_T dwStackSize|LPVOID lpStartAddress|LPVOID lpParameter|DWORD dwCreationFlags|LPDWORD lpThreadId"),
    ("WaitForSingleObject", "DWORD", "HANDLE hHandle|DWORD dwMilliseconds"),
    ("CreateMutex", "HANDLE hMutex", "LPVOID lpMutexAttributes|BOOL bInitialOwner|LPCSTR lpName"),
    ("CreateEvent", "HANDLE hEvent", "LPVOID lpEventAttributes|BOOL bManualReset|BOOL bInitialState|LPCSTR lpName"),
    ("SetEvent", "BOOL", "HANDLE hEvent"),
    ("CreateProcess", "BOOL", "LPCSTR lpApplicationName|LPSTR lpCommandLine|LPVOID lpProcessAttributes|LPVOID lpThreadAttributes|BOOL bInheritHandles|DWORD dwCreationFlags|LPVOID lpEnvironment|LPCSTR lpCurrentDirectory|LPVOID lpStartupInfo|LPVOID lpProcessInformation"),
    ("WinExec", "UINT", "LPCSTR lpCmdLine|UINT:SW uCmdShow"),
    ("GetProcessHeap", "HANDLE hHeap", ""),
    ("HeapAlloc", "LPVOID lpMem", "HANDLE hHeap|DWORD dwFlags|SIZE_T dwBytes"),
    ("HeapFree", "BOOL", "HANDLE hHeap|DWORD dwFlags|LPVOID lpMem"),
    ("LocalAlloc", "HLOCAL hMem", "UINT uFlags|SIZE_T uBytes"),
    ("LocalFree", "HLOCAL", "HLOCAL hMem"),
    ("GlobalAlloc", "HGLOBAL hMem", "UINT uFlags|SIZE_T dwBytes"),
    ("GlobalFree", "HGLOBAL", "HGLOBAL hMem"),
    ("lstrlen", "int", "LPCSTR lpString"),
    ("lstrcpy", "LPSTR", "LPSTR lpString1|LPCSTR lpString2"),
    ("lstrcat", "LPSTR", "LPSTR lpString1|LPCSTR lpString2"),
    ("lstrcmp", "int", "LPCSTR lpString1|LPCSTR lpString2"),
    ("lstrcmpi", "int", "LPCSTR lpString1|LPCSTR lpString2"),
    ("MultiByteToWideChar", "int", "UINT CodePage|DWORD dwFlags|LPCSTR lpMultiByteStr|int cbMultiByte|LPWSTR lpWideCharStr|int cchWideChar"),
    ("WideCharToMultiByte", "int", "UINT CodePage|DWORD dwFlags|LPCWSTR lpWideCharStr|int cchWideChar|LPSTR lpMultiByteStr|int cbMultiByte|LPCSTR lpDefaultChar|BOOL* lpUsedDefaultChar"),
    ("GetSystemTime", "void", "LPVOID lpSystemTime"),
    ("GetLocalTime", "void", "LPVOID lpSystemTime"),
    ("AllocConsole", "BOOL", ""),
    ("GetStdHandle", "HANDLE hStd", "DWORD nStdHandle"),
    ("WriteConsole", "BOOL", "HANDLE hConsoleOutput|LPCVOID lpBuffer|DWORD nNumberOfCharsToWrite|LPDWORD lpNumberOfCharsWritten|LPVOID lpReserved"),
    ("SetConsoleTitle", "BOOL", "LPCSTR lpConsoleTitle"),
    // ---- advapi32 / shell32
    ("RegOpenKeyEx", "LSTATUS", "HKEY hKey|LPCSTR lpSubKey|DWORD ulOptions|DWORD samDesired|HKEY* phkResult"),
    ("RegQueryValueEx", "LSTATUS", "HKEY hKey|LPCSTR lpValueName|LPDWORD lpReserved|LPDWORD lpType|LPBYTE lpData|LPDWORD lpcbData"),
    ("RegSetValueEx", "LSTATUS", "HKEY hKey|LPCSTR lpValueName|DWORD Reserved|DWORD dwType|LPBYTE lpData|DWORD cbData"),
    ("RegCreateKeyEx", "LSTATUS", "HKEY hKey|LPCSTR lpSubKey|DWORD Reserved|LPSTR lpClass|DWORD dwOptions|DWORD samDesired|LPVOID lpSecurityAttributes|HKEY* phkResult|LPDWORD lpdwDisposition"),
    ("RegCloseKey", "LSTATUS", "HKEY hKey"),
    ("ShellExecute", "HINSTANCE", "HWND hwnd|LPCSTR lpOperation|LPCSTR lpFile|LPCSTR lpParameters|LPCSTR lpDirectory|int:SW nShowCmd"),
    // ---- libc
    ("printf", "int", "const char* format|..."),
    ("wprintf", "int", "const wchar_t* format|..."),
    ("puts", "int", "const char* s"),
    ("putchar", "int", "int c"),
    ("getchar", "int", ""),
    ("scanf", "int", "const char* format|..."),
    ("sprintf", "int", "char* buf|const char* format|..."),
    ("snprintf", "int", "char* buf|size_t n|const char* format|..."),
    ("fprintf", "int", "FILE* stream|const char* format|..."),
    ("sscanf", "int", "const char* str|const char* format|..."),
    ("fopen", "FILE* fp", "const char* path|const char* mode"),
    ("fclose", "int", "FILE* stream"),
    ("fgets", "char*", "char* s|int n|FILE* stream"),
    ("fputs", "int", "const char* s|FILE* stream"),
    ("fread", "size_t", "void* ptr|size_t size|size_t n|FILE* stream"),
    ("fwrite", "size_t", "const void* ptr|size_t size|size_t n|FILE* stream"),
    ("malloc", "void* ptr", "size_t size"),
    ("calloc", "void* ptr", "size_t n|size_t size"),
    ("realloc", "void* ptr", "void* p|size_t size"),
    ("free", "void", "void* ptr"),
    ("memset", "void*", "void* s|int c|size_t n"),
    ("memcpy", "void*", "void* dst|const void* src|size_t n"),
    ("memmove", "void*", "void* dst|const void* src|size_t n"),
    ("memcmp", "int", "const void* a|const void* b|size_t n"),
    ("strlen", "size_t len", "const char* s"),
    ("strcmp", "int", "const char* a|const char* b"),
    ("strncmp", "int", "const char* a|const char* b|size_t n"),
    ("strcpy", "char*", "char* dst|const char* src"),
    ("strncpy", "char*", "char* dst|const char* src|size_t n"),
    ("strcat", "char*", "char* dst|const char* src"),
    ("strchr", "char*", "const char* s|int c"),
    ("strstr", "char*", "const char* s|const char* sub"),
    ("strdup", "char*", "const char* s"),
    ("atoi", "int", "const char* s"),
    ("atol", "long", "const char* s"),
    ("strtol", "long", "const char* s|char** end|int base"),
    ("rand", "int", ""),
    ("srand", "void", "unsigned int seed"),
    ("time", "time_t", "time_t* t"),
    ("system", "int", "const char* command"),
    ("getenv", "char*", "const char* name"),
    ("exit", "void", "int status"),
    ("_exit", "void", "int status"),
    ("abort", "void", ""),
    ("sleep", "unsigned int", "unsigned int seconds"),
    ("usleep", "int", "unsigned int usec"),
    ("open", "int fd", "const char* path|int flags|..."),
    ("close", "int", "int fd"),
    ("read", "ssize_t", "int fd|void* buf|size_t n"),
    ("write", "ssize_t", "int fd|const void* buf|size_t n"),
    ("__stack_chk_fail", "void", ""),
];

/// Funções que não retornam.
pub static NORETURN: &[&str] = &[
    "ExitProcess", "ExitThread", "exit", "_exit", "abort", "__stack_chk_fail", "__assert_fail",
    "_assert", "longjmp", "__cxa_throw", "_CxxThrowException", "__fortify_fail", "err", "errx",
    "__chk_fail", "_Exit", "quick_exit", "FatalExit", "RaiseFailFastException",
];

#[derive(Debug, Clone)]
pub struct Param {
    pub ty: String,
    pub group: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Api {
    pub name: String,
    pub ret: String,
    /// Nome sugerido para a variável que recebe o retorno (`hWnd`, `hMenu`...).
    pub ret_name: Option<String>,
    pub params: Vec<Param>,
    pub variadic: bool,
}

impl Api {
    pub fn returns_void(&self) -> bool {
        self.ret == "void"
    }
}

/// Nome limpo para exibição e busca: tira `DLL!`, `@plt`, `__imp_` e o `_`/`@N` do stdcall.
pub fn clean_name(raw: &str) -> String {
    let s = raw.rsplit('!').next().unwrap_or(raw);
    let s = s.strip_suffix("@plt").unwrap_or(s);
    let s = s.strip_prefix("__imp_").unwrap_or(s);
    // `_Foo@16` (stdcall x86)
    if let Some((head, tail)) = s.rsplit_once('@') {
        if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) {
            return head.strip_prefix('_').unwrap_or(head).to_string();
        }
    }
    s.to_string()
}

/// Procura a assinatura (`MessageBoxA` → `MessageBox`).
pub fn lookup(name: &str) -> Option<Api> {
    let n = clean_name(name);
    let candidates = [n.as_str(), n.strip_suffix(['A', 'W']).unwrap_or(""), n.strip_prefix('_').unwrap_or("")];
    for c in candidates {
        if c.is_empty() {
            continue;
        }
        if let Some(&(base, ret, params)) = APIS.iter().find(|(b, _, _)| *b == c) {
            return Some(parse_api(base, ret, params));
        }
    }
    None
}

pub fn is_noreturn(name: &str) -> bool {
    let n = clean_name(name);
    NORETURN.contains(&n.as_str())
}

fn parse_api(name: &str, ret: &str, params: &str) -> Api {
    let (ret_ty, ret_name) = match ret.rsplit_once(' ') {
        Some((t, n)) if !matches!(t, "unsigned" | "signed" | "long" | "short") && n.starts_with(|c: char| c.is_ascii_lowercase()) => {
            (t.to_string(), Some(n.to_string()))
        }
        _ => (ret.to_string(), None),
    };
    let mut variadic = false;
    let mut ps = vec![];
    for p in params.split('|').filter(|p| !p.is_empty()) {
        if p == "..." {
            variadic = true;
            continue;
        }
        let ty = p.rsplit_once(' ').map_or(p, |(t, _)| t);
        let (ty, group) = match ty.split_once(':') {
            Some((t, g)) => (t.to_string(), Some(g.to_string())),
            None => (ty.to_string(), None),
        };
        ps.push(Param { ty, group });
    }
    Api { name: name.to_string(), ret: ret_ty, ret_name, params: ps, variadic }
}

/// Tipo ponteiro/handle: 0 vira `NULL`.
pub fn is_pointer_type(ty: &str) -> bool {
    ty.ends_with('*')
        || ty.starts_with("LP")
        || ty.starts_with("PL")
        || ty == "PDWORD"
        || ty == "FARPROC"
        || ty == "WNDPROC"
        || ty == "DLGPROC"
        || ty == "TIMERPROC"
        || ty == "HOOKPROC"
        || (ty.starts_with('H') && ty.len() > 1 && ty.chars().all(|c| c.is_ascii_uppercase()))
}

// ------------------------------------------------------------------ constantes

/// Grupos "um valor de cada": (máscara, [(valor, nome)]).
type Field = (u64, &'static [(u64, &'static str)]);

struct Group {
    /// Campos mutuamente exclusivos (ex.: botões do MessageBox).
    fields: &'static [Field],
    /// Flags simples, testadas na ordem (compostas primeiro).
    flags: &'static [(u64, &'static str)],
}

static MB: Group = Group {
    fields: &[
        (0xF, &[(0, "MB_OK"), (1, "MB_OKCANCEL"), (2, "MB_ABORTRETRYIGNORE"), (3, "MB_YESNOCANCEL"), (4, "MB_YESNO"), (5, "MB_RETRYCANCEL"), (6, "MB_CANCELTRYCONTINUE")]),
        (0xF0, &[(0x10, "MB_ICONERROR"), (0x20, "MB_ICONQUESTION"), (0x30, "MB_ICONWARNING"), (0x40, "MB_ICONINFORMATION")]),
        (0xF00, &[(0x100, "MB_DEFBUTTON2"), (0x200, "MB_DEFBUTTON3"), (0x300, "MB_DEFBUTTON4")]),
        (0x3000, &[(0x1000, "MB_SYSTEMMODAL"), (0x2000, "MB_TASKMODAL")]),
    ],
    flags: &[(0x4000, "MB_HELP"), (0x10000, "MB_SETFOREGROUND"), (0x40000, "MB_TOPMOST"), (0x80000, "MB_RIGHT"), (0x200000, "MB_SERVICE_NOTIFICATION")],
};

static WS: Group = Group {
    fields: &[],
    flags: &[
        (0x00CF_0000, "WS_OVERLAPPEDWINDOW"),
        (0x8088_0000, "WS_POPUPWINDOW"),
        (0x00C0_0000, "WS_CAPTION"),
        (0x8000_0000, "WS_POPUP"),
        (0x4000_0000, "WS_CHILD"),
        (0x2000_0000, "WS_MINIMIZE"),
        (0x1000_0000, "WS_VISIBLE"),
        (0x0800_0000, "WS_DISABLED"),
        (0x0400_0000, "WS_CLIPSIBLINGS"),
        (0x0200_0000, "WS_CLIPCHILDREN"),
        (0x0100_0000, "WS_MAXIMIZE"),
        (0x0080_0000, "WS_BORDER"),
        (0x0040_0000, "WS_DLGFRAME"),
        (0x0020_0000, "WS_VSCROLL"),
        (0x0010_0000, "WS_HSCROLL"),
        (0x0008_0000, "WS_SYSMENU"),
        (0x0004_0000, "WS_THICKFRAME"),
        (0x0002_0000, "WS_MINIMIZEBOX"),
        (0x0001_0000, "WS_MAXIMIZEBOX"),
    ],
};

/// Estilos de controles (16 bits baixos), por classe.
static BS: Group = Group {
    fields: &[(0xF, &[(0, "BS_PUSHBUTTON"), (1, "BS_DEFPUSHBUTTON"), (2, "BS_CHECKBOX"), (3, "BS_AUTOCHECKBOX"), (4, "BS_RADIOBUTTON"), (5, "BS_3STATE"), (6, "BS_AUTO3STATE"), (7, "BS_GROUPBOX"), (9, "BS_AUTORADIOBUTTON")])],
    flags: &[(0x100, "BS_LEFT"), (0x200, "BS_RIGHT"), (0x300, "BS_CENTER"), (0x2000, "BS_MULTILINE"), (0x8000, "BS_FLAT")],
};

static ES: Group = Group {
    fields: &[],
    flags: &[(0x1, "ES_CENTER"), (0x2, "ES_RIGHT"), (0x4, "ES_MULTILINE"), (0x8, "ES_UPPERCASE"), (0x10, "ES_LOWERCASE"), (0x20, "ES_PASSWORD"), (0x40, "ES_AUTOVSCROLL"), (0x80, "ES_AUTOHSCROLL"), (0x800, "ES_READONLY"), (0x2000, "ES_NUMBER")],
};

static WSEX: Group = Group {
    fields: &[],
    flags: &[
        (0x300, "WS_EX_OVERLAPPEDWINDOW"),
        (0x1, "WS_EX_DLGMODALFRAME"),
        (0x8, "WS_EX_TOPMOST"),
        (0x10, "WS_EX_ACCEPTFILES"),
        (0x20, "WS_EX_TRANSPARENT"),
        (0x80, "WS_EX_TOOLWINDOW"),
        (0x100, "WS_EX_WINDOWEDGE"),
        (0x200, "WS_EX_CLIENTEDGE"),
        (0x40000, "WS_EX_APPWINDOW"),
        (0x80000, "WS_EX_LAYERED"),
        (0x8000000, "WS_EX_NOACTIVATE"),
    ],
};

static MF: Group = Group {
    fields: &[],
    flags: &[(0x1, "MF_GRAYED"), (0x2, "MF_DISABLED"), (0x4, "MF_BITMAP"), (0x8, "MF_CHECKED"), (0x10, "MF_POPUP"), (0x20, "MF_MENUBARBREAK"), (0x40, "MF_MENUBREAK"), (0x100, "MF_OWNERDRAW"), (0x400, "MF_BYPOSITION"), (0x800, "MF_SEPARATOR")],
};

static CS: Group = Group {
    fields: &[],
    flags: &[(0x1, "CS_VREDRAW"), (0x2, "CS_HREDRAW"), (0x8, "CS_DBLCLKS"), (0x20, "CS_OWNDC"), (0x40, "CS_CLASSDC"), (0x80, "CS_PARENTDC"), (0x200, "CS_NOCLOSE"), (0x800, "CS_SAVEBITS"), (0x20000, "CS_DROPSHADOW")],
};

static GENERIC: Group = Group {
    fields: &[],
    flags: &[(0x8000_0000, "GENERIC_READ"), (0x4000_0000, "GENERIC_WRITE"), (0x2000_0000, "GENERIC_EXECUTE"), (0x1000_0000, "GENERIC_ALL")],
};

static SHARE: Group = Group {
    fields: &[],
    flags: &[(1, "FILE_SHARE_READ"), (2, "FILE_SHARE_WRITE"), (4, "FILE_SHARE_DELETE")],
};

static MEM: Group = Group {
    fields: &[],
    flags: &[(0x1000, "MEM_COMMIT"), (0x2000, "MEM_RESERVE"), (0x4000, "MEM_DECOMMIT"), (0x8000, "MEM_RELEASE"), (0x80000, "MEM_RESET"), (0x20000000, "MEM_LARGE_PAGES")],
};

static WM: &[(u64, &str)] = &[
    (0x0000, "WM_NULL"), (0x0001, "WM_CREATE"), (0x0002, "WM_DESTROY"), (0x0003, "WM_MOVE"), (0x0005, "WM_SIZE"),
    (0x0006, "WM_ACTIVATE"), (0x0007, "WM_SETFOCUS"), (0x0008, "WM_KILLFOCUS"), (0x000A, "WM_ENABLE"),
    (0x000C, "WM_SETTEXT"), (0x000D, "WM_GETTEXT"), (0x000E, "WM_GETTEXTLENGTH"), (0x000F, "WM_PAINT"),
    (0x0010, "WM_CLOSE"), (0x0011, "WM_QUERYENDSESSION"), (0x0012, "WM_QUIT"), (0x0014, "WM_ERASEBKGND"),
    (0x0018, "WM_SHOWWINDOW"), (0x001C, "WM_ACTIVATEAPP"), (0x0020, "WM_SETCURSOR"), (0x0021, "WM_MOUSEACTIVATE"),
    (0x0024, "WM_GETMINMAXINFO"), (0x002B, "WM_DRAWITEM"), (0x0030, "WM_SETFONT"), (0x0031, "WM_GETFONT"),
    (0x0046, "WM_WINDOWPOSCHANGING"), (0x0047, "WM_WINDOWPOSCHANGED"), (0x004E, "WM_NOTIFY"),
    (0x007B, "WM_CONTEXTMENU"), (0x007F, "WM_GETICON"), (0x0080, "WM_SETICON"), (0x0081, "WM_NCCREATE"),
    (0x0082, "WM_NCDESTROY"), (0x0083, "WM_NCCALCSIZE"), (0x0084, "WM_NCHITTEST"), (0x0085, "WM_NCPAINT"),
    (0x0086, "WM_NCACTIVATE"), (0x00A1, "WM_NCLBUTTONDOWN"), (0x0100, "WM_KEYDOWN"), (0x0101, "WM_KEYUP"),
    (0x0102, "WM_CHAR"), (0x0104, "WM_SYSKEYDOWN"), (0x0105, "WM_SYSKEYUP"), (0x0110, "WM_INITDIALOG"),
    (0x0111, "WM_COMMAND"), (0x0112, "WM_SYSCOMMAND"), (0x0113, "WM_TIMER"), (0x0114, "WM_HSCROLL"),
    (0x0115, "WM_VSCROLL"), (0x0116, "WM_INITMENU"), (0x0117, "WM_INITMENUPOPUP"), (0x011F, "WM_MENUSELECT"),
    (0x0133, "WM_CTLCOLOREDIT"), (0x0136, "WM_CTLCOLORDLG"), (0x0138, "WM_CTLCOLORSTATIC"),
    (0x0200, "WM_MOUSEMOVE"), (0x0201, "WM_LBUTTONDOWN"), (0x0202, "WM_LBUTTONUP"), (0x0203, "WM_LBUTTONDBLCLK"),
    (0x0204, "WM_RBUTTONDOWN"), (0x0205, "WM_RBUTTONUP"), (0x0206, "WM_RBUTTONDBLCLK"), (0x0207, "WM_MBUTTONDOWN"),
    (0x0208, "WM_MBUTTONUP"), (0x020A, "WM_MOUSEWHEEL"), (0x0214, "WM_SIZING"), (0x0216, "WM_MOVING"),
    (0x0231, "WM_ENTERSIZEMOVE"), (0x0232, "WM_EXITSIZEMOVE"), (0x0233, "WM_DROPFILES"), (0x02A3, "WM_MOUSELEAVE"),
    (0x0300, "WM_CUT"), (0x0301, "WM_COPY"), (0x0302, "WM_PASTE"), (0x0312, "WM_HOTKEY"), (0x0400, "WM_USER"),
    (0x8000, "WM_APP"),
];

static SW: &[(u64, &str)] = &[
    (0, "SW_HIDE"), (1, "SW_SHOWNORMAL"), (2, "SW_SHOWMINIMIZED"), (3, "SW_SHOWMAXIMIZED"), (4, "SW_SHOWNOACTIVATE"),
    (5, "SW_SHOW"), (6, "SW_MINIMIZE"), (7, "SW_SHOWMINNOACTIVE"), (8, "SW_SHOWNA"), (9, "SW_RESTORE"), (10, "SW_SHOWDEFAULT"),
];

static IDC: &[(u64, &str)] = &[
    (32512, "IDC_ARROW"), (32513, "IDC_IBEAM"), (32514, "IDC_WAIT"), (32515, "IDC_CROSS"), (32516, "IDC_UPARROW"),
    (32642, "IDC_SIZENWSE"), (32643, "IDC_SIZENESW"), (32644, "IDC_SIZEWE"), (32645, "IDC_SIZENS"), (32646, "IDC_SIZEALL"),
    (32648, "IDC_NO"), (32649, "IDC_HAND"), (32650, "IDC_APPSTARTING"), (32651, "IDC_HELP"),
];

static IDI: &[(u64, &str)] = &[
    (32512, "IDI_APPLICATION"), (32513, "IDI_ERROR"), (32514, "IDI_QUESTION"), (32515, "IDI_WARNING"),
    (32516, "IDI_INFORMATION"), (32517, "IDI_WINLOGO"), (32518, "IDI_SHIELD"),
];

static DISP: &[(u64, &str)] = &[(1, "CREATE_NEW"), (2, "CREATE_ALWAYS"), (3, "OPEN_EXISTING"), (4, "OPEN_ALWAYS"), (5, "TRUNCATE_EXISTING")];

static PAGE: &[(u64, &str)] = &[
    (0x01, "PAGE_NOACCESS"), (0x02, "PAGE_READONLY"), (0x04, "PAGE_READWRITE"), (0x08, "PAGE_WRITECOPY"),
    (0x10, "PAGE_EXECUTE"), (0x20, "PAGE_EXECUTE_READ"), (0x40, "PAGE_EXECUTE_READWRITE"), (0x80, "PAGE_EXECUTE_WRITECOPY"),
];

static PM: &[(u64, &str)] = &[(0, "PM_NOREMOVE"), (1, "PM_REMOVE")];

static COLOR: &[(u64, &str)] = &[
    (0, "COLOR_SCROLLBAR"), (1, "COLOR_BACKGROUND"), (2, "COLOR_ACTIVECAPTION"), (3, "COLOR_INACTIVECAPTION"),
    (4, "COLOR_MENU"), (5, "COLOR_WINDOW"), (6, "COLOR_WINDOWFRAME"), (7, "COLOR_MENUTEXT"), (8, "COLOR_WINDOWTEXT"),
    (9, "COLOR_CAPTIONTEXT"), (10, "COLOR_ACTIVEBORDER"), (11, "COLOR_INACTIVEBORDER"), (12, "COLOR_APPWORKSPACE"),
    (13, "COLOR_HIGHLIGHT"), (14, "COLOR_HIGHLIGHTTEXT"), (15, "COLOR_BTNFACE"), (16, "COLOR_BTNSHADOW"),
    (17, "COLOR_GRAYTEXT"), (18, "COLOR_BTNTEXT"),
];

fn exact(table: &[(u64, &str)], v: u64) -> Option<String> {
    table.iter().find(|(k, _)| *k == v).map(|(_, n)| n.to_string())
}

fn flags(g: &Group, v: u64, zero_name: Option<&str>) -> Option<String> {
    let mut rest = v;
    let mut parts: Vec<&str> = vec![];
    for &(mask, values) in g.fields {
        let f = rest & mask;
        if let Some((_, n)) = values.iter().find(|(k, _)| *k == f) {
            if f != 0 || parts.is_empty() && mask == g.fields[0].0 {
                parts.push(n);
            }
            rest &= !mask;
        }
    }
    for &(bits, n) in g.flags {
        if bits != 0 && rest & bits == bits {
            parts.push(n);
            rest &= !bits;
        }
    }
    if rest != 0 {
        return None; // sobrou bit sem nome: melhor mostrar o número
    }
    if parts.is_empty() {
        return zero_name.map(str::to_string);
    }
    Some(parts.join(" | "))
}

/// Nome simbólico de uma constante para um grupo (`MB`, `WS`...). `class` é a
/// classe da janela em `CreateWindowEx` (para estilos `BS_`/`ES_`).
pub fn constant(group: &str, value: i64, class: Option<&str>) -> Option<String> {
    let v = value as u64 & 0xFFFF_FFFF;
    match group {
        "MB" => flags(&MB, v, Some("MB_OK")),
        "WS" => {
            let low = v & 0xFFFF;
            let high = v & !0xFFFF;
            let low_name = match class.map(str::to_ascii_uppercase).as_deref() {
                Some("BUTTON") => flags(&BS, low, Some("BS_PUSHBUTTON")),
                Some("EDIT") => flags(&ES, low, None),
                _ if low == 0 => None,
                _ => return None,
            };
            if low != 0 && low_name.is_none() {
                return None;
            }
            let high_name = if high == 0 { None } else { Some(flags(&WS, high, None)?) };
            match (high_name, low_name) {
                (Some(h), Some(l)) => Some(format!("{h} | {l}")),
                (Some(h), None) => Some(h),
                (None, Some(l)) => Some(l),
                (None, None) => Some("WS_OVERLAPPED".into()),
            }
        }
        "WSEX" => if v == 0 { None } else { flags(&WSEX, v, None) },
        "MF" => flags(&MF, v, Some("MF_STRING")),
        "CS" => if v == 0 { None } else { flags(&CS, v, None) },
        "GENERIC" => flags(&GENERIC, v, None),
        "SHARE" => if v == 0 { None } else { flags(&SHARE, v, None) },
        "MEM" => flags(&MEM, v, None),
        "CW" => (v == 0x8000_0000).then(|| "CW_USEDEFAULT".to_string()),
        "WM" => exact(WM, v),
        "SW" => exact(SW, v),
        "IDC" => exact(IDC, v),
        "IDI" => exact(IDI, v),
        "DISP" => exact(DISP, v),
        "PAGE" => exact(PAGE, v),
        "PM" => exact(PM, v),
        "COLOR1" => (1..=31).contains(&v).then(|| exact(COLOR, v - 1)).flatten().map(|c| format!("(HBRUSH)({c} + 1)")),
        _ => None,
    }
}

// ------------------------------------------------------------------ structs

pub struct StructField {
    pub off: i64,
    pub name: &'static str,
    pub group: Option<&'static str>,
}

pub struct StructDef {
    pub name: &'static str,
    /// Nome sugerido para a variável local.
    pub var: &'static str,
    pub size: i64,
    pub fields: Vec<StructField>,
}

/// Campo: deslocamento, tamanho (só documenta o layout), nome, grupo de constantes.
fn sf(off: i64, _size: u8, name: &'static str, group: Option<&'static str>) -> StructField {
    StructField { off, name, group }
}

/// Layout da struct para 32 ou 64 bits.
pub fn struct_def(ty: &str, ptr64: bool) -> Option<StructDef> {
    let p = if ptr64 { 8 } else { 4 };
    let base = ty.trim_end_matches('*').trim().trim_start_matches("const ");
    let base = base.strip_suffix(['A', 'W']).filter(|b| b.starts_with("WNDCLASS")).unwrap_or(base);
    Some(match base {
        "WNDCLASS" => {
            // style, lpfnWndProc, cbClsExtra, cbWndExtra, hInstance, hIcon, hCursor, hbrBackground, lpszMenuName, lpszClassName
            let mut o = 0i64;
            let mut fs = vec![sf(0, 4, "style", Some("CS"))];
            o += if ptr64 { 8 } else { 4 };
            fs.push(sf(o, p, "lpfnWndProc", None));
            o += p as i64;
            fs.push(sf(o, 4, "cbClsExtra", None));
            fs.push(sf(o + 4, 4, "cbWndExtra", None));
            o += 8;
            for (n, g) in [("hInstance", None), ("hIcon", None), ("hCursor", None), ("hbrBackground", Some("COLOR1")), ("lpszMenuName", None), ("lpszClassName", None)] {
                fs.push(sf(o, p, n, g));
                o += p as i64;
            }
            StructDef { name: "WNDCLASSA", var: "wc", size: o, fields: fs }
        }
        "WNDCLASSEX" => {
            let mut fs = vec![sf(0, 4, "cbSize", None), sf(4, 4, "style", Some("CS"))];
            let mut o = 8i64;
            fs.push(sf(o, p, "lpfnWndProc", None));
            o += p as i64;
            fs.push(sf(o, 4, "cbClsExtra", None));
            fs.push(sf(o + 4, 4, "cbWndExtra", None));
            o += 8;
            for (n, g) in [("hInstance", None), ("hIcon", None), ("hCursor", None), ("hbrBackground", Some("COLOR1")), ("lpszMenuName", None), ("lpszClassName", None), ("hIconSm", None)] {
                fs.push(sf(o, p, n, g));
                o += p as i64;
            }
            StructDef { name: "WNDCLASSEXA", var: "wcex", size: o, fields: fs }
        }
        "MSG" => {
            let mut fs = vec![sf(0, p, "hwnd", None)];
            let mut o = p as i64;
            fs.push(sf(o, 4, "message", Some("WM")));
            o += p as i64; // alinhamento
            fs.push(sf(o, p, "wParam", None));
            o += p as i64;
            fs.push(sf(o, p, "lParam", None));
            o += p as i64;
            fs.push(sf(o, 4, "time", None));
            fs.push(sf(o + 4, 8, "pt", None));
            let size = if ptr64 { 0x30 } else { 0x1c };
            StructDef { name: "MSG", var: "msg", size, fields: fs }
        }
        "PAINTSTRUCT" => {
            let fs = vec![sf(0, p, "hdc", None), sf(p as i64, 4, "fErase", None), sf(p as i64 + 4, 16, "rcPaint", None)];
            StructDef { name: "PAINTSTRUCT", var: "ps", size: if ptr64 { 0x48 } else { 0x40 }, fields: fs }
        }
        "RECT" => StructDef {
            name: "RECT",
            var: "rc",
            size: 16,
            fields: vec![sf(0, 4, "left", None), sf(4, 4, "top", None), sf(8, 4, "right", None), sf(12, 4, "bottom", None)],
        },
        "POINT" => StructDef { name: "POINT", var: "pt", size: 8, fields: vec![sf(0, 4, "x", None), sf(4, 4, "y", None)] },
        _ => return None,
    })
}

/// Assinaturas conhecidas para funções do próprio programa (pelo nome).
/// Parâmetro conhecido: (tipo, nome, grupo de constantes).
pub type KnownParam = (&'static str, &'static str, Option<&'static str>);

pub fn known_signature(name: &str) -> Option<(&'static str, &'static [KnownParam])> {
    match name {
        "WndProc" | "WindowProc" | "MainWndProc" | "WndProcA" | "WndProcW" => Some((
            "LRESULT CALLBACK",
            &[("HWND", "hwnd", None), ("UINT", "uMsg", Some("WM")), ("WPARAM", "wParam", None), ("LPARAM", "lParam", None)],
        )),
        "DlgProc" | "DialogProc" => Some((
            "INT_PTR CALLBACK",
            &[("HWND", "hDlg", None), ("UINT", "uMsg", Some("WM")), ("WPARAM", "wParam", None), ("LPARAM", "lParam", None)],
        )),
        "WinMain" | "wWinMain" => Some((
            "int WINAPI",
            &[("HINSTANCE", "hInstance", None), ("HINSTANCE", "hPrevInstance", None), ("LPSTR", "lpCmdLine", None), ("int", "nShowCmd", Some("SW"))],
        )),
        "main" => Some(("int", &[("int", "argc", None), ("char**", "argv", None), ("char**", "envp", None)])),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_strips_suffixes() {
        let a = lookup("USER32.dll!MessageBoxA").unwrap();
        assert_eq!(a.params.len(), 4);
        assert_eq!(a.ret, "int");
        assert!(lookup("puts@plt").is_some());
        assert_eq!(clean_name("_MessageBoxA@16"), "MessageBoxA");
        let c = lookup("CreateWindowExA").unwrap();
        assert_eq!(c.params.len(), 12);
        assert_eq!(c.ret_name.as_deref(), Some("hWnd"));
        assert!(lookup("sleep").unwrap().ret_name.is_none());
    }

    #[test]
    fn constants() {
        assert_eq!(constant("MB", 0x40, None).as_deref(), Some("MB_OK | MB_ICONINFORMATION"));
        assert_eq!(constant("MB", 0x24, None).as_deref(), Some("MB_YESNO | MB_ICONQUESTION"));
        assert_eq!(constant("WS", 0x50000000, Some("BUTTON")).as_deref(), Some("WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON"));
        assert_eq!(constant("WS", 0xcf0000, None).as_deref(), Some("WS_OVERLAPPEDWINDOW"));
        assert_eq!(constant("MF", 0x800, None).as_deref(), Some("MF_SEPARATOR"));
        assert_eq!(constant("MF", 0, None).as_deref(), Some("MF_STRING"));
        assert_eq!(constant("WM", 0x111, None).as_deref(), Some("WM_COMMAND"));
        assert_eq!(constant("CW", 0x80000000u32 as i32 as i64, None).as_deref(), Some("CW_USEDEFAULT"));
        assert_eq!(constant("COLOR1", 6, None).as_deref(), Some("(HBRUSH)(COLOR_WINDOW + 1)"));
        assert_eq!(constant("IDC", 0x7f00, None).as_deref(), Some("IDC_ARROW"));
        assert_eq!(constant("MB", 0x12345678, None), None);
    }

    #[test]
    fn wndclass_layout() {
        let d = struct_def("WNDCLASS*", true).unwrap();
        assert_eq!(d.size, 0x48);
        assert!(d.fields.iter().any(|f| f.name == "lpszClassName" && f.off == 0x40));
        let d32 = struct_def("WNDCLASS*", false).unwrap();
        assert_eq!(d32.size, 0x28);
        let m = struct_def("MSG*", true).unwrap();
        assert!(m.fields.iter().any(|f| f.name == "wParam" && f.off == 0x10));
    }
}
