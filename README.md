# nvgt_geoname_map

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Release](https://github.com/ashu-choudhury/nvgt_geoname_map/actions/workflows/release.yml/badge.svg)](https://github.com/ashu-choudhury/nvgt_geoname_map/actions/workflows/release.yml)

A high-performance, **100% offline**, zero-configuration geographic map engine and plugin for **NVGT** (NonVisual Gaming Toolkit). Powered by a native **Rust spatial core** with the entire **global dataset embedded directly into binary memory**.

---

## Table of Contents

- [What is This?](#what-is-this)
- [Why Does It Exist?](#why-does-it-exist)
- [Key Features](#key-features)
- [Performance Benchmarks](#performance-benchmarks)
- [Quick Start](#quick-start)
- [API Reference](#api-reference)
  - [`geoname_map` Class](#geoname_map-class)
  - [`geoname_place` Object](#geoname_place-object)
- [3D Spatial Audio & Local ENU Coordinates](#3d-spatial-audio--local-enu-coordinates)
- [Cross-Platform & Android Compatibility](#cross-platform--android-compatibility)
- [GitHub Releases & Automated Builds](#github-releases--automated-builds)
- [Building from Source](#building-from-source)
- [License](#license)

---

## What is This?

`nvgt_geoname_map` is an offline geographic discovery, reverse geocoding, and spatial navigation library designed specifically for NVGT audio games and interactive simulations. 

It equips audio games with rich, real-world geography across Earth:
- Over **239,135 locations** worldwide.
- **All Countries** (~252 sovereign nations and territories with ISO codes, capitals, and populations).
- **All Oceans, Seas, Gulfs, and Bays** (Pacific, Atlantic, Indian, Arctic, Southern, Mediterranean, Caribbean, etc.).
- **Over 235,000 Populated Places** (Cities, towns, and villages down to 500 inhabitants from GeoNames `cities500`).
- **Mountains and Summits** with elevation (Mount Everest, K2, Mont Blanc, Mount Fuji, etc.).
- **Lakes and Reservoirs** (Lake Baikal, Lake Superior, Caspian Sea, etc.).
- **Rivers and Watercourses** (Nile, Amazon, Yangtze, Mississippi, Danube, etc.).
- **True 3D Coordinates**: Every place includes Longitude ($X$), Latitude ($Y$), and Elevation in meters ($Z$).

---

## Why Does It Exist?

Developing real-world geographic features for audio games has historically been painful:

1. **Online APIs Are Unusable in Offline / Real-Time Games:**
   - Web services (Google Maps, OpenStreetMap Nominatim, GeoNames REST API) require continuous internet connectivity.
   - Network requests add 100–500 ms latency per query, ruining real-time game loops.
   - API quotas, rate-limiting, and API keys create fragility and maintenance burden.

2. **Raw GIS Datasets Are Gigantic and Impractical:**
   - Full GIS shapefiles and raw GeoNames database dumps exceed **80 GB** or gigabytes of uncompressed text.
   - Parsing huge CSV or TSV files in AngelScript or Python takes seconds to minutes and consumes hundreds of megabytes of RAM.

3. **Loose Files Break on Android and Mobile Sandboxes:**
   - Loading external `.dat` or `.csv` files relative to executable paths fails on Android APK apps because assets are zipped inside the APK and require complex native `AAssetManager` hooks.

### The Solution:
`nvgt_geoname_map` solves all three issues simultaneously:
- **Packed Binary Format:** All 239,135 locations are filtered, deduplicated, and packed into a tiny **6.35 MB binary** using variable-length strings and fixed 32-bit float coordinates.
- **100% Embedded in Memory (`include_bytes!`):** The dataset is compiled directly into the Rust shared library (`.rodata`). It requires **zero file system calls, zero runtime file permissions, and zero loose files**.
- **Instant Cold Start:** Initialized and spatially indexed in **~34 milliseconds**.
- **Sub-Millisecond Spatial Queries:** Uses a 2D spatial grid index ($10^\circ \times 10^\circ$ buckets) in native memory, executing reverse geocoding and radius searches in **~1–4 milliseconds**.

---

## Performance Benchmarks

| Metric | Loose Text / TSV | `nvgt_geoname_map` (Rust Embedded) | Speedup |
| :--- | :--- | :--- | :--- |
| **Data Size** | ~35 MB uncompressed text | **6.35 MB packed binary** | **5.5x smaller** |
| **Startup / Index Time** | ~3,600 ms | **~34 ms** | **105x faster** |
| **Nearest Place Query** | ~45 ms | **~4 ms** | **11x faster** |
| **Runtime File Dependencies** | Multiple loose files | **0 (100% embedded in binary)** | **Zero I/O** |
| **Android APK Sandbox Ready** | No (fails on loose path) | **Yes (in-memory `.rodata`)** | **100% Native** |

---

## Quick Start

### 1. Download or Place the Binary
Drop `nvgt_geoname_map.dll` (Windows) or `libnvgt_geoname_map.so` (Linux/Android) into your project folder.

### 2. Include in AngelScript

```angelscript
#include "geoname_map.nvgt"

void main() {
    geoname_map map;

    // Zero-parameter initialization: automatically loads native library and embedded dataset!
    if (!map.load()) {
        alert("Error", "Could not initialize geographic map engine!");
        return;
    }

    println("Successfully loaded " + map.count() + " locations on Earth.");

    // 1. Reverse Geocoding (Coordinates -> Nearest Real-World Place)
    // Coordinates near New Delhi: 77.2090 (X/Lon), 28.6139 (Y/Lat)
    geoname_place@ place = map.find_nearest(77.2090, 28.6139);
    if (@place != null) {
        println("Found: " + place.name + " (" + place.get_category_name() + "), Country: " + place.country);
        println("Elevation (Z): " + place.z + " meters");
        println("Distance: " + place.distance_to_km(77.2090, 28.6139) + " km");
        println("Compass Bearing: " + place.compass_direction_to(77.2090, 28.6139));
    }

    // 2. Direct Lookup by Name (Mountains, Lakes, Rivers, Oceans, Cities)
    geoname_place@ everest = map.get_by_name("Mount Everest");
    if (@everest != null) {
        println("Mount Everest Summit: " + everest.z + "m elevation at (" + everest.x + ", " + everest.y + ")");
    }

    // 3. Forward Geocoding / Settlement Search
    geoname_place@[] settlements = map.search("Springfield");
    for (uint i = 0; i < settlements.length() and i < 5; i++) {
        println("- " + settlements[i].name + " (" + settlements[i].country + ", pop: " + settlements[i].population + ")");
    }

    // 4. Radius Search: All settlements within 50 km of London
    geoname_place@[] nearby = map.find_in_radius(-0.1276, 51.5074, 50.0);
    println("Places within 50 km: " + nearby.length());
}
```

---

## API Reference

### `geoname_map` Class

| Method | Description |
| :--- | :--- |
| `bool load(const string& in dll_path = "")` | Loads the native shared library and initializes the embedded dataset. Pass `""` for automatic zero-configuration discovery. |
| `void unload()` | Unloads the native library from memory. |
| `bool get_loaded() const` | Returns `true` if the engine is loaded and initialized. |
| `uint count()` | Total number of geographic locations loaded in the engine (239,135). |
| `geoname_place@ find_nearest(double x_lon, double y_lat)` | Finds the nearest real-world place to the given longitude and latitude. |
| `geoname_place@ find_nearest_xyz(double x, double y, double z)` | Finds the nearest place considering 3D elevation. |
| `geoname_place@ get_by_name(const string& in name, const string& in country = "")` | Looks up a place by exact or case-insensitive name, optionally filtered by 2-letter country code. |
| `geoname_place@[] search(const string& in query, uint max_results = 20)` | Searches for settlements whose names contain the query string. |
| `geoname_place@[] find_in_radius(double x_lon, double y_lat, double radius_km, uint max_results = 50)` | Returns all locations within the specified radius in kilometers. |

---

### `geoname_place` Object

| Property / Method | Type | Description |
| :--- | :--- | :--- |
| `name` | `string` | Name of the location (e.g., `"Tokyo"`, `"Mount Everest"`, `"Pacific Ocean"`). |
| `category` | `int` | Category ID: `0`=Country, `1`=Ocean, `2`=Sea, `3`=City, `4`=Town, `5`=Village, `6`=Mountain, `7`=Lake, `8`=River. |
| `get_category_name()` | `string` | Human-readable string representation of the category. |
| `country` | `string` | 2-letter ISO country code (`"US"`, `"IN"`, `"JP"`, `"--"` for international waters). |
| `x` | `double` | Longitude in degrees ($-180.0^\circ$ to $+180.0^\circ$). |
| `y` | `double` | Latitude in degrees ($-90.0^\circ$ to $+90.0^\circ$). |
| `z` | `double` | Elevation / altitude in meters above sea level. |
| `population` | `int` | Recorded population for settlements and countries. |
| `distance_to_km(lon, lat)` | `double` | Great-circle spherical distance to target coordinates in kilometers. |
| `bearing_to(lon, lat)` | `double` | True compass bearing in degrees ($0^\circ - 360^\circ$). |
| `compass_direction_to(lon, lat)` | `string` | 8-point compass cardinal direction (`"North"`, `"South-East"`, `"West"`, etc.). |
| `to_local_xyz(origin_lon, origin_lat, origin_alt)` | `vector` | Converts spherical geographic coordinates into local Cartesian East-North-Up (ENU) meters for 3D game audio. |

---

## 3D Spatial Audio & Local ENU Coordinates

In audio games built with NVGT, sound sources are placed in 3D Cartesian coordinates (East = $X$, North = $Y$, Up = $Z$ in meters).

The `to_local_xyz()` method projects spherical Earth coordinates into local flat tangent plane coordinates relative to the player's reference position:

```angelscript
// Player is standing in London (-0.1276 lon, 51.5074 lat, 15m elevation)
geoname_place@ paris = map.get_by_name("Paris");
if (@paris != null) {
    // Calculate local (X, Y, Z) in meters relative to London
    vector rel_pos = paris.to_local_xyz(-0.1276, 51.5074, 15.0);

    // Feed directly to NVGT's 3D sound positioning
    sound snd;
    snd.load("ambient_city.ogg");
    // Position: X=East, Y=North, Z=Up
    snd.position = rel_pos;
    snd.play_3d(rel_pos.x, rel_pos.y, rel_pos.z);
}
```

---

## Cross-Platform & Android Compatibility

Because the 6.35 MB binary dataset is embedded into the shared library (`.rodata` segment), `nvgt_geoname_map` is 100% compatible with mobile and sandboxed environments:

- **Desktop (Windows, Linux, macOS):** Simply place the compiled dynamic library alongside your executable or script.
- **Android (`.apk`):** In Android APKs, loose filesystem paths fail because assets are compressed inside the APK file. With `nvgt_geoname_map`, place `libnvgt_geoname_map.so` in your APK's `jniLibs/arm64-v8a/` and `jniLibs/armeabi-v7a/`. Android's dynamic linker loads the `.so` straight into memory, and the embedded data is accessed **with zero filesystem permissions or file reading calls**.

---

## GitHub Releases & Automated Builds

Every commit pushed to the repository automatically triggers the GitHub Actions CI/CD release workflow (`.github/workflows/release.yml`), compiling native binaries across all platforms:

- **`nvgt_geoname_map-windows-x64.zip`**: Windows 64-bit DLL (`nvgt_geoname_map.dll`)
- **`nvgt_geoname_map-android.zip`**: Android native libraries (`arm64-v8a`, `armeabi-v7a`, `x86_64`)
- **`nvgt_geoname_map-linux-x64.zip`**: Linux 64-bit shared library (`libnvgt_geoname_map.so`)
- **`nvgt_geoname_map-macos-universal.zip`**: macOS Universal dynamic library (`libnvgt_geoname_map.dylib` Intel + Apple Silicon)
- **`nvgt_geoname_map-all-platforms.zip`**: Comprehensive package including all prebuilt binaries.

---

## Building from Source

### Prerequisites
- [Rust toolchain](https://rustup.rs/) (1.80+ or latest stable)
- Python 3.10+ (only required if rebuilding the raw dataset from GeoNames)

### Compile Rust Shared Library
```powershell
cd rust_core
cargo build --release
copy target\release\nvgt_geoname_map.dll ..\
cd ..
```

### Compile for Android
```bash
cargo install cargo-ndk
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cd rust_core
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o ../android_libs build --release
cd ..
```

### Run Tests in NVGT
```powershell
nvgt test_geoname_map.nvgt
```

---

## License

This project is licensed under the **MIT License**. See the [LICENSE](LICENSE) file for details.
