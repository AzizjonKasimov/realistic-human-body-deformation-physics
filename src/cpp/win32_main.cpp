#define NOMINMAX
#include <windows.h>

#include "simulation.hpp"

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstdio>
#include <memory>
#include <string>

namespace {

constexpr int kTimerId = 1;
constexpr int kTargetFrameMs = 16;

struct AppState {
    int width = 1280;
    int height = 720;
    bool running = true;
    bool pointerDown = false;
    double accumulator = 0.0;
    double pointerX = 220.0;
    double pointerY = 240.0;
    double previousPointerX = 220.0;
    double previousPointerY = 240.0;
    double pointerVx = 0.0;
    double pointerVy = 0.0;
    double impactPower = 2.0;
    std::chrono::steady_clock::time_point lastTick = std::chrono::steady_clock::now();
    rp::World world = rp::createLayeredBody(1280.0, 720.0);
};

COLORREF color(unsigned char r, unsigned char g, unsigned char b) {
    return RGB(r, g, b);
}

POINT toPoint(const rp::Point& point) {
    return POINT{static_cast<LONG>(std::lround(point.position.x)), static_cast<LONG>(std::lround(point.position.y))};
}

std::wstring makeTitle(const AppState& app) {
    wchar_t buffer[256];
    std::swprintf(buffer,
                 sizeof(buffer) / sizeof(buffer[0]),
                 L"Realistic Physics C++ | skin %d | muscle %d | detach %d | impact %.0fx",
                 app.world.stats().brokenSkin,
                 app.world.stats().brokenMuscle,
                 app.world.stats().brokenAttachments,
                 app.impactPower);
    return buffer;
}

void rebuildWorld(AppState& app) {
    app.world = rp::createLayeredBody(static_cast<double>(app.width), static_cast<double>(app.height));
}

void stepSimulation(AppState& app, HWND hwnd) {
    const auto now = std::chrono::steady_clock::now();
    const double frameDt = std::min(0.05, std::chrono::duration<double>(now - app.lastTick).count());
    app.lastTick = now;

    app.pointerVx = (app.pointerX - app.previousPointerX) / std::max(frameDt, 0.001);
    app.pointerVy = (app.pointerY - app.previousPointerY) / std::max(frameDt, 0.001);
    app.previousPointerX = app.pointerX;
    app.previousPointerY = app.pointerY;

    if (app.running) {
        app.accumulator += frameDt;
        const double fixedDt = app.world.materials().fixedDt;
        while (app.accumulator >= fixedDt) {
            rp::InputState input;
            input.active = true;
            input.down = app.pointerDown;
            input.x = app.pointerX;
            input.y = app.pointerY;
            input.vx = app.pointerVx;
            input.vy = app.pointerVy;
            input.power = app.impactPower;
            app.world.step(fixedDt, input, static_cast<double>(app.width), static_cast<double>(app.height));
            app.accumulator -= fixedDt;
        }
    }

    const std::wstring title = makeTitle(app);
    SetWindowTextW(hwnd, title.c_str());
}

void fillPolygon(HDC dc, const rp::World& world, const rp::Triangle& triangle, COLORREF fill, COLORREF outline) {
    const auto& points = world.points();
    POINT polygon[3] = {
        toPoint(points[triangle.a]),
        toPoint(points[triangle.b]),
        toPoint(points[triangle.c]),
    };

    HBRUSH brush = CreateSolidBrush(fill);
    HPEN pen = CreatePen(PS_SOLID, 1, outline);
    HGDIOBJ oldBrush = SelectObject(dc, brush);
    HGDIOBJ oldPen = SelectObject(dc, pen);
    Polygon(dc, polygon, 3);
    SelectObject(dc, oldPen);
    SelectObject(dc, oldBrush);
    DeleteObject(pen);
    DeleteObject(brush);
}

void drawLine(HDC dc, int x1, int y1, int x2, int y2, COLORREF stroke, int width) {
    HPEN pen = CreatePen(PS_SOLID, width, stroke);
    HGDIOBJ oldPen = SelectObject(dc, pen);
    MoveToEx(dc, x1, y1, nullptr);
    LineTo(dc, x2, y2);
    SelectObject(dc, oldPen);
    DeleteObject(pen);
}

void drawScene(HDC dc, const AppState& app) {
    RECT background{0, 0, app.width, app.height};
    HBRUSH bgBrush = CreateSolidBrush(color(24, 24, 24));
    FillRect(dc, &background, bgBrush);
    DeleteObject(bgBrush);

    RECT floor{0, app.height - 38, app.width, app.height};
    HBRUSH floorBrush = CreateSolidBrush(color(36, 33, 29));
    FillRect(dc, &floor, floorBrush);
    DeleteObject(floorBrush);
    drawLine(dc, 0, app.height - 38, app.width, app.height - 38, color(75, 69, 61), 1);

    const rp::World& world = app.world;
    for (const rp::Triangle& triangle : world.triangles()) {
        if (!world.triangleAlive(triangle)) {
            continue;
        }
        if (triangle.layer == rp::TissueLayer::Muscle) {
            const auto& points = world.points();
            const double exposure = (points[triangle.a].exposure + points[triangle.b].exposure + points[triangle.c].exposure) / 3.0;
            if (exposure < 0.04) {
                continue;
            }
            const int red = static_cast<int>(std::clamp(95.0 + exposure * 60.0 + triangle.damage * 35.0, 0.0, 255.0));
            fillPolygon(dc, world, triangle, color(static_cast<unsigned char>(red), 30, 38), color(95, 35, 40));
        }
    }

    for (const rp::Triangle& triangle : world.triangles()) {
        if (triangle.layer != rp::TissueLayer::Skin || !world.triangleAlive(triangle)) {
            continue;
        }
        const auto& points = world.points();
        const double load = (points[triangle.a].load + points[triangle.b].load + points[triangle.c].load) / 3.0;
        const int heat = static_cast<int>(std::clamp(load / 18.0, 0.0, 70.0));
        fillPolygon(dc, world, triangle, color(155 + heat, 112 - heat / 3, 94 - heat / 4), color(76, 48, 43));
    }

    for (const rp::Spring& spring : world.springs()) {
        if (!spring.broken || spring.layer != rp::TissueLayer::Skin) {
            continue;
        }
        const rp::Point& a = world.points()[spring.a];
        const rp::Point& b = world.points()[spring.b];
        drawLine(dc,
                 static_cast<int>(a.position.x),
                 static_cast<int>(a.position.y),
                 static_cast<int>(b.position.x),
                 static_cast<int>(b.position.y),
                 color(225, 60, 52),
                 2);
    }

    HBRUSH strikerBrush = CreateSolidBrush(app.pointerDown ? color(230, 83, 58) : color(225, 183, 75));
    HPEN strikerPen = CreatePen(PS_SOLID, 2, color(42, 30, 22));
    HGDIOBJ oldBrush = SelectObject(dc, strikerBrush);
    HGDIOBJ oldPen = SelectObject(dc, strikerPen);
    const int radius = static_cast<int>(app.world.materials().strikerRadius);
    Ellipse(dc,
            static_cast<int>(app.pointerX) - radius,
            static_cast<int>(app.pointerY) - radius,
            static_cast<int>(app.pointerX) + radius,
            static_cast<int>(app.pointerY) + radius);
    SelectObject(dc, oldPen);
    SelectObject(dc, oldBrush);
    DeleteObject(strikerPen);
    DeleteObject(strikerBrush);

    SetBkMode(dc, TRANSPARENT);
    SetTextColor(dc, color(220, 216, 205));
    constexpr const wchar_t* instructions = L"Left drag: strike | R: reset | Space: pause | 1/2/4: impact";
    TextOutW(dc, 16, 16, instructions, lstrlenW(instructions));
}

void paint(HWND hwnd, AppState& app) {
    PAINTSTRUCT ps;
    HDC windowDc = BeginPaint(hwnd, &ps);
    HDC memoryDc = CreateCompatibleDC(windowDc);
    HBITMAP bitmap = CreateCompatibleBitmap(windowDc, app.width, app.height);
    HGDIOBJ oldBitmap = SelectObject(memoryDc, bitmap);

    drawScene(memoryDc, app);
    BitBlt(windowDc, 0, 0, app.width, app.height, memoryDc, 0, 0, SRCCOPY);

    SelectObject(memoryDc, oldBitmap);
    DeleteObject(bitmap);
    DeleteDC(memoryDc);
    EndPaint(hwnd, &ps);
}

AppState* appFrom(HWND hwnd) {
    return reinterpret_cast<AppState*>(GetWindowLongPtrW(hwnd, GWLP_USERDATA));
}

LRESULT CALLBACK windowProc(HWND hwnd, UINT message, WPARAM wParam, LPARAM lParam) {
    switch (message) {
    case WM_CREATE: {
        auto app = std::make_unique<AppState>();
        RECT client;
        GetClientRect(hwnd, &client);
        app->width = std::max(1L, client.right - client.left);
        app->height = std::max(1L, client.bottom - client.top);
        rebuildWorld(*app);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(app.release()));
        SetTimer(hwnd, kTimerId, kTargetFrameMs, nullptr);
        return 0;
    }
    case WM_DESTROY: {
        KillTimer(hwnd, kTimerId);
        delete appFrom(hwnd);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        PostQuitMessage(0);
        return 0;
    }
    case WM_SIZE: {
        AppState* app = appFrom(hwnd);
        if (!app) {
            return 0;
        }
        app->width = std::max(1, LOWORD(lParam));
        app->height = std::max(1, HIWORD(lParam));
        rebuildWorld(*app);
        InvalidateRect(hwnd, nullptr, FALSE);
        return 0;
    }
    case WM_TIMER: {
        AppState* app = appFrom(hwnd);
        if (!app || wParam != kTimerId) {
            return 0;
        }
        stepSimulation(*app, hwnd);
        InvalidateRect(hwnd, nullptr, FALSE);
        return 0;
    }
    case WM_MOUSEMOVE:
    case WM_LBUTTONDOWN:
    case WM_LBUTTONUP: {
        AppState* app = appFrom(hwnd);
        if (!app) {
            return 0;
        }
        app->pointerX = static_cast<short>(LOWORD(lParam));
        app->pointerY = static_cast<short>(HIWORD(lParam));
        if (message == WM_LBUTTONDOWN) {
            app->pointerDown = true;
            SetCapture(hwnd);
        } else if (message == WM_LBUTTONUP) {
            app->pointerDown = false;
            ReleaseCapture();
        }
        return 0;
    }
    case WM_KEYDOWN: {
        AppState* app = appFrom(hwnd);
        if (!app) {
            return 0;
        }
        if (wParam == 'R') {
            rebuildWorld(*app);
        } else if (wParam == VK_SPACE) {
            app->running = !app->running;
        } else if (wParam == '1') {
            app->impactPower = 1.0;
        } else if (wParam == '2') {
            app->impactPower = 2.0;
        } else if (wParam == '4') {
            app->impactPower = 4.0;
        }
        return 0;
    }
    case WM_PAINT: {
        AppState* app = appFrom(hwnd);
        if (!app) {
            return DefWindowProcW(hwnd, message, wParam, lParam);
        }
        paint(hwnd, *app);
        return 0;
    }
    default:
        return DefWindowProcW(hwnd, message, wParam, lParam);
    }
}

} // namespace

int WINAPI wWinMain(HINSTANCE instance, HINSTANCE, PWSTR, int showCommand) {
    const wchar_t className[] = L"RealisticPhysicsWindow";

    WNDCLASSW wc{};
    wc.lpfnWndProc = windowProc;
    wc.hInstance = instance;
    wc.lpszClassName = className;
    wc.hCursor = LoadCursor(nullptr, IDC_ARROW);
    wc.hbrBackground = reinterpret_cast<HBRUSH>(COLOR_WINDOW + 1);

    RegisterClassW(&wc);

    HWND hwnd = CreateWindowExW(0,
                                className,
                                L"Realistic Physics C++",
                                WS_OVERLAPPEDWINDOW,
                                CW_USEDEFAULT,
                                CW_USEDEFAULT,
                                1280,
                                720,
                                nullptr,
                                nullptr,
                                instance,
                                nullptr);
    if (!hwnd) {
        return 1;
    }

    ShowWindow(hwnd, showCommand);

    MSG message{};
    while (GetMessageW(&message, nullptr, 0, 0)) {
        TranslateMessage(&message);
        DispatchMessageW(&message);
    }

    return 0;
}
