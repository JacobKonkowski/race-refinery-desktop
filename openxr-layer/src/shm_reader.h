// Reader half of the Race Refinery VR shared-memory contract (see race_refinery_vr_shm.h).
//
// Opens the producer's file mapping read-only and performs seqlock-protected
// copies so a frame is never composited from a half-written block.

#pragma once

#include <windows.h>

#include <atomic>
#include <cstring>

#include "race_refinery_vr_shm.h"

class ShmReader {
public:
    ShmReader() = default;

    ~ShmReader() {
        if (m_view) {
            UnmapViewOfFile(m_view);
        }
        if (m_mapping) {
            CloseHandle(m_mapping);
        }
    }

    // Try to attach to the producer's mapping. Returns false if Race Refinery has not
    // created it yet; callers should retry on a later frame.
    bool EnsureOpen() {
        if (m_view) {
            return true;
        }
        m_mapping = OpenFileMappingA(FILE_MAP_READ, FALSE, RACE_REFINERY_VR_SHM_NAME);
        if (!m_mapping) {
            return false;
        }
        m_view = static_cast<const RrSharedBlock*>(
            MapViewOfFile(m_mapping, FILE_MAP_READ, 0, 0, sizeof(RrSharedBlock)));
        if (!m_view) {
            CloseHandle(m_mapping);
            m_mapping = nullptr;
            return false;
        }
        return true;
    }

    // Copy a consistent snapshot of the block into `out`. Returns false when the
    // mapping is missing, the header is invalid, or the data is stale (no write
    // within `max_age_ms`).
    bool Read(RrSharedBlock& out, uint64_t now_ms, uint64_t max_age_ms) {
        if (!EnsureOpen()) {
            return false;
        }

        // Seqlock: read seq, copy, re-read seq. Retry a few times on contention.
        for (int attempt = 0; attempt < 8; ++attempt) {
            const uint32_t before =
                reinterpret_cast<const std::atomic<uint32_t>*>(&m_view->seq)
                    ->load(std::memory_order_acquire);
            if (before & 1u) {
                continue;  // writer mid-update
            }
            std::memcpy(&out, m_view, sizeof(RrSharedBlock));
            std::atomic_thread_fence(std::memory_order_acquire);
            const uint32_t after =
                reinterpret_cast<const std::atomic<uint32_t>*>(&m_view->seq)
                    ->load(std::memory_order_acquire);
            if (before != after) {
                continue;  // torn read, retry
            }

            if (out.magic != RACE_REFINERY_VR_MAGIC || out.version != RACE_REFINERY_VR_VERSION) {
                return false;
            }
            const uint64_t written =
                (static_cast<uint64_t>(out.write_ms_hi) << 32) | out.write_ms_lo;
            if (max_age_ms != 0 && now_ms > written && (now_ms - written) > max_age_ms) {
                return false;
            }
            return true;
        }
        return false;
    }

private:
    HANDLE m_mapping = nullptr;
    const RrSharedBlock* m_view = nullptr;
};
