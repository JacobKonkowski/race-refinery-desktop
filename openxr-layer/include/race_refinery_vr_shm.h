// Race Refinery VR shared-memory contract.
//
// This header is the single source of truth for the binary layout exchanged
// between the Race Refinery desktop process (producer, written in Rust) and the
// race-refinery-openxr-layer DLL (consumer, this C++ project). The Rust mirror lives
// in `crates/race-refinery-vr/src/shm.rs` and MUST stay byte-for-byte identical.
//
// Layout rules that keep both sides in sync without compiler-specific packing:
//   * Every field is 4 bytes (i32 / u32 / f32) or a char array whose length is
//     a multiple of 4, so natural alignment never inserts hidden padding.
//   * 64-bit values are split into lo/hi u32 pairs for the same reason.
//   * "Absent" optional floats are encoded as NaN; absent positions use 0;
//     absent lap counts use -1.
//
// Concurrency: a seqlock on `seq`. The writer sets `seq` odd before mutating
// the block and even (incremented) after. A reader copies the block, then
// rechecks that `seq` is unchanged and even; otherwise it retries.

#ifndef RACE_REFINERY_VR_SHM_H
#define RACE_REFINERY_VR_SHM_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// "RRVR" as little-endian bytes ('R'=0x52, 'R'=0x52, 'V'=0x56, 'R'=0x52).
#define RACE_REFINERY_VR_MAGIC 0x52565252u
// v2 added slot 4 (track map) and `RrSnapshot.track_map`. Consumers built
// against v1 must refuse a v2 block: both MAX_OVERLAYS and RrSnapshot changed.
// v3 added `RrSharedBlock.recenter_seq`, shifting every later field.
#define RACE_REFINERY_VR_VERSION 3u

#define RACE_REFINERY_VR_SHM_NAME "Local\\RaceRefineryVR"

#define RACE_REFINERY_VR_MAX_OVERLAYS 5
#define RACE_REFINERY_VR_MAX_COMPETITORS 64
#define RACE_REFINERY_VR_MAX_TRACK_MAP_POINTS 256
#define RACE_REFINERY_VR_NUM_LEN 8
#define RACE_REFINERY_VR_NAME_LEN 40
#define RACE_REFINERY_VR_TRACK_LEN 64
#define RACE_REFINERY_VR_SESSION_LEN 32

// Overlay slot kind. The kind equals its index in `overlays`.
enum RrOverlayKind {
    RR_OVERLAY_COACH = 0,
    RR_OVERLAY_STANDINGS = 1,
    RR_OVERLAY_RELATIVE = 2,
    RR_OVERLAY_RADAR = 3,
    RR_OVERLAY_TRACKMAP = 4,
};

// Reference space the overlay quad is locked to.
enum RrLockSpace {
    RR_LOCK_VIEW = 0,   // head-locked (XR_REFERENCE_SPACE_TYPE_VIEW)
    RR_LOCK_LOCAL = 1,  // world-locked (XR_REFERENCE_SPACE_TYPE_LOCAL)
};

// One overlay's placement + visibility. 13 x 4 = 52 bytes.
typedef struct RrOverlay {
    uint32_t enabled;     // 0 / 1
    uint32_t kind;        // RrOverlayKind
    uint32_t lock_space;  // RrLockSpace
    float opacity;        // 0..1
    float pos_x;          // meters, relative to the locked space
    float pos_y;
    float pos_z;
    float rot_x;          // orientation quaternion (xyzw)
    float rot_y;
    float rot_z;
    float rot_w;
    float size_w;         // quad size in meters
    float size_h;
} RrOverlay;

// One competitor row for the standings / relative / radar overlays. 80 bytes.
typedef struct RrCompetitor {
    int32_t position;        // overall position, 0 = none
    int32_t class_position;  // class position, 0 = none
    int32_t class_id;
    float best_lap_ms;       // NaN = none
    float last_lap_ms;       // NaN = none
    float lap_dist_pct;      // 0..1 around the lap
    float gap_to_player_s;   // signed seconds; + = ahead of player
    uint32_t flags;          // bit0 = is_player, bit1 = on_pit_road
    char number[RACE_REFINERY_VR_NUM_LEN];
    char name[RACE_REFINERY_VR_NAME_LEN];
} RrCompetitor;

#define RR_COMPETITOR_IS_PLAYER 0x1u
#define RR_COMPETITOR_ON_PIT_ROAD 0x2u

// One vertex of the generated circuit outline. `x` / `y` are normalized into a
// 0..1 box (y grows downward); `pct` is the lap fraction at that vertex.
typedef struct RrTrackMapPoint {
    float x;
    float y;
    float pct;
} RrTrackMapPoint;

// Circuit outline for the current track, built by the desktop app from IBT GPS
// and cached per track. `point_count == 0` means no outline is available; the
// track map overlay then draws its empty state.
typedef struct RrTrackMap {
    uint32_t point_count;
    RrTrackMapPoint points[RACE_REFINERY_VR_MAX_TRACK_MAP_POINTS];
} RrTrackMap;

// Field-pace display preference, mirrors AppSettings.vr_field_pace_mode.
enum RrFieldPaceMode {
    RR_FIELD_PACE_BEST = 0,
    RR_FIELD_PACE_OPTIMAL = 1,
    RR_FIELD_PACE_BOTH = 2,
};

// Compact mirror of LiveSnapshot, the data every overlay renders from.
typedef struct RrSnapshot {
    int32_t lap;
    float lap_time_ms;
    float last_lap_ms;             // NaN = none
    float best_lap_ms;             // NaN = none
    float delta_best_ms;           // NaN = none
    float delta_last_ms;           // NaN = none
    float delta_field_best_ms;     // NaN = none
    float delta_field_optimal_ms;  // NaN = none
    int32_t player_position;       // 0 = none
    int32_t player_class_position; // 0 = none
    float gap_ahead_s;             // NaN = none
    float gap_behind_s;            // NaN = none
    float fuel_level;
    float speed;
    float lap_dist_pct;
    int32_t current_sector;
    uint32_t pack_state;           // mirrors PackState ordinal
    uint32_t session_flags;        // raw iRacing SessionFlags bitfield
    int32_t incident_count;
    int32_t session_laps_remain;   // -1 = none
    float session_time_remain_s;   // NaN = none
    uint32_t on_track;             // 0 / 1
    uint32_t field_pace_mode;      // RrFieldPaceMode
    float sector_pct[3];           // 0..1 progress per sector
    uint32_t sector_done[3];       // 0 / 1 completed flag per sector
    uint32_t competitor_count;
    char track[RACE_REFINERY_VR_TRACK_LEN];
    char session_type[RACE_REFINERY_VR_SESSION_LEN];
    RrCompetitor competitors[RACE_REFINERY_VR_MAX_COMPETITORS];
    RrTrackMap track_map;  // v2
} RrSnapshot;

// PackState ordinals, matching src-tauri/src/live/pack.rs.
enum RrPackState {
    RR_PACK_OFF = 0,
    RR_PACK_CLEAR = 1,
    RR_PACK_CAR_LEFT = 2,
    RR_PACK_CAR_RIGHT = 3,
    RR_PACK_THREE_WIDE = 4,
    RR_PACK_TWO_LEFT = 5,
    RR_PACK_TWO_RIGHT = 6,
};

// Top-level shared block. Producer maps it writable; layer maps it read-only.
typedef struct RrSharedBlock {
    uint32_t magic;         // RACE_REFINERY_VR_MAGIC
    uint32_t version;       // RACE_REFINERY_VR_VERSION
    uint32_t seq;           // seqlock; odd while writing
    uint32_t overlay_count; // active overlays in `overlays`
    uint32_t write_ms_lo;   // low 32 bits of last write time (ms since epoch)
    uint32_t write_ms_hi;   // high 32 bits
    uint32_t recenter_seq;  // v3: changes on each recenter request
    RrOverlay overlays[RACE_REFINERY_VR_MAX_OVERLAYS];
    RrSnapshot snapshot;
} RrSharedBlock;

#ifdef __cplusplus
}  // extern "C"
#endif

#endif  // RACE_REFINERY_VR_SHM_H
