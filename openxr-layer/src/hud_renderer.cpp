#include "hud_renderer.h"

#include <dxgi.h>

#include <cmath>
#include <cstdio>
#include <string>

using Microsoft::WRL::ComPtr;

namespace {

// Race Refinery HUD palette (matches the ?layout=ironman CSS in hud_server.rs).
const D2D1_COLOR_F kGlow = {0.36f, 1.0f, 0.66f, 1.0f};      // #5dffa8
const D2D1_COLOR_F kGlowDim = {0.36f, 1.0f, 0.66f, 0.6f};
const D2D1_COLOR_F kHero = {0.91f, 1.0f, 0.95f, 1.0f};      // #e8fff3
const D2D1_COLOR_F kFast = {0.36f, 1.0f, 0.66f, 1.0f};
const D2D1_COLOR_F kSlow = {1.0f, 0.42f, 0.42f, 1.0f};      // #ff6b6b
const D2D1_COLOR_F kWarn = {1.0f, 0.70f, 0.28f, 1.0f};      // #ffb347

bool IsNone(float v) { return std::isnan(v); }

std::wstring FormatLap(float ms) {
    if (IsNone(ms) || ms <= 0.0f) return L"\u2014";
    const double s = ms / 1000.0;
    const int m = static_cast<int>(s / 60.0);
    const double sec = s - m * 60.0;
    wchar_t buf[32];
    if (m > 0) {
        swprintf(buf, 32, L"%d:%06.3f", m, sec);
    } else {
        swprintf(buf, 32, L"%.3f", sec);
    }
    return buf;
}

std::wstring FormatDelta(float ms) {
    if (IsNone(ms)) return L"\u2014";
    wchar_t buf[32];
    swprintf(buf, 32, L"%+.3f", ms / 1000.0f);
    return buf;
}

std::wstring FormatGap(float s) {
    if (IsNone(s)) return L"\u2014";
    wchar_t buf[32];
    swprintf(buf, 32, L"%.1fs", std::fabs(s));
    return buf;
}

const wchar_t* PackLabel(uint32_t state) {
    switch (state) {
        case RR_PACK_CLEAR: return L"CLEAR";
        case RR_PACK_CAR_LEFT: return L"\u25C0 CAR";
        case RR_PACK_CAR_RIGHT: return L"CAR \u25B6";
        case RR_PACK_THREE_WIDE: return L"3-WIDE";
        case RR_PACK_TWO_LEFT: return L"2 LEFT";
        case RR_PACK_TWO_RIGHT: return L"2 RIGHT";
        default: return L"";
    }
}

D2D1_COLOR_F DeltaColor(float ms) {
    if (IsNone(ms)) return kGlowDim;
    return ms > 0.0f ? kSlow : kFast;
}

// Track-map vertices are normalized 0..1; letterbox them into the render target
// so the circuit keeps its aspect ratio on a non-square overlay.
D2D1_POINT_2F TrackMapPixel(const RrTrackMapPoint& p, float w, float h) {
    const float side = w < h ? w : h;
    return {(w - side) * 0.5f + p.x * side, (h - side) * 0.5f + p.y * side};
}

// Interpolate a lap fraction onto the outline, treating the gap between the last
// and first vertex as the closing segment. Mirrors `track_map::point_at`.
D2D1_POINT_2F TrackMapPointAt(const RrTrackMap& map, float pct, float w, float h) {
    const uint32_t count = map.point_count;
    float t = std::fmod(pct, 1.0f);
    if (t < 0.0f) t += 1.0f;

    const RrTrackMapPoint& first = map.points[0];
    const RrTrackMapPoint& last = map.points[count - 1];
    if (t <= first.pct || t >= last.pct) {
        const float span = 1.0f - last.pct + first.pct;
        if (span <= 0.0f) return TrackMapPixel(first, w, h);
        const float travelled = t >= last.pct ? t - last.pct : 1.0f - last.pct + t;
        const float u = travelled / span;
        const RrTrackMapPoint lerped = {last.x + (first.x - last.x) * u,
                                        last.y + (first.y - last.y) * u, t};
        return TrackMapPixel(lerped, w, h);
    }

    uint32_t hi = 1;
    while (hi < count && map.points[hi].pct <= t) ++hi;
    if (hi >= count) hi = count - 1;
    const RrTrackMapPoint& a = map.points[hi - 1];
    const RrTrackMapPoint& b = map.points[hi];
    const float span = b.pct - a.pct;
    const float u = span > 0.0f ? (t - a.pct) / span : 0.0f;
    const RrTrackMapPoint lerped = {a.x + (b.x - a.x) * u, a.y + (b.y - a.y) * u, t};
    return TrackMapPixel(lerped, w, h);
}

}  // namespace

bool HudRenderer::InitD2D(ID3D11Device* device) {
    D2D1_FACTORY_OPTIONS opts{};
    if (FAILED(D2D1CreateFactory(D2D1_FACTORY_TYPE_MULTI_THREADED, __uuidof(ID2D1Factory1),
                                 &opts,
                                 reinterpret_cast<void**>(m_d2dFactory.ReleaseAndGetAddressOf())))) {
        return false;
    }
    if (FAILED(DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED, __uuidof(IDWriteFactory),
                                   reinterpret_cast<IUnknown**>(
                                       m_dwriteFactory.ReleaseAndGetAddressOf())))) {
        return false;
    }

    ComPtr<ID3D10Multithread> mt;
    if (SUCCEEDED(device->QueryInterface(__uuidof(ID3D10Multithread),
                                         reinterpret_cast<void**>(mt.GetAddressOf())))) {
        mt->SetMultithreadProtected(TRUE);
    }

    ComPtr<IDXGIDevice> dxgiDevice;
    if (FAILED(device->QueryInterface(__uuidof(IDXGIDevice),
                                      reinterpret_cast<void**>(dxgiDevice.GetAddressOf())))) {
        return false;
    }
    if (FAILED(m_d2dFactory->CreateDevice(dxgiDevice.Get(),
                                          m_d2dDevice.ReleaseAndGetAddressOf()))) {
        return false;
    }
    if (FAILED(m_d2dDevice->CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
                                                m_d2dContext.ReleaseAndGetAddressOf()))) {
        return false;
    }

    const wchar_t* kFont = L"Consolas";
    m_dwriteFactory->CreateTextFormat(kFont, nullptr, DWRITE_FONT_WEIGHT_BOLD,
                                      DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL,
                                      96.0f, L"en-us", m_hero.ReleaseAndGetAddressOf());
    m_dwriteFactory->CreateTextFormat(kFont, nullptr, DWRITE_FONT_WEIGHT_NORMAL,
                                      DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL,
                                      20.0f, L"en-us", m_label.ReleaseAndGetAddressOf());
    m_dwriteFactory->CreateTextFormat(kFont, nullptr, DWRITE_FONT_WEIGHT_BOLD,
                                      DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL,
                                      30.0f, L"en-us", m_value.ReleaseAndGetAddressOf());
    m_dwriteFactory->CreateTextFormat(kFont, nullptr, DWRITE_FONT_WEIGHT_BOLD,
                                      DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL,
                                      26.0f, L"en-us", m_badge.ReleaseAndGetAddressOf());
    return true;
}

bool HudRenderer::Initialize(ID3D11Device* appDevice) {
    if (m_ready) {
        return true;
    }
    if (!appDevice) {
        return false;
    }

    m_appDevice = appDevice;
    appDevice->GetImmediateContext(m_appCtx.ReleaseAndGetAddressOf());
    if (!m_appCtx) {
        return false;
    }

    // Fast path: D2D on the app device (needs BGRA support).
    if (InitD2D(appDevice)) {
        m_drawDevice = appDevice;
        m_drawCtx = m_appCtx;
        m_directToSwapchain = true;
        m_ready = true;
        return true;
    }

    // Fallback: private BGRA device on the same adapter + shared-texture copy.
    m_d2dFactory.Reset();
    m_dwriteFactory.Reset();
    m_d2dDevice.Reset();
    m_d2dContext.Reset();

    ComPtr<IDXGIDevice> appDxgi;
    if (FAILED(appDevice->QueryInterface(__uuidof(IDXGIDevice),
                                         reinterpret_cast<void**>(appDxgi.GetAddressOf())))) {
        return false;
    }
    ComPtr<IDXGIAdapter> adapter;
    if (FAILED(appDxgi->GetAdapter(adapter.GetAddressOf()))) {
        return false;
    }

    static const D3D_FEATURE_LEVEL kLevels[] = {
        D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_10_1,
        D3D_FEATURE_LEVEL_10_0};
    D3D_FEATURE_LEVEL got = D3D_FEATURE_LEVEL_11_0;
    const UINT flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_SINGLETHREADED;
    if (FAILED(D3D11CreateDevice(adapter.Get(), D3D_DRIVER_TYPE_UNKNOWN, nullptr, flags, kLevels,
                                 ARRAYSIZE(kLevels), D3D11_SDK_VERSION,
                                 m_drawDevice.ReleaseAndGetAddressOf(), &got,
                                 m_drawCtx.ReleaseAndGetAddressOf()))) {
        // Retry without 11_1 if the adapter rejects the level list mid-way.
        if (FAILED(D3D11CreateDevice(adapter.Get(), D3D_DRIVER_TYPE_UNKNOWN, nullptr, flags,
                                     nullptr, 0, D3D11_SDK_VERSION,
                                     m_drawDevice.ReleaseAndGetAddressOf(), &got,
                                     m_drawCtx.ReleaseAndGetAddressOf()))) {
            return false;
        }
    }

    if (!InitD2D(m_drawDevice.Get())) {
        return false;
    }
    m_directToSwapchain = false;
    m_ready = true;
    return true;
}

bool HudRenderer::EnsureDrawTexture(uint32_t width, uint32_t height) {
    if (m_directToSwapchain) {
        return true;
    }
    if (m_drawTex && m_texW == width && m_texH == height) {
        return true;
    }

    m_drawTex.Reset();
    m_appSharedTex.Reset();
    m_texW = width;
    m_texH = height;

    D3D11_TEXTURE2D_DESC desc{};
    desc.Width = width;
    desc.Height = height;
    desc.MipLevels = 1;
    desc.ArraySize = 1;
    desc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    desc.SampleDesc.Count = 1;
    desc.Usage = D3D11_USAGE_DEFAULT;
    desc.BindFlags = D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE;
    desc.MiscFlags = D3D11_RESOURCE_MISC_SHARED;

    if (FAILED(m_drawDevice->CreateTexture2D(&desc, nullptr, m_drawTex.GetAddressOf()))) {
        return false;
    }

    ComPtr<IDXGIResource> dxgiRes;
    if (FAILED(m_drawTex.As(&dxgiRes))) {
        return false;
    }
    HANDLE shared = nullptr;
    if (FAILED(dxgiRes->GetSharedHandle(&shared)) || !shared) {
        return false;
    }
    if (FAILED(m_appDevice->OpenSharedResource(shared, __uuidof(ID3D11Texture2D),
                                               reinterpret_cast<void**>(
                                                   m_appSharedTex.GetAddressOf())))) {
        return false;
    }
    return true;
}

bool HudRenderer::DrawToSurface(IDXGISurface* surface, const RrOverlay& overlay,
                                const RrSnapshot& snapshot, float opacity) {
    D2D1_BITMAP_PROPERTIES1 props = D2D1::BitmapProperties1(
        D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_PREMULTIPLIED));
    ComPtr<ID2D1Bitmap1> bitmap;
    if (FAILED(m_d2dContext->CreateBitmapFromDxgiSurface(surface, &props,
                                                         bitmap.GetAddressOf()))) {
        return false;
    }

    D2D1_SIZE_F size = bitmap->GetSize();
    m_d2dContext->SetTarget(bitmap.Get());
    m_d2dContext->BeginDraw();
    m_d2dContext->Clear(D2D1::ColorF(0, 0, 0, 0));
    m_d2dContext->SetTransform(D2D1::Matrix3x2F::Identity());
    (void)opacity;

    switch (overlay.kind) {
        case RR_OVERLAY_STANDINGS: DrawStandings(snapshot, size.width, size.height); break;
        case RR_OVERLAY_RELATIVE: DrawRelative(snapshot, size.width, size.height); break;
        case RR_OVERLAY_RADAR: DrawRadar(snapshot, size.width, size.height); break;
        case RR_OVERLAY_TRACKMAP: DrawTrackMap(snapshot, size.width, size.height); break;
        case RR_OVERLAY_COACH:
        default: DrawCoach(snapshot, size.width, size.height); break;
    }

    const HRESULT hr = m_d2dContext->EndDraw();
    m_d2dContext->SetTarget(nullptr);
    return SUCCEEDED(hr);
}

void HudRenderer::DrawText(const wchar_t* text, IDWriteTextFormat* fmt, D2D1_RECT_F rect,
                           D2D1_COLOR_F color) {
    if (!text || !*text || !fmt) {
        return;
    }
    ComPtr<ID2D1SolidColorBrush> brush;
    if (FAILED(m_d2dContext->CreateSolidColorBrush(color, brush.GetAddressOf()))) {
        return;
    }
    m_d2dContext->DrawTextW(text, static_cast<UINT32>(wcslen(text)), fmt, rect, brush.Get());
}

void HudRenderer::DrawCornerBrackets(D2D1_RECT_F r, D2D1_COLOR_F color) {
    ComPtr<ID2D1SolidColorBrush> brush;
    if (FAILED(m_d2dContext->CreateSolidColorBrush(color, brush.GetAddressOf()))) {
        return;
    }
    const float len = 26.0f;
    const float w = 2.0f;
    m_d2dContext->DrawLine({r.left, r.top}, {r.left + len, r.top}, brush.Get(), w);
    m_d2dContext->DrawLine({r.left, r.top}, {r.left, r.top + len}, brush.Get(), w);
    m_d2dContext->DrawLine({r.right, r.bottom}, {r.right - len, r.bottom}, brush.Get(), w);
    m_d2dContext->DrawLine({r.right, r.bottom}, {r.right, r.bottom - len}, brush.Get(), w);
}

bool HudRenderer::Render(ID3D11Texture2D* target, const RrOverlay& overlay,
                         const RrSnapshot& snapshot, float opacity) {
    if (!m_ready || !target) {
        return false;
    }

    D3D11_TEXTURE2D_DESC targetDesc{};
    target->GetDesc(&targetDesc);
    if (!EnsureDrawTexture(targetDesc.Width, targetDesc.Height)) {
        return false;
    }

    if (m_directToSwapchain) {
        ComPtr<IDXGISurface> surface;
        if (FAILED(target->QueryInterface(__uuidof(IDXGISurface),
                                          reinterpret_cast<void**>(surface.GetAddressOf())))) {
            return false;
        }
        return DrawToSurface(surface.Get(), overlay, snapshot, opacity);
    }

    ComPtr<IDXGISurface> drawSurface;
    if (FAILED(m_drawTex.As(&drawSurface))) {
        return false;
    }
    if (!DrawToSurface(drawSurface.Get(), overlay, snapshot, opacity)) {
        return false;
    }
    m_drawCtx->Flush();
    m_appCtx->CopyResource(target, m_appSharedTex.Get());
    return true;
}

void HudRenderer::DrawCoach(const RrSnapshot& s, float w, float h) {
    const float cx = w * 0.5f;

    // Hero lap time, centered.
    DrawText(FormatLap(s.lap_time_ms).c_str(), m_hero.Get(),
             {cx - 320.0f, 70.0f, cx + 320.0f, 190.0f}, kHero);

    // Lap number above the hero.
    wchar_t lapBuf[32];
    swprintf(lapBuf, 32, L"LAP %d", s.lap);
    DrawText(lapBuf, m_label.Get(), {cx - 120.0f, 44.0f, cx + 120.0f, 70.0f}, kGlowDim);

    // Flanking gaps.
    DrawText(L"AHEAD", m_label.Get(), {40.0f, 96.0f, 220.0f, 120.0f}, kGlowDim);
    DrawText(FormatGap(s.gap_ahead_s).c_str(), m_value.Get(),
             {40.0f, 120.0f, 220.0f, 156.0f}, kGlow);
    DrawText(L"BEHIND", m_label.Get(), {w - 220.0f, 96.0f, w - 40.0f, 120.0f}, kGlowDim);
    DrawText(FormatGap(s.gap_behind_s).c_str(), m_value.Get(),
             {w - 220.0f, 120.0f, w - 40.0f, 156.0f}, kGlow);

    // Position corners (class + overall).
    if (s.player_position > 0 || s.player_class_position > 0) {
        wchar_t pos[48];
        if (s.player_class_position > 0 && s.player_position > 0 &&
            s.player_class_position != s.player_position) {
            swprintf(pos, 48, L"P%d  ·  P%d", s.player_class_position, s.player_position);
        } else {
            const int p = s.player_class_position > 0 ? s.player_class_position
                                                      : s.player_position;
            swprintf(pos, 48, L"P%d", p);
        }
        DrawText(pos, m_value.Get(), {40.0f, 24.0f, 360.0f, 56.0f}, kGlow);
    }

    // Flag badge (only when a flag is raised).
    if (s.session_flags != 0) {
        DrawText(L"FLAG", m_badge.Get(), {w - 200.0f, 24.0f, w - 40.0f, 56.0f}, kWarn);
    }

    // Coaching deltas row.
    const float dy = 196.0f;
    DrawText((std::wstring(L"\u0394B ") + FormatDelta(s.delta_best_ms)).c_str(),
             m_value.Get(), {cx - 360.0f, dy, cx - 120.0f, dy + 34.0f},
             DeltaColor(s.delta_best_ms));
    DrawText((std::wstring(L"\u0394L ") + FormatDelta(s.delta_last_ms)).c_str(),
             m_value.Get(), {cx - 110.0f, dy, cx + 120.0f, dy + 34.0f},
             DeltaColor(s.delta_last_ms));

    // Field pace, per user preference.
    std::wstring field;
    if (s.field_pace_mode == RR_FIELD_PACE_OPTIMAL) {
        field = std::wstring(L"OPT ") + FormatDelta(s.delta_field_optimal_ms);
    } else if (s.field_pace_mode == RR_FIELD_PACE_BOTH) {
        field = std::wstring(L"FLD ") + FormatDelta(s.delta_field_best_ms) + L"  OPT " +
                FormatDelta(s.delta_field_optimal_ms);
    } else {
        field = std::wstring(L"FLD ") + FormatDelta(s.delta_field_best_ms);
    }
    DrawText(field.c_str(), m_value.Get(), {cx + 130.0f, dy, cx + 380.0f, dy + 34.0f},
             DeltaColor(s.field_pace_mode == RR_FIELD_PACE_OPTIMAL
                            ? s.delta_field_optimal_ms
                            : s.delta_field_best_ms));

    // Pack / spotter line (hidden when off / clear handled by empty label).
    const wchar_t* pack = PackLabel(s.pack_state);
    if (pack && *pack) {
        const D2D1_COLOR_F packColor = s.pack_state == RR_PACK_CLEAR ? kGlow : kWarn;
        DrawText(pack, m_badge.Get(), {cx - 120.0f, dy + 36.0f, cx + 120.0f, dy + 70.0f},
                 packColor);
    }

    // Sector arcs.
    ComPtr<ID2D1SolidColorBrush> base, fill;
    m_d2dContext->CreateSolidColorBrush(kGlowDim, base.GetAddressOf());
    m_d2dContext->CreateSolidColorBrush(kGlow, fill.GetAddressOf());
    if (base && fill) {
        const float top = h - 14.0f;
        const float segW = (w - 120.0f) / 3.0f;
        for (int i = 0; i < 3; ++i) {
            const float x0 = 60.0f + segW * i + 8.0f;
            const float x1 = 60.0f + segW * (i + 1) - 8.0f;
            m_d2dContext->DrawLine({x0, top}, {x1, top}, base.Get(), 4.0f);
            const float pct = s.sector_done[i] ? 1.0f : s.sector_pct[i];
            if (pct > 0.0f) {
                m_d2dContext->DrawLine({x0, top}, {x0 + (x1 - x0) * pct, top}, fill.Get(),
                                       4.0f);
            }
        }
    }

    // Fuel + speed, small and centered at the bottom.
    wchar_t footer[64];
    swprintf(footer, 64, L"%.1f L   ·   %d", s.fuel_level, static_cast<int>(s.speed));
    DrawText(footer, m_label.Get(), {cx - 160.0f, h - 44.0f, cx + 160.0f, h - 18.0f},
             kGlowDim);
}

void HudRenderer::DrawStandings(const RrSnapshot& s, float w, float h) {
    DrawText(L"STANDINGS", m_label.Get(), {24.0f, 8.0f, w - 24.0f, 32.0f}, kGlowDim);
    const float rowH = 30.0f;
    const uint32_t count = s.competitor_count < RACE_REFINERY_VR_MAX_COMPETITORS
                               ? s.competitor_count
                               : RACE_REFINERY_VR_MAX_COMPETITORS;
    const uint32_t maxRows = static_cast<uint32_t>((h - 40.0f) / rowH);
    for (uint32_t i = 0; i < count && i < maxRows; ++i) {
        const RrCompetitor& c = s.competitors[i];
        const bool isPlayer = (c.flags & RR_COMPETITOR_IS_PLAYER) != 0;
        const float y = 40.0f + rowH * i;
        wchar_t line[128];
        char name[RACE_REFINERY_VR_NAME_LEN + 1];
        std::memcpy(name, c.name, RACE_REFINERY_VR_NAME_LEN);
        name[RACE_REFINERY_VR_NAME_LEN] = '\0';
        swprintf(line, 128, L"P%-2d  #%hs  %hs", c.position, c.number, name);
        DrawText(line, m_label.Get(), {24.0f, y, w - 24.0f, y + rowH}, isPlayer ? kHero : kGlow);
    }
}

void HudRenderer::DrawRelative(const RrSnapshot& s, float w, float h) {
    DrawText(L"RELATIVE", m_label.Get(), {24.0f, 8.0f, w - 24.0f, 32.0f}, kGlowDim);
    const float rowH = 32.0f;
    const float cy = h * 0.5f;
    const uint32_t count = s.competitor_count < RACE_REFINERY_VR_MAX_COMPETITORS
                               ? s.competitor_count
                               : RACE_REFINERY_VR_MAX_COMPETITORS;
    for (uint32_t i = 0; i < count; ++i) {
        const RrCompetitor& c = s.competitors[i];
        if (IsNone(c.gap_to_player_s) || (c.flags & RR_COMPETITOR_IS_PLAYER)) {
            continue;
        }
        if (std::fabs(c.gap_to_player_s) > 8.0f) {
            continue;  // only nearby cars
        }
        const float y = cy + (c.gap_to_player_s) * rowH;
        wchar_t line[96];
        swprintf(line, 96, L"#%hs  %+.1fs", c.number, c.gap_to_player_s);
        DrawText(line, m_label.Get(), {24.0f, y, w - 24.0f, y + rowH}, kGlow);
    }
    DrawText(L"YOU", m_value.Get(), {24.0f, cy - 16.0f, 200.0f, cy + 16.0f}, kHero);
}

void HudRenderer::DrawRadar(const RrSnapshot& s, float w, float h) {
    const float cx = w * 0.5f;
    const float cy = h * 0.5f;
    ComPtr<ID2D1SolidColorBrush> me, them;
    m_d2dContext->CreateSolidColorBrush(kHero, me.GetAddressOf());
    m_d2dContext->CreateSolidColorBrush(kWarn, them.GetAddressOf());
    if (me) {
        m_d2dContext->FillEllipse(D2D1::Ellipse({cx, cy}, 8.0f, 8.0f), me.Get());
    }
    const uint32_t count = s.competitor_count < RACE_REFINERY_VR_MAX_COMPETITORS
                               ? s.competitor_count
                               : RACE_REFINERY_VR_MAX_COMPETITORS;
    const float scale = 14.0f;  // pixels per second of gap
    for (uint32_t i = 0; i < count && them; ++i) {
        const RrCompetitor& c = s.competitors[i];
        if (IsNone(c.gap_to_player_s) || (c.flags & RR_COMPETITOR_IS_PLAYER)) {
            continue;
        }
        if (std::fabs(c.gap_to_player_s) > 3.0f) {
            continue;
        }
        const float y = cy - c.gap_to_player_s * scale * 4.0f;
        m_d2dContext->FillEllipse(D2D1::Ellipse({cx, y}, 6.0f, 6.0f), them.Get());
    }
}

void HudRenderer::DrawTrackMap(const RrSnapshot& s, float w, float h) {
    const RrTrackMap& map = s.track_map;
    if (map.point_count < 2) {
        DrawText(L"NO TRACK MAP", m_label.Get(),
                 {0.0f, h * 0.5f - 16.0f, w, h * 0.5f + 16.0f}, kGlowDim);
        return;
    }
    const uint32_t count = map.point_count < RACE_REFINERY_VR_MAX_TRACK_MAP_POINTS
                               ? map.point_count
                               : RACE_REFINERY_VR_MAX_TRACK_MAP_POINTS;

    ComPtr<ID2D1SolidColorBrush> circuit, me, them, pit;
    m_d2dContext->CreateSolidColorBrush(kGlowDim, circuit.GetAddressOf());
    m_d2dContext->CreateSolidColorBrush(kHero, me.GetAddressOf());
    m_d2dContext->CreateSolidColorBrush(kWarn, them.GetAddressOf());
    m_d2dContext->CreateSolidColorBrush(kGlowDim, pit.GetAddressOf());
    if (!circuit) {
        return;
    }

    // Closed polyline: one segment per vertex, last wrapping back to the start.
    for (uint32_t i = 0; i < count; ++i) {
        const D2D1_POINT_2F a = TrackMapPixel(map.points[i], w, h);
        const D2D1_POINT_2F b = TrackMapPixel(map.points[(i + 1) % count], w, h);
        m_d2dContext->DrawLine(a, b, circuit.Get(), 5.0f);
    }

    // Start / finish marker.
    const D2D1_POINT_2F start = TrackMapPixel(map.points[0], w, h);
    m_d2dContext->DrawEllipse(D2D1::Ellipse(start, 9.0f, 9.0f), circuit.Get(), 3.0f);

    const uint32_t cars = s.competitor_count < RACE_REFINERY_VR_MAX_COMPETITORS
                              ? s.competitor_count
                              : RACE_REFINERY_VR_MAX_COMPETITORS;
    for (uint32_t i = 0; i < cars && them && pit; ++i) {
        const RrCompetitor& c = s.competitors[i];
        if (c.flags & RR_COMPETITOR_IS_PLAYER) {
            continue;  // the player is drawn from the snapshot below
        }
        ID2D1SolidColorBrush* brush =
            (c.flags & RR_COMPETITOR_ON_PIT_ROAD) ? pit.Get() : them.Get();
        const D2D1_POINT_2F at = TrackMapPointAt(map, c.lap_dist_pct, w, h);
        m_d2dContext->FillEllipse(D2D1::Ellipse(at, 7.0f, 7.0f), brush);
    }

    if (me) {
        const D2D1_POINT_2F at = TrackMapPointAt(map, s.lap_dist_pct, w, h);
        m_d2dContext->FillEllipse(D2D1::Ellipse(at, 9.0f, 9.0f), me.Get());
    }
}
