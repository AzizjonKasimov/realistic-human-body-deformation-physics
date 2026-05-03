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

enum class ViewMode {
    Normal,
    Anatomy
};

struct AppState {
    int width = 1280;
    int height = 720;
    bool running = true;
    bool pointerDown = false;
    bool debugOverlay = false;
    double accumulator = 0.0;
    double pointerX = 220.0;
    double pointerY = 240.0;
    double previousPointerX = 220.0;
    double previousPointerY = 240.0;
    double pointerVx = 0.0;
    double pointerVy = 0.0;
    double strikerX = 220.0;
    double strikerY = 240.0;
    double strikerVx = 0.0;
    double strikerVy = 0.0;
    double impactPower = 2.0;
    ViewMode viewMode = ViewMode::Anatomy;
    std::chrono::steady_clock::time_point lastTick = std::chrono::steady_clock::now();
    rp::World world = rp::createLayeredBody(1280.0, 720.0);
};

COLORREF color(int r, int g, int b) {
    return RGB(static_cast<unsigned char>(std::clamp(r, 0, 255)),
               static_cast<unsigned char>(std::clamp(g, 0, 255)),
               static_cast<unsigned char>(std::clamp(b, 0, 255)));
}

POINT toPoint(const rp::Point& point) {
    return POINT{static_cast<LONG>(std::lround(point.position.x)), static_cast<LONG>(std::lround(point.position.y))};
}

std::wstring makeTitle(const AppState& app) {
    wchar_t buffer[256];
    std::swprintf(buffer,
                 sizeof(buffer) / sizeof(buffer[0]),
                 L"Realistic Physics C++ | view %s | skin %d | muscle %d | detach %d/%d | bone %d | striker mass %.0fx",
                 app.viewMode == ViewMode::Anatomy ? L"anatomy" : L"normal",
                 app.world.stats().brokenSkin,
                 app.world.stats().brokenMuscle,
                 app.world.stats().brokenAttachments,
                 app.world.stats().brokenBoneAttachments,
                 app.world.stats().fracturedBones,
                 app.impactPower);
    return buffer;
}

void resetStriker(AppState& app) {
    app.strikerX = app.pointerX;
    app.strikerY = app.pointerY;
    app.strikerVx = 0.0;
    app.strikerVy = 0.0;
}

void rebuildWorld(AppState& app) {
    app.world = rp::createLayeredBody(static_cast<double>(app.width), static_cast<double>(app.height));
    resetStriker(app);
}

void advanceStriker(AppState& app, double dt) {
    const double dx = app.pointerX - app.strikerX;
    const double dy = app.pointerY - app.strikerY;
    const double drive = app.pointerDown ? 118.0 : 62.0;
    const double damping = app.pointerDown ? 15.0 : 20.0;

    app.strikerVx += (dx * drive - app.strikerVx * damping) * dt;
    app.strikerVy += (dy * drive - app.strikerVy * damping) * dt;

    const double speed = std::sqrt(app.strikerVx * app.strikerVx + app.strikerVy * app.strikerVy);
    constexpr double maxSpeed = 4200.0;
    if (speed > maxSpeed) {
        const double scale = maxSpeed / speed;
        app.strikerVx *= scale;
        app.strikerVy *= scale;
    }

    app.strikerX += app.strikerVx * dt;
    app.strikerY += app.strikerVy * dt;
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
            advanceStriker(app, fixedDt);
            rp::InputState input;
            input.active = true;
            input.down = app.pointerDown;
            input.x = app.strikerX;
            input.y = app.strikerY;
            input.vx = app.strikerVx;
            input.vy = app.strikerVy;
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

void fillEllipse(HDC dc, int cx, int cy, int radius, COLORREF fill, COLORREF stroke, int strokeWidth) {
    HBRUSH brush = CreateSolidBrush(fill);
    HPEN pen = CreatePen(PS_SOLID, strokeWidth, stroke);
    HGDIOBJ oldBrush = SelectObject(dc, brush);
    HGDIOBJ oldPen = SelectObject(dc, pen);
    Ellipse(dc, cx - radius, cy - radius, cx + radius, cy + radius);
    SelectObject(dc, oldPen);
    SelectObject(dc, oldBrush);
    DeleteObject(pen);
    DeleteObject(brush);
}

void drawFractureCap(HDC dc, const rp::BoneSegment& bone, bool atStart) {
    const rp::Vec2 p = atStart ? bone.a : bone.b;
    const double dx = bone.b.x - bone.a.x;
    const double dy = bone.b.y - bone.a.y;
    const double len = std::max(1.0, std::sqrt(dx * dx + dy * dy));
    const rp::Vec2 storedNormal = atStart ? bone.brokenStartNormal : bone.brokenEndNormal;
    const double storedLen = std::sqrt(storedNormal.x * storedNormal.x + storedNormal.y * storedNormal.y);
    const double nx = storedLen > 0.001 ? storedNormal.x / storedLen : -dy / len;
    const double ny = storedLen > 0.001 ? storedNormal.y / storedLen : dx / len;
    const double dir = atStart ? -1.0 : 1.0;
    const double cap = bone.radius * 1.15;
    const double tx = dx / len * dir;
    const double ty = dy / len * dir;
    const COLORREF dark = color(92, 18, 17);
    const COLORREF fresh = color(214, 42, 34);
    const COLORREF pale = color(246, 238, 212);

    drawLine(dc,
             static_cast<int>(p.x - nx * cap),
             static_cast<int>(p.y - ny * cap),
             static_cast<int>(p.x - nx * cap * 0.28 + tx * bone.radius * 0.45),
             static_cast<int>(p.y - ny * cap * 0.28 + ty * bone.radius * 0.45),
             pale,
             3);
    drawLine(dc,
             static_cast<int>(p.x - nx * cap * 0.28 + tx * bone.radius * 0.45),
             static_cast<int>(p.y - ny * cap * 0.28 + ty * bone.radius * 0.45),
             static_cast<int>(p.x + nx * cap * 0.24 - tx * bone.radius * 0.20),
             static_cast<int>(p.y + ny * cap * 0.24 - ty * bone.radius * 0.20),
             pale,
             3);
    drawLine(dc,
             static_cast<int>(p.x + nx * cap * 0.24 - tx * bone.radius * 0.20),
             static_cast<int>(p.y + ny * cap * 0.24 - ty * bone.radius * 0.20),
             static_cast<int>(p.x + nx * cap),
             static_cast<int>(p.y + ny * cap),
             pale,
             3);
    drawLine(dc,
             static_cast<int>(p.x - nx * cap * 0.65),
             static_cast<int>(p.y - ny * cap * 0.65),
             static_cast<int>(p.x + tx * 7.0),
             static_cast<int>(p.y + ty * 7.0),
             dark,
             2);
    drawLine(dc,
             static_cast<int>(p.x - nx * cap * 0.10),
             static_cast<int>(p.y - ny * cap * 0.10),
             static_cast<int>(p.x + tx * 9.0 + nx * cap * 0.38),
             static_cast<int>(p.y + ty * 9.0 + ny * cap * 0.38),
             fresh,
             2);
    drawLine(dc,
             static_cast<int>(p.x + nx * cap * 0.65),
             static_cast<int>(p.y + ny * cap * 0.65),
             static_cast<int>(p.x + tx * 5.0),
             static_cast<int>(p.y + ty * 5.0),
             dark,
             2);
}

void drawWoundEdge(HDC dc, const rp::Point& a, const rp::Point& b) {
    const double dx = b.position.x - a.position.x;
    const double dy = b.position.y - a.position.y;
    const double len = std::sqrt(dx * dx + dy * dy);
    if (len < 2.0) {
        return;
    }
    const double nx = -dy / len;
    const double ny = dx / len;
    const double mark = std::clamp(len * 0.18, 4.0, 8.0);
    const double inset = std::clamp(len * 0.14, 2.0, 7.0);

    drawLine(dc,
             static_cast<int>(a.position.x + dx / len * inset - nx * mark),
             static_cast<int>(a.position.y + dy / len * inset - ny * mark),
             static_cast<int>(a.position.x + dx / len * inset + nx * mark),
             static_cast<int>(a.position.y + dy / len * inset + ny * mark),
             color(118, 19, 24),
             3);
    drawLine(dc,
             static_cast<int>(b.position.x - dx / len * inset - nx * mark),
             static_cast<int>(b.position.y - dy / len * inset - ny * mark),
             static_cast<int>(b.position.x - dx / len * inset + nx * mark),
             static_cast<int>(b.position.y - dy / len * inset + ny * mark),
             color(118, 19, 24),
             3);
}

void outlineTriangle(HDC dc, const rp::World& world, const rp::Triangle& triangle, COLORREF stroke) {
    const auto& points = world.points();
    const rp::Point& a = points[triangle.a];
    const rp::Point& b = points[triangle.b];
    const rp::Point& c = points[triangle.c];
    drawLine(dc,
             static_cast<int>(a.position.x),
             static_cast<int>(a.position.y),
             static_cast<int>(b.position.x),
             static_cast<int>(b.position.y),
             stroke,
             1);
    drawLine(dc,
             static_cast<int>(b.position.x),
             static_cast<int>(b.position.y),
             static_cast<int>(c.position.x),
             static_cast<int>(c.position.y),
             stroke,
             1);
    drawLine(dc,
             static_cast<int>(c.position.x),
             static_cast<int>(c.position.y),
             static_cast<int>(a.position.x),
             static_cast<int>(a.position.y),
             stroke,
             1);
}

void drawBone(HDC dc, const rp::BoneSegment& bone) {
    const COLORREF stroke = bone.fractured ? color(235, 235, 222) : color(214, 202, 172);
    const int width = std::max(3, static_cast<int>(std::lround(bone.radius * 1.7)));
    drawLine(dc,
             static_cast<int>(bone.a.x),
             static_cast<int>(bone.a.y),
             static_cast<int>(bone.b.x),
             static_cast<int>(bone.b.y),
             stroke,
             width);
    if (bone.brokenStart) {
        drawFractureCap(dc, bone, true);
    }
    if (bone.brokenEnd) {
        drawFractureCap(dc, bone, false);
    }
}

void drawStriker(HDC dc, const AppState& app) {
    const double speed = std::sqrt(app.strikerVx * app.strikerVx + app.strikerVy * app.strikerVy);
    double dirX = 1.0;
    double dirY = 0.0;
    if (speed > 1.0) {
        dirX = app.strikerVx / speed;
        dirY = app.strikerVy / speed;
    } else {
        const double targetDx = app.strikerX - app.pointerX;
        const double targetDy = app.strikerY - app.pointerY;
        const double targetDistance = std::sqrt(targetDx * targetDx + targetDy * targetDy);
        if (targetDistance > 1.0) {
            dirX = targetDx / targetDistance;
            dirY = targetDy / targetDistance;
        }
    }

    const int radius = static_cast<int>(app.world.materials().strikerRadius);
    const int cx = static_cast<int>(std::lround(app.strikerX));
    const int cy = static_cast<int>(std::lround(app.strikerY));
    const double targetDx = app.pointerX - app.strikerX;
    const double targetDy = app.pointerY - app.strikerY;
    const double targetDistance = std::sqrt(targetDx * targetDx + targetDy * targetDy);
    int handleStartX = static_cast<int>(std::lround(app.strikerX - dirX * (radius + 58.0)));
    int handleStartY = static_cast<int>(std::lround(app.strikerY - dirY * (radius + 58.0)));
    int handleEndX = static_cast<int>(std::lround(app.strikerX - dirX * (radius * 0.55)));
    int handleEndY = static_cast<int>(std::lround(app.strikerY - dirY * (radius * 0.55)));
    if (targetDistance > radius * 0.65) {
        handleStartX = static_cast<int>(std::lround(app.pointerX));
        handleStartY = static_cast<int>(std::lround(app.pointerY));
        handleEndX = static_cast<int>(std::lround(app.strikerX + targetDx / targetDistance * radius * 0.72));
        handleEndY = static_cast<int>(std::lround(app.strikerY + targetDy / targetDistance * radius * 0.72));
    }

    drawLine(dc, handleStartX, handleStartY, handleEndX, handleEndY, color(72, 66, 57), 10);
    drawLine(dc, handleStartX, handleStartY, handleEndX, handleEndY, color(154, 136, 102), 4);
    fillEllipse(dc,
                static_cast<int>(std::lround(app.pointerX)),
                static_cast<int>(std::lround(app.pointerY)),
                app.pointerDown ? 5 : 4,
                app.pointerDown ? color(255, 205, 83) : color(130, 119, 96),
                color(40, 35, 28),
                1);

    if (app.pointerDown && speed > 80.0) {
        const double arrowLength = std::clamp(speed * 0.030, 18.0, 82.0);
        const int ax = static_cast<int>(std::lround(app.strikerX + dirX * (radius + arrowLength)));
        const int ay = static_cast<int>(std::lround(app.strikerY + dirY * (radius + arrowLength)));
        const int sx = static_cast<int>(std::lround(app.strikerX + dirX * radius * 0.35));
        const int sy = static_cast<int>(std::lround(app.strikerY + dirY * radius * 0.35));
        drawLine(dc, sx, sy, ax, ay, color(255, 205, 83), 3);
        const double nx = -dirY;
        const double ny = dirX;
        POINT head[3] = {
            POINT{ax, ay},
            POINT{static_cast<LONG>(std::lround(ax - dirX * 12.0 + nx * 6.0)), static_cast<LONG>(std::lround(ay - dirY * 12.0 + ny * 6.0))},
            POINT{static_cast<LONG>(std::lround(ax - dirX * 12.0 - nx * 6.0)), static_cast<LONG>(std::lround(ay - dirY * 12.0 - ny * 6.0))},
        };
        HBRUSH arrowBrush = CreateSolidBrush(color(255, 205, 83));
        HPEN arrowPen = CreatePen(PS_SOLID, 1, color(100, 72, 24));
        HGDIOBJ oldBrush = SelectObject(dc, arrowBrush);
        HGDIOBJ oldPen = SelectObject(dc, arrowPen);
        Polygon(dc, head, 3);
        SelectObject(dc, oldPen);
        SelectObject(dc, oldBrush);
        DeleteObject(arrowPen);
        DeleteObject(arrowBrush);
    }

    fillEllipse(dc, cx, cy, radius + 4, color(35, 32, 29), color(18, 16, 15), 2);
    fillEllipse(dc,
                cx,
                cy,
                radius,
                app.pointerDown ? color(190, 63, 48) : color(194, 174, 121),
                color(42, 30, 22),
                3);
    fillEllipse(dc,
                static_cast<int>(std::lround(app.strikerX - dirX * radius * 0.18 - dirY * radius * 0.20)),
                static_cast<int>(std::lround(app.strikerY - dirY * radius * 0.18 + dirX * radius * 0.20)),
                std::max(5, radius / 4),
                color(235, 218, 163),
                color(96, 76, 42),
                1);
}

void drawDebugText(HDC dc, int x, int y, const wchar_t* text) {
    TextOutW(dc, x, y, text, lstrlenW(text));
}

void drawDebugOverlay(HDC dc, const AppState& app) {
    RECT panel{12, 42, 430, 180};
    HBRUSH brush = CreateSolidBrush(color(22, 24, 27));
    HPEN pen = CreatePen(PS_SOLID, 1, color(98, 105, 112));
    HGDIOBJ oldBrush = SelectObject(dc, brush);
    HGDIOBJ oldPen = SelectObject(dc, pen);
    Rectangle(dc, panel.left, panel.top, panel.right, panel.bottom);
    SelectObject(dc, oldPen);
    SelectObject(dc, oldBrush);
    DeleteObject(pen);
    DeleteObject(brush);

    SetBkMode(dc, TRANSPARENT);
    SetTextColor(dc, color(226, 224, 215));

    const rp::ContactDebug& debug = app.world.debug();
    wchar_t line[256];
    int y = panel.top + 10;
    drawDebugText(dc, panel.left + 12, y, L"Contact debug (D): striker is a spring-driven blunt mass");
    y += 20;
    std::swprintf(line,
                  sizeof(line) / sizeof(line[0]),
                  L"head=(%.0f, %.0f) target=(%.0f, %.0f) radius=%.0f",
                  debug.strikerPosition.x,
                  debug.strikerPosition.y,
                  app.pointerX,
                  app.pointerY,
                  debug.strikerRadius);
    drawDebugText(dc, panel.left + 12, y, line);
    y += 20;
    std::swprintf(line,
                  sizeof(line) / sizeof(line[0]),
                  L"speed=%.0f px/s  mass=%.1f  impact=%.0f  down=%s",
                  debug.strikerSpeed,
                  debug.strikerMass,
                  debug.impact,
                  debug.down ? L"yes" : L"no");
    drawDebugText(dc, panel.left + 12, y, line);
    y += 20;
    std::swprintf(line,
                  sizeof(line) / sizeof(line[0]),
                  L"contacts: tissue=%d bone=%d  max depth=%.1f",
                  debug.tissueContacts,
                  debug.boneContacts,
                  debug.maxDepth);
    drawDebugText(dc, panel.left + 12, y, line);
    y += 20;
    std::swprintf(line,
                  sizeof(line) / sizeof(line[0]),
                  L"loads: tissue=%.0f bone=%.0f  fracture impulse=%.0f",
                  debug.maxPointLoad,
                  debug.maxBoneLoad,
                  debug.lastFractureImpulse);
    drawDebugText(dc, panel.left + 12, y, line);
    y += 20;
    std::swprintf(line,
                  sizeof(line) / sizeof(line[0]),
                  L"totals: skin=%d muscle=%d attach=%d/%d joints=%d fractures=%d step=%d",
                  app.world.stats().brokenSkin,
                  app.world.stats().brokenMuscle,
                  app.world.stats().brokenAttachments,
                  app.world.stats().brokenBoneAttachments,
                  app.world.stats().brokenBoneJoints,
                  app.world.stats().fracturedBones,
                  debug.fractures);
    drawDebugText(dc, panel.left + 12, y, line);

    if (debug.maxDepth > 0.0) {
        fillEllipse(dc,
                    static_cast<int>(std::lround(debug.strongestContact.x)),
                    static_cast<int>(std::lround(debug.strongestContact.y)),
                    5,
                    color(255, 215, 90),
                    color(98, 65, 20),
                    1);
    }
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

    if (app.viewMode == ViewMode::Normal) {
        for (const rp::BoneSegment& bone : world.bones()) {
            drawBone(dc, bone);
        }
    }

    for (const rp::Triangle& triangle : world.triangles()) {
        if (!world.triangleAlive(triangle)) {
            continue;
        }
        if (triangle.layer == rp::TissueLayer::Muscle) {
            const auto& points = world.points();
            const double exposure = (points[triangle.a].exposure + points[triangle.b].exposure + points[triangle.c].exposure) / 3.0;
            if (app.viewMode == ViewMode::Normal && exposure < 0.04) {
                continue;
            }
            const int red = static_cast<int>(std::clamp(105.0 + exposure * 58.0 + triangle.damage * 35.0, 0.0, 255.0));
            fillPolygon(dc, world, triangle, color(red, 30, 38), color(95, 35, 40));
        }
    }

    for (const rp::Triangle& triangle : world.triangles()) {
        if (triangle.layer != rp::TissueLayer::Skin || !world.triangleAlive(triangle)) {
            continue;
        }
        const auto& points = world.points();
        const double load = (points[triangle.a].load + points[triangle.b].load + points[triangle.c].load) / 3.0;
        const int heat = static_cast<int>(std::clamp(load / 18.0, 0.0, 70.0));
        if (app.viewMode == ViewMode::Anatomy) {
            outlineTriangle(dc, world, triangle, color(104 + heat, 70, 62));
        } else {
            fillPolygon(dc, world, triangle, color(155 + heat, 112 - heat / 3, 94 - heat / 4), color(76, 48, 43));
        }
    }

    if (app.viewMode == ViewMode::Anatomy) {
        for (const rp::BoneSegment& bone : world.bones()) {
            drawBone(dc, bone);
        }
    }

    if (app.viewMode == ViewMode::Normal) {
        for (const rp::Spring& spring : world.springs()) {
            if (!spring.broken || spring.layer != rp::TissueLayer::Skin) {
                continue;
            }
            const rp::Point& a = world.points()[spring.a];
            const rp::Point& b = world.points()[spring.b];
            drawWoundEdge(dc, a, b);
        }
    }

    drawStriker(dc, app);
    if (app.debugOverlay) {
        drawDebugOverlay(dc, app);
    }

    SetBkMode(dc, TRANSPARENT);
    SetTextColor(dc, color(220, 216, 205));
    constexpr const wchar_t* instructions = L"Left drag: swing blunt striker | D: debug | Tab: anatomy | R: reset | Space: pause | 1/2/4: mass";
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
        app->width = std::max(1, static_cast<int>(LOWORD(lParam)));
        app->height = std::max(1, static_cast<int>(HIWORD(lParam)));
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
        } else if (wParam == VK_TAB) {
            app->viewMode = app->viewMode == ViewMode::Anatomy ? ViewMode::Normal : ViewMode::Anatomy;
        } else if (wParam == 'D') {
            app->debugOverlay = !app->debugOverlay;
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
