# Race Refinery brand assets

Canonical visual identity for the desktop app, store listings, and docs.

## Mark

The mark is a **racing apex refined into telemetry**: a blue race-line dips through an apex (highlighted), pulses, then resolves into rising bars — raw lap data turned into pace insight.

| Token | Value |
|-------|-------|
| Accent | `#4AA3FF` |
| Accent strong | `#1F6FEB` |
| Apex highlight | `#B8E0FF` |
| Background | `#0B0E14` |
| Surface | `#12161F` |
| Text | `#E6EDF3` |
| Muted | `#8B95A5` |

## Layout

```
brand/
  source/                 # editable SVG masters
    mark.svg
    icon.svg
    logo-horizontal-*.svg
    logo-stacked-*.svg
    wordmark-*.svg
  icon-1024.png           # Tauri / store master
  icon-512.png
  mark-512.png
  logo-*.png
  promo/                  # marketing / social art
public/brand/             # runtime web assets (favicon, header mark)
src-tauri/icons/          # packaged desktop icons (generated)
```

## Regenerate

```bash
python3 scripts/build-brand-assets.py
```

Requires `librsvg2-bin` and Pillow. Promo JPEGs are copied from the agent artifact cache when present; SVG/PNG masters are always rebuilt from this script.
