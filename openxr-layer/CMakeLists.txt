cmake_minimum_required(VERSION 3.22)

project(race-refinery-openxr-layer LANGUAGES CXX)

# This is a standalone Windows OpenXR API layer, intentionally built outside the
# Tauri/Cargo tree. It is loaded by the OpenXR loader into the iRacing process,
# not into Race Refinery. See docs/NATIVE_VR.md for the build + install story.

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if(NOT WIN32)
    message(FATAL_ERROR "race-refinery-openxr-layer targets Windows (D3D11 + Win32 SHM) only.")
endif()

include(FetchContent)

# OpenXR-SDK provides the loader headers (openxr.h, openxr_platform.h) and the
# api layer negotiation structs. We only need headers + the generated dispatch
# helpers; we do not link the loader (the runtime/loader loads us).
FetchContent_Declare(
    OpenXR
    GIT_REPOSITORY https://github.com/KhronosGroup/OpenXR-SDK.git
    GIT_TAG release-1.1.43
)
FetchContent_MakeAvailable(OpenXR)

add_library(race-refinery-openxr-layer SHARED
    src/layer.cpp
    src/hud_renderer.cpp
    race-refinery-openxr-layer.def
)

target_include_directories(race-refinery-openxr-layer PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/include"
    "${CMAKE_CURRENT_SOURCE_DIR}/src"
)

target_compile_definitions(race-refinery-openxr-layer PRIVATE
    XR_USE_GRAPHICS_API_D3D11
    XR_USE_PLATFORM_WIN32
    NOMINMAX
    WIN32_LEAN_AND_MEAN
    UNICODE
    _UNICODE
)

target_link_libraries(race-refinery-openxr-layer PRIVATE
    OpenXR::headers
    d3d11
    dxgi
    d2d1
    dwrite
)

# The runtime expects the DLL name referenced by the manifest's library_path.
set_target_properties(race-refinery-openxr-layer PROPERTIES
    OUTPUT_NAME "race-refinery-openxr-layer"
    PREFIX ""
)

# Stage the DLL next to the manifest so the registry key can point at one folder.
add_custom_command(TARGET race-refinery-openxr-layer POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy
        "${CMAKE_CURRENT_SOURCE_DIR}/manifest/race_refinery_openxr_layer.json"
        "$<TARGET_FILE_DIR:race-refinery-openxr-layer>/race_refinery_openxr_layer.json"
)
