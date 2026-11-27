#include <windows.h>

#define ID_HELLO 1
#define ID_SAIR  2

LRESULT CALLBACK WndProc(HWND h, UINT msg, WPARAM w, LPARAM l) {
    switch (msg) {
    case WM_CREATE: {
        HMENU bar = CreateMenu(), arq = CreatePopupMenu();
        AppendMenuA(arq, MF_STRING, ID_HELLO, "Hello World");
        AppendMenuA(arq, MF_SEPARATOR, 0, NULL);
        AppendMenuA(arq, MF_STRING, ID_SAIR, "Sair");
        AppendMenuA(bar, MF_POPUP, (UINT_PTR)arq, "Menu");
        SetMenu(h, bar);
        CreateWindowA("BUTTON", "Hello World", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                      110, 60, 160, 40, h, (HMENU)ID_HELLO, NULL, NULL);
        return 0;
    }
    case WM_COMMAND:
        if (LOWORD(w) == ID_HELLO) MessageBoxA(h, "Hello, World!", "Hello", MB_OK | MB_ICONINFORMATION);
        else if (LOWORD(w) == ID_SAIR) DestroyWindow(h);
        return 0;
    case WM_DESTROY:
        PostQuitMessage(0);
        return 0;
    }
    return DefWindowProcA(h, msg, w, l);
}

int WINAPI WinMain(HINSTANCE inst, HINSTANCE prev, LPSTR cmd, int show) {
    WNDCLASSA wc = {0};
    wc.lpfnWndProc = WndProc;
    wc.hInstance = inst;
    wc.hCursor = LoadCursor(NULL, IDC_ARROW);
    wc.hbrBackground = (HBRUSH)(COLOR_WINDOW + 1);
    wc.lpszClassName = "HelloMenu";
    RegisterClassA(&wc);

    HWND h = CreateWindowA("HelloMenu", "Teste - Hello World", WS_OVERLAPPEDWINDOW,
                           CW_USEDEFAULT, CW_USEDEFAULT, 400, 220, NULL, NULL, inst, NULL);
    ShowWindow(h, show);

    MSG m;
    while (GetMessage(&m, NULL, 0, 0) > 0) { TranslateMessage(&m); DispatchMessage(&m); }
    return 0;
}
