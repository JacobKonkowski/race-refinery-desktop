// Direct2D/DirectWrite renderer for the Race Refinery overlays.
//
// Draws into OpenXR D3D11 swapchain textures. iRacing's device often lacks
// D3D11_CREATE_DEVICE_BGRA_SUPPORT, so we prefer a private BGRA device on the
// same adapter and CopyResource into the swapchain via a shared texture.

#pragma once

#include <d2d1_1.h>
#include <d3d11.h>
#include <dwrite.h>
#include <wrl/client.h>

#include "race_refinery_vr_shm.h"

class HudRenderer {
public:
    HudRenderer() = default;
    ~HudRenderer() = default;

    // Bind using the session's D3D11 device (from binding or texture->GetDevice).
    // Safe to call repeatedly; only the first successful call allocates.
    bool Initialize(ID3D11Device* appDevice);

    // Draw one overlay into `target` (a BGRA swapchain texture) from `snapshot`.
    bool Render(ID3D11Texture2D* target, const RrOverlay& overlay,
                const RrSnapshot& snapshot, float opacity);

private:
    Microsoft::WRL::ComPtr<ID3D11Device> m_appDevice;
    Microsoft::WRL::ComPtr<ID3D11DeviceContext> m_appCtx;
    Microsoft::WRL::ComPtr<ID3D11Device> m_drawDevice;
    Microsoft::WRL::ComPtr<ID3D11DeviceContext> m_drawCtx;

    Microsoft::WRL::ComPtr<ID2D1Factory1> m_d2dFactory;
    Microsoft::WRL::ComPtr<IDWriteFactory> m_dwriteFactory;
    Microsoft::WRL::ComPtr<ID2D1Device> m_d2dDevice;
    Microsoft::WRL::ComPtr<ID2D1DeviceContext> m_d2dContext;

    Microsoft::WRL::ComPtr<IDWriteTextFormat> m_hero;
    Microsoft::WRL::ComPtr<IDWriteTextFormat> m_label;
    Microsoft::WRL::ComPtr<IDWriteTextFormat> m_value;
    Microsoft::WRL::ComPtr<IDWriteTextFormat> m_badge;

    // Intermediate draw target on m_drawDevice (shared → opened on app device).
    Microsoft::WRL::ComPtr<ID3D11Texture2D> m_drawTex;
    Microsoft::WRL::ComPtr<ID3D11Texture2D> m_appSharedTex;
    uint32_t m_texW = 0;
    uint32_t m_texH = 0;
    bool m_directToSwapchain = false;  // D2D on app device works
    bool m_ready = false;

    bool InitD2D(ID3D11Device* device);
    bool EnsureDrawTexture(uint32_t width, uint32_t height);
    bool DrawToSurface(IDXGISurface* surface, const RrOverlay& overlay,
                       const RrSnapshot& snapshot, float opacity);

    void DrawCoach(const RrSnapshot& s, float w, float h);
    void DrawStandings(const RrSnapshot& s, float w, float h);
    void DrawRelative(const RrSnapshot& s, float w, float h);
    void DrawRadar(const RrSnapshot& s, float w, float h);
    void DrawTrackMap(const RrSnapshot& s, float w, float h);

    void DrawText(const wchar_t* text, IDWriteTextFormat* fmt, D2D1_RECT_F rect,
                  D2D1_COLOR_F color);
    void DrawCornerBrackets(D2D1_RECT_F rect, D2D1_COLOR_F color);
};
