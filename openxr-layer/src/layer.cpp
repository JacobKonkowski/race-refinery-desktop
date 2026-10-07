// Race Refinery OpenXR API layer.
//
// Inserted by the OpenXR loader between iRacing and the active runtime (Meta,
// SteamVR, VDXR, ...). It hooks xrEndFrame and appends one composition-layer
// quad per enabled Race Refinery overlay, drawing the pixels with Direct2D from the
// shared-memory snapshot produced by the Race Refinery desktop process.
//
// Structure follows the standard implicit-layer pattern from
// Ybalrid/OpenXR-API-Layer-Template: negotiate -> create-instance shim ->
// per-function dispatch via xrGetInstanceProcAddr.

#include <d3d11.h>
#include <windows.h>

#include <chrono>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#include <openxr/openxr.h>
#include <openxr/openxr_platform.h>
#include <openxr/openxr_loader_negotiation.h>

#include "hud_renderer.h"
#include "race_refinery_vr_shm.h"
#include "shm_reader.h"

// One-shot diagnostics for "is the layer actually in the iRacing process?"
// Avoids any I/O in xrEndFrame.
static void WriteLayerStatus(const char* line) {
    char path[MAX_PATH] = {};
    if (GetEnvironmentVariableA("LOCALAPPDATA", path, MAX_PATH) == 0) {
        return;
    }
    const std::string file =
        std::string(path) + "\\race-refinery\\layer-status.txt";
    HANDLE h = CreateFileA(file.c_str(), GENERIC_WRITE, FILE_SHARE_READ, nullptr,
                           CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (h == INVALID_HANDLE_VALUE) {
        return;
    }
    const DWORD len = static_cast<DWORD>(std::strlen(line));
    DWORD written = 0;
    WriteFile(h, line, len, &written, nullptr);
    CloseHandle(h);
}

namespace {

constexpr int64_t kSwapchainFormatBgra = DXGI_FORMAT_B8G8R8A8_UNORM;
constexpr uint64_t kMaxSnapshotAgeMs = 2000;

uint64_t NowMs() {
    using namespace std::chrono;
    return duration_cast<milliseconds>(system_clock::now().time_since_epoch()).count();
}

// Per-kind swapchain dimensions.
void HudDimensions(uint32_t kind, uint32_t& width, uint32_t& height) {
    switch (kind) {
        case RR_OVERLAY_STANDINGS: width = 512; height = 640; break;  // tall list
        case RR_OVERLAY_RELATIVE:  width = 512; height = 512; break;  // square board
        case RR_OVERLAY_RADAR:     width = 512; height = 512; break;  // square dish
        case RR_OVERLAY_TRACKMAP:  width = 512; height = 512; break;  // square circuit
        case RR_OVERLAY_COACH:
        default:                   width = 1024; height = 288; break; // wide-short
    }
}

// One overlay slot's GPU resources.
struct OverlaySwapchain {
    XrSwapchain swapchain = XR_NULL_HANDLE;
    std::vector<ID3D11Texture2D*> images;
    uint32_t width = 0;
    uint32_t height = 0;
};

// Next-layer dispatch pointers we need.
struct Dispatch {
    PFN_xrGetInstanceProcAddr getInstanceProcAddr = nullptr;
    PFN_xrDestroyInstance destroyInstance = nullptr;
    PFN_xrCreateSession createSession = nullptr;
    PFN_xrDestroySession destroySession = nullptr;
    PFN_xrCreateReferenceSpace createReferenceSpace = nullptr;
    PFN_xrDestroySpace destroySpace = nullptr;
    PFN_xrLocateSpace locateSpace = nullptr;
    PFN_xrEndFrame endFrame = nullptr;
    PFN_xrCreateSwapchain createSwapchain = nullptr;
    PFN_xrDestroySwapchain destroySwapchain = nullptr;
    PFN_xrEnumerateSwapchainImages enumerateSwapchainImages = nullptr;
    PFN_xrAcquireSwapchainImage acquireSwapchainImage = nullptr;
    PFN_xrWaitSwapchainImage waitSwapchainImage = nullptr;
    PFN_xrReleaseSwapchainImage releaseSwapchainImage = nullptr;
};

struct LayerContext {
    XrInstance instance = XR_NULL_HANDLE;
    Dispatch dispatch;

    XrSession session = XR_NULL_HANDLE;
    ID3D11Device* device = nullptr;
    XrSpace viewSpace = XR_NULL_HANDLE;
    XrSpace localSpace = XR_NULL_HANDLE;
    // LOCAL space re-originated at the driver's level head pose; world-locked
    // quads are placed relative to it. Rebuilt on the first frame and on recenter.
    XrSpace anchorSpace = XR_NULL_HANDLE;
    uint32_t lastRecenterSeq = 0;
    bool needAnchor = true;

    OverlaySwapchain overlays[RACE_REFINERY_VR_MAX_OVERLAYS];
    HudRenderer renderer;
    ShmReader shm;
    bool sessionReady = false;
    bool deviceOwned = false;  // true when device came from texture->GetDevice
};

LayerContext g_ctx;

bool EnsureRendererFromTexture(ID3D11Texture2D* texture) {
    if (g_ctx.renderer.Initialize(g_ctx.device)) {
        return true;
    }
    if (!texture) {
        return false;
    }
    ID3D11Device* device = nullptr;
    texture->GetDevice(&device);
    if (!device) {
        WriteLayerStatus("swapchain: texture GetDevice returned null");
        return false;
    }
    const bool ok = g_ctx.renderer.Initialize(device);
    if (ok) {
        if (g_ctx.deviceOwned && g_ctx.device) {
            g_ctx.device->Release();
        }
        g_ctx.device = device;  // takes the GetDevice ref
        g_ctx.deviceOwned = true;
        WriteLayerStatus("sessionReady: compositing enabled (device from swapchain)");
    } else {
        WriteLayerStatus("swapchain: HudRenderer::Initialize failed");
        device->Release();
    }
    return ok;
}

// ---------------------------------------------------------------------------
// Swapchain helpers
// ---------------------------------------------------------------------------

bool EnsureOverlaySwapchain(int slot, uint32_t kind) {
    if (slot < 0 || slot >= RACE_REFINERY_VR_MAX_OVERLAYS) {
        return false;
    }
    OverlaySwapchain& ov = g_ctx.overlays[slot];
    if (ov.swapchain != XR_NULL_HANDLE) {
        return true;
    }

    HudDimensions(kind, ov.width, ov.height);

    XrSwapchainCreateInfo info{XR_TYPE_SWAPCHAIN_CREATE_INFO};
    info.usageFlags = XR_SWAPCHAIN_USAGE_COLOR_ATTACHMENT_BIT;
    info.format = kSwapchainFormatBgra;
    info.sampleCount = 1;
    info.width = ov.width;
    info.height = ov.height;
    info.faceCount = 1;
    info.arraySize = 1;
    info.mipCount = 1;

    if (XR_FAILED(g_ctx.dispatch.createSwapchain(g_ctx.session, &info, &ov.swapchain))) {
        ov.swapchain = XR_NULL_HANDLE;
        WriteLayerStatus("createSwapchain failed");
        return false;
    }

    uint32_t count = 0;
    if (XR_FAILED(g_ctx.dispatch.enumerateSwapchainImages(ov.swapchain, 0, &count, nullptr)) ||
        count == 0) {
        WriteLayerStatus("enumerateSwapchainImages failed (is session D3D11?)");
        return false;
    }
    std::vector<XrSwapchainImageD3D11KHR> images(
        count, {XR_TYPE_SWAPCHAIN_IMAGE_D3D11_KHR});
    if (XR_FAILED(g_ctx.dispatch.enumerateSwapchainImages(
            ov.swapchain, count, &count,
            reinterpret_cast<XrSwapchainImageBaseHeader*>(images.data())))) {
        WriteLayerStatus("enumerateSwapchainImages(D3D11) failed");
        return false;
    }
    ov.images.clear();
    for (auto& img : images) {
        ov.images.push_back(img.texture);
    }
    if (!ov.images.empty() && !EnsureRendererFromTexture(ov.images[0])) {
        return false;
    }
    return true;
}

// Acquire/wait/render/release one overlay's swapchain image. Returns true if the
// image is ready to be referenced by a composition layer this frame.
bool RenderOverlay(int slot, const RrOverlay& overlay, const RrSnapshot& snapshot) {
    if (!EnsureOverlaySwapchain(slot, overlay.kind)) {
        return false;
    }
    OverlaySwapchain& ov = g_ctx.overlays[slot];

    uint32_t index = 0;
    XrSwapchainImageAcquireInfo acquire{XR_TYPE_SWAPCHAIN_IMAGE_ACQUIRE_INFO};
    if (XR_FAILED(g_ctx.dispatch.acquireSwapchainImage(ov.swapchain, &acquire, &index))) {
        return false;
    }
    XrSwapchainImageWaitInfo wait{XR_TYPE_SWAPCHAIN_IMAGE_WAIT_INFO};
    wait.timeout = XR_INFINITE_DURATION;
    if (XR_FAILED(g_ctx.dispatch.waitSwapchainImage(ov.swapchain, &wait))) {
        return false;
    }

    bool ok = false;
    if (index < ov.images.size()) {
        ok = g_ctx.renderer.Render(ov.images[index], overlay, snapshot, overlay.opacity);
    }

    XrSwapchainImageReleaseInfo release{XR_TYPE_SWAPCHAIN_IMAGE_RELEASE_INFO};
    g_ctx.dispatch.releaseSwapchainImage(ov.swapchain, &release);
    return ok;
}

XrSpace SpaceFor(uint32_t lockSpace) {
    if (lockSpace == RR_LOCK_LOCAL) {
        return g_ctx.anchorSpace != XR_NULL_HANDLE ? g_ctx.anchorSpace : g_ctx.localSpace;
    }
    return g_ctx.viewSpace;
}

// Rebuild the anchor at the head's current position, keeping only yaw so the
// panels stay level. Returns false (and keeps the old anchor) when tracking is
// not valid yet, so the caller retries on a later frame.
bool RebuildAnchor(XrTime displayTime) {
    if (!g_ctx.dispatch.locateSpace || g_ctx.localSpace == XR_NULL_HANDLE) {
        return false;
    }
    XrSpaceLocation loc{XR_TYPE_SPACE_LOCATION};
    if (XR_FAILED(g_ctx.dispatch.locateSpace(g_ctx.viewSpace, g_ctx.localSpace, displayTime,
                                             &loc))) {
        return false;
    }
    const XrSpaceLocationFlags needed =
        XR_SPACE_LOCATION_POSITION_VALID_BIT | XR_SPACE_LOCATION_ORIENTATION_VALID_BIT;
    if ((loc.locationFlags & needed) != needed) {
        return false;
    }

    // Yaw of the head's forward (-Z) vector projected onto the horizontal plane.
    const XrQuaternionf& q = loc.pose.orientation;
    const float fx = -2.0f * (q.x * q.z + q.w * q.y);
    const float fz = -(1.0f - 2.0f * (q.x * q.x + q.y * q.y));
    const float yaw = std::atan2(-fx, -fz);

    XrReferenceSpaceCreateInfo info{XR_TYPE_REFERENCE_SPACE_CREATE_INFO};
    info.referenceSpaceType = XR_REFERENCE_SPACE_TYPE_LOCAL;
    info.poseInReferenceSpace.position = loc.pose.position;
    info.poseInReferenceSpace.orientation = {0.0f, std::sin(yaw * 0.5f), 0.0f,
                                             std::cos(yaw * 0.5f)};
    XrSpace next = XR_NULL_HANDLE;
    if (XR_FAILED(g_ctx.dispatch.createReferenceSpace(g_ctx.session, &info, &next))) {
        return false;
    }
    if (g_ctx.anchorSpace != XR_NULL_HANDLE) {
        g_ctx.dispatch.destroySpace(g_ctx.anchorSpace);
    }
    g_ctx.anchorSpace = next;
    WriteLayerStatus("anchor: recentered");
    return true;
}

// ---------------------------------------------------------------------------
// Hooked entry points
// ---------------------------------------------------------------------------

XRAPI_ATTR XrResult XRAPI_CALL RaceRefinery_xrEndFrame(XrSession session,
                                                  const XrFrameEndInfo* frameEndInfo) {
    if (session != g_ctx.session || !g_ctx.sessionReady || frameEndInfo == nullptr) {
        return g_ctx.dispatch.endFrame(session, frameEndInfo);
    }

    RrSharedBlock block;
    if (!g_ctx.shm.Read(block, NowMs(), kMaxSnapshotAgeMs)) {
        return g_ctx.dispatch.endFrame(session, frameEndInfo);
    }

    if (g_ctx.needAnchor || block.recenter_seq != g_ctx.lastRecenterSeq) {
        if (RebuildAnchor(frameEndInfo->displayTime)) {
            g_ctx.needAnchor = false;
            g_ctx.lastRecenterSeq = block.recenter_seq;
        }
    }

    std::vector<const XrCompositionLayerBaseHeader*> layers(
        frameEndInfo->layers, frameEndInfo->layers + frameEndInfo->layerCount);

    // Quad structs must outlive the endFrame call below.
    std::vector<XrCompositionLayerQuad> quads;
    quads.reserve(RACE_REFINERY_VR_MAX_OVERLAYS);

    const uint32_t overlayCount =
        block.overlay_count < RACE_REFINERY_VR_MAX_OVERLAYS ? block.overlay_count
                                                      : RACE_REFINERY_VR_MAX_OVERLAYS;
    for (uint32_t i = 0; i < overlayCount; ++i) {
        const RrOverlay& ov = block.overlays[i];
        if (!ov.enabled) {
            continue;
        }
        if (!RenderOverlay(static_cast<int>(i), ov, block.snapshot)) {
            continue;
        }

        XrCompositionLayerQuad quad{XR_TYPE_COMPOSITION_LAYER_QUAD};
        quad.layerFlags = XR_COMPOSITION_LAYER_BLEND_TEXTURE_SOURCE_ALPHA_BIT;
        quad.space = SpaceFor(ov.lock_space);
        quad.eyeVisibility = XR_EYE_VISIBILITY_BOTH;
        quad.subImage.swapchain = g_ctx.overlays[i].swapchain;
        quad.subImage.imageArrayIndex = 0;
        quad.subImage.imageRect = {{0, 0},
                                   {static_cast<int32_t>(g_ctx.overlays[i].width),
                                    static_cast<int32_t>(g_ctx.overlays[i].height)}};
        quad.pose.orientation = {ov.rot_x, ov.rot_y, ov.rot_z, ov.rot_w};
        quad.pose.position = {ov.pos_x, ov.pos_y, ov.pos_z};
        quad.size = {ov.size_w, ov.size_h};
        quads.push_back(quad);
    }

    for (auto& quad : quads) {
        layers.push_back(reinterpret_cast<const XrCompositionLayerBaseHeader*>(&quad));
    }

    XrFrameEndInfo modified = *frameEndInfo;
    modified.layerCount = static_cast<uint32_t>(layers.size());
    modified.layers = layers.data();
    return g_ctx.dispatch.endFrame(session, &modified);
}

XRAPI_ATTR XrResult XRAPI_CALL RaceRefinery_xrCreateSession(XrInstance instance,
                                                       const XrSessionCreateInfo* createInfo,
                                                       XrSession* session) {
    const XrResult res = g_ctx.dispatch.createSession(instance, createInfo, session);
    if (XR_FAILED(res)) {
        return res;
    }

    // Pull the D3D11 device from the graphics binding chain when present.
    char typesBuf[192] = "createSession types:";
    size_t typesLen = std::strlen(typesBuf);
    const void* entry = createInfo ? createInfo->next : nullptr;
    int hops = 0;
    while (entry && hops < 12) {
        const auto* hdr = static_cast<const XrBaseInStructure*>(entry);
        char piece[24];
        std::snprintf(piece, sizeof(piece), " %d", static_cast<int>(hdr->type));
        const size_t pieceLen = std::strlen(piece);
        if (typesLen + pieceLen < sizeof(typesBuf) - 1) {
            std::memcpy(typesBuf + typesLen, piece, pieceLen + 1);
            typesLen += pieceLen;
        }
        if (hdr->type == XR_TYPE_GRAPHICS_BINDING_D3D11_KHR) {
            g_ctx.device =
                reinterpret_cast<const XrGraphicsBindingD3D11KHR*>(hdr)->device;
            break;
        }
        entry = hdr->next;
        ++hops;
    }

    g_ctx.session = *session;

    XrReferenceSpaceCreateInfo viewInfo{XR_TYPE_REFERENCE_SPACE_CREATE_INFO};
    viewInfo.referenceSpaceType = XR_REFERENCE_SPACE_TYPE_VIEW;
    viewInfo.poseInReferenceSpace.orientation.w = 1.0f;
    g_ctx.dispatch.createReferenceSpace(*session, &viewInfo, &g_ctx.viewSpace);

    XrReferenceSpaceCreateInfo localInfo{XR_TYPE_REFERENCE_SPACE_CREATE_INFO};
    localInfo.referenceSpaceType = XR_REFERENCE_SPACE_TYPE_LOCAL;
    localInfo.poseInReferenceSpace.orientation.w = 1.0f;
    g_ctx.dispatch.createReferenceSpace(*session, &localInfo, &g_ctx.localSpace);

    g_ctx.sessionReady = (g_ctx.viewSpace != XR_NULL_HANDLE);
    if (!g_ctx.sessionReady) {
        WriteLayerStatus("createSession: view space failed");
        return res;
    }

    if (g_ctx.device && g_ctx.renderer.Initialize(g_ctx.device)) {
        WriteLayerStatus("sessionReady: compositing enabled (D3D11 binding)");
    } else if (g_ctx.device) {
        WriteLayerStatus("createSession: D3D11 binding present but renderer init failed");
    } else {
        // Defer: first EndFrame swapchain textures expose the real device via GetDevice.
        WriteLayerStatus(typesBuf);
    }
    return res;
}

XRAPI_ATTR XrResult XRAPI_CALL RaceRefinery_xrDestroySession(XrSession session) {
    if (session == g_ctx.session) {
        for (auto& ov : g_ctx.overlays) {
            if (ov.swapchain != XR_NULL_HANDLE) {
                g_ctx.dispatch.destroySwapchain(ov.swapchain);
                ov.swapchain = XR_NULL_HANDLE;
                ov.images.clear();
            }
        }
        if (g_ctx.viewSpace) g_ctx.dispatch.destroySpace(g_ctx.viewSpace);
        if (g_ctx.localSpace) g_ctx.dispatch.destroySpace(g_ctx.localSpace);
        if (g_ctx.anchorSpace) g_ctx.dispatch.destroySpace(g_ctx.anchorSpace);
        g_ctx.viewSpace = g_ctx.localSpace = g_ctx.anchorSpace = XR_NULL_HANDLE;
        g_ctx.needAnchor = true;
        g_ctx.session = XR_NULL_HANDLE;
        g_ctx.sessionReady = false;
        if (g_ctx.deviceOwned && g_ctx.device) {
            g_ctx.device->Release();
        }
        g_ctx.device = nullptr;
        g_ctx.deviceOwned = false;
    }
    return g_ctx.dispatch.destroySession(session);
}

XRAPI_ATTR XrResult XRAPI_CALL RaceRefinery_xrDestroyInstance(XrInstance instance) {
    PFN_xrDestroyInstance down = g_ctx.dispatch.destroyInstance;
    g_ctx = LayerContext{};
    return down ? down(instance) : XR_SUCCESS;
}

XRAPI_ATTR XrResult XRAPI_CALL RaceRefinery_xrGetInstanceProcAddr(XrInstance instance,
                                                             const char* name,
                                                             PFN_xrVoidFunction* function) {
    const auto bind = [&](PFN_xrVoidFunction fn) {
        *function = fn;
        return XR_SUCCESS;
    };
    if (std::strcmp(name, "xrEndFrame") == 0)
        return bind(reinterpret_cast<PFN_xrVoidFunction>(RaceRefinery_xrEndFrame));
    if (std::strcmp(name, "xrCreateSession") == 0)
        return bind(reinterpret_cast<PFN_xrVoidFunction>(RaceRefinery_xrCreateSession));
    if (std::strcmp(name, "xrDestroySession") == 0)
        return bind(reinterpret_cast<PFN_xrVoidFunction>(RaceRefinery_xrDestroySession));
    if (std::strcmp(name, "xrDestroyInstance") == 0)
        return bind(reinterpret_cast<PFN_xrVoidFunction>(RaceRefinery_xrDestroyInstance));
    return g_ctx.dispatch.getInstanceProcAddr(instance, name, function);
}

template <typename T>
void Load(const char* name, T& slot) {
    PFN_xrVoidFunction fn = nullptr;
    if (XR_SUCCEEDED(g_ctx.dispatch.getInstanceProcAddr(g_ctx.instance, name, &fn))) {
        slot = reinterpret_cast<T>(fn);
    }
}

XRAPI_ATTR XrResult XRAPI_CALL RaceRefinery_xrCreateApiLayerInstance(
    const XrInstanceCreateInfo* info, const XrApiLayerCreateInfo* apiLayerInfo,
    XrInstance* instance) {
    XrApiLayerNextInfo* nextInfo = apiLayerInfo->nextInfo;
    g_ctx.dispatch.getInstanceProcAddr = nextInfo->nextGetInstanceProcAddr;

    XrApiLayerCreateInfo nextLayerInfo = *apiLayerInfo;
    nextLayerInfo.nextInfo = nextInfo->next;
    const XrResult res =
        nextInfo->nextCreateApiLayerInstance(info, &nextLayerInfo, instance);
    if (XR_FAILED(res)) {
        return res;
    }

    g_ctx.instance = *instance;
    Load("xrDestroyInstance", g_ctx.dispatch.destroyInstance);
    Load("xrCreateSession", g_ctx.dispatch.createSession);
    Load("xrDestroySession", g_ctx.dispatch.destroySession);
    Load("xrCreateReferenceSpace", g_ctx.dispatch.createReferenceSpace);
    Load("xrDestroySpace", g_ctx.dispatch.destroySpace);
    Load("xrLocateSpace", g_ctx.dispatch.locateSpace);
    Load("xrEndFrame", g_ctx.dispatch.endFrame);
    Load("xrCreateSwapchain", g_ctx.dispatch.createSwapchain);
    Load("xrDestroySwapchain", g_ctx.dispatch.destroySwapchain);
    Load("xrEnumerateSwapchainImages", g_ctx.dispatch.enumerateSwapchainImages);
    Load("xrAcquireSwapchainImage", g_ctx.dispatch.acquireSwapchainImage);
    Load("xrWaitSwapchainImage", g_ctx.dispatch.waitSwapchainImage);
    Load("xrReleaseSwapchainImage", g_ctx.dispatch.releaseSwapchainImage);
    WriteLayerStatus("apiLayerInstance: hooks installed");
    return res;
}

}  // namespace

extern "C" {

// Entry point the OpenXR loader looks up by name (see the manifest's
// negotiation contract). Advertises our two shim functions.
XRAPI_ATTR XrResult XRAPI_CALL xrNegotiateLoaderApiLayerInterface(
    const XrNegotiateLoaderInfo* loaderInfo, const char* /*layerName*/,
    XrNegotiateApiLayerRequest* apiLayerRequest) {
    if (!loaderInfo || !apiLayerRequest ||
        loaderInfo->structType != XR_LOADER_INTERFACE_STRUCT_LOADER_INFO ||
        apiLayerRequest->structType != XR_LOADER_INTERFACE_STRUCT_API_LAYER_REQUEST) {
        return XR_ERROR_INITIALIZATION_FAILED;
    }
    apiLayerRequest->layerInterfaceVersion = XR_CURRENT_LOADER_API_LAYER_VERSION;
    apiLayerRequest->layerApiVersion = XR_CURRENT_API_VERSION;
    apiLayerRequest->getInstanceProcAddr = RaceRefinery_xrGetInstanceProcAddr;
    apiLayerRequest->createApiLayerInstance = RaceRefinery_xrCreateApiLayerInstance;
    WriteLayerStatus("negotiate: Race Refinery OpenXR layer loaded");
    return XR_SUCCESS;
}

BOOL WINAPI DllMain(HINSTANCE, DWORD, LPVOID) {
    return TRUE;
}

}  // extern "C"
