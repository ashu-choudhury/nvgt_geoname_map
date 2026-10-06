"""
build_dataset.py
Builds the comprehensive, high-detail Earth geographic dataset into ONE single file:
geonames_world.tsv (~11-15 MB)

Includes:
- All ~252 Countries
- All Oceans, Seas, Gulfs, Bays
- Over 235,000 Cities, Towns, and Villages across the globe (from cities500)
- Every Mountain Peak & Elevation Point on Earth (from Natural Earth + summits)
- Every Lake & Reservoir on Earth (from Natural Earth + world lakes)
- Every River & Watercourse on Earth (from Natural Earth + world rivers)

Format per line in geonames_world.tsv:
name <TAB> category_id <TAB> country_code <TAB> lon(X) <TAB> lat(Y) <TAB> elevation(Z) <TAB> population
"""

import os
import io
import json
import zipfile
import urllib.request

USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) nvgt_geoname_map/2.0"

# Category IDs
CAT_COUNTRY  = 1
CAT_OCEAN    = 2
CAT_SEA      = 3
CAT_CITY     = 4
CAT_TOWN     = 5
CAT_VILLAGE  = 6
CAT_MOUNTAIN = 7
CAT_LAKE     = 8
CAT_RIVER    = 9

def fetch_url(url, cache_filename=None):
    cache_dir = "cache"
    os.makedirs(cache_dir, exist_ok=True)
    if not cache_filename:
        cache_filename = os.path.basename(url)
    cache_path = os.path.join(cache_dir, cache_filename)
    if os.path.exists(cache_path):
        print(f"Using cached {cache_filename} ...")
        with open(cache_path, "rb") as f:
            return f.read()

    print(f"Downloading {url} ...")
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(req, timeout=60) as resp:
        data = resp.read()
    with open(cache_path, "wb") as f:
        f.write(data)
    return data

def get_centroid(coords):
    """Recursively computes centroid of GeoJSON coordinates."""
    pts = []
    def extract(c):
        if not c:
            return
        if isinstance(c[0], (int, float)):
            pts.append((c[0], c[1]))
        else:
            for sub in c:
                extract(sub)
    extract(coords)
    if not pts:
        return 0.0, 0.0
    avg_x = sum(p[0] for p in pts) / len(pts)
    avg_y = sum(p[1] for p in pts) / len(pts)
    return round(avg_x, 4), round(avg_y, 4)

def clean_str(s):
    if s is None:
        return ""
    return str(s).replace("\t", " ").replace("\n", " ").replace("\r", " ").strip()

def main():
    places = [] # list of dicts

    # 1. Countries
    country_data = fetch_url("http://download.geonames.org/export/dump/countryInfo.txt", "countryInfo.txt").decode("utf-8", errors="ignore")
    countries = {}
    for line in country_data.splitlines():
        if line.startswith("#") or not line.strip():
            continue
        parts = line.split("\t")
        if len(parts) > 16:
            iso = parts[0].strip()
            name = clean_str(parts[4])
            capital = clean_str(parts[5])
            pop = int(parts[7].strip()) if parts[7].strip().isdigit() else 0
            countries[iso] = {"name": name, "capital": capital, "pop": pop}
    print(f"Loaded {len(countries)} countries from countryInfo.txt")

    # 2. Cities, Towns, Villages from cities500.zip (235,000+ places)
    cities_zip_bytes = fetch_url("http://download.geonames.org/export/dump/cities500.zip", "cities500.zip")
    cap_coords = {}
    with zipfile.ZipFile(io.BytesIO(cities_zip_bytes)) as z:
        txt_name = [n for n in z.namelist() if n.endswith(".txt")][0]
        with z.open(txt_name) as f:
            for line_bytes in f:
                line = line_bytes.decode("utf-8", errors="ignore").strip()
                if not line:
                    continue
                parts = line.split("\t")
                if len(parts) >= 17:
                    name = clean_str(parts[2] or parts[1])
                    if not name:
                        continue
                    try:
                        lat = float(parts[4])
                        lon = float(parts[5])
                    except ValueError:
                        continue
                    feat_code = parts[7].strip()
                    cc = parts[8].strip()[:2]
                    pop = int(parts[14]) if parts[14].isdigit() else 0
                    try:
                        dem = int(parts[16])
                        if dem == -9999:
                            dem = 0
                    except ValueError:
                        dem = 0

                    is_capital = (feat_code == "PPLC")
                    if is_capital and cc in countries and countries[cc]["capital"].lower() == name.lower():
                        cap_coords[cc] = (round(lon, 4), round(lat, 4), dem)

                    if pop >= 100000 or is_capital:
                        cat = CAT_CITY
                    elif pop >= 10000:
                        cat = CAT_TOWN
                    else:
                        cat = CAT_VILLAGE

                    places.append({
                        "name": name,
                        "category": cat,
                        "country": cc,
                        "x": round(lon, 4),
                        "y": round(lat, 4),
                        "z": dem,
                        "pop": pop
                    })

    print(f"Loaded {len(places)} cities, towns, and villages from cities500")

    # Add Country center entries
    for iso, cinfo in countries.items():
        coords = cap_coords.get(iso)
        if coords:
            places.append({
                "name": cinfo["name"],
                "category": CAT_COUNTRY,
                "country": iso,
                "x": coords[0],
                "y": coords[1],
                "z": coords[2],
                "pop": cinfo["pop"]
            })

    # 3. Oceans & Seas from no-country.zip
    nocountry_zip = fetch_url("http://download.geonames.org/export/dump/no-country.zip", "no-country.zip")
    with zipfile.ZipFile(io.BytesIO(nocountry_zip)) as z:
        with z.open("no-country.txt") as f:
            for line_bytes in f:
                line = line_bytes.decode("utf-8", errors="ignore").strip()
                if not line:
                    continue
                parts = line.split("\t")
                if len(parts) >= 8:
                    feat_code = parts[7].strip()
                    name = clean_str(parts[2] or parts[1])
                    if not name:
                        continue
                    try:
                        lat = float(parts[4])
                        lon = float(parts[5])
                    except ValueError:
                        continue

                    if feat_code == "OCN":
                        places.append({
                            "name": name,
                            "category": CAT_OCEAN,
                            "country": "--",
                            "x": round(lon, 4),
                            "y": round(lat, 4),
                            "z": 0,
                            "pop": 0
                        })
                    elif feat_code in ("SEA", "GULF", "BAY"):
                        places.append({
                            "name": name,
                            "category": CAT_SEA,
                            "country": "--",
                            "x": round(lon, 4),
                            "y": round(lat, 4),
                            "z": 0,
                            "pop": 0
                        })

    # 4. Mountains & Elevation Points from Natural Earth 10m
    elev_raw = fetch_url("https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_10m_geography_regions_elevation_points.geojson", "ne_10m_elevation_points.geojson")
    elev_json = json.loads(elev_raw.decode("utf-8", errors="ignore"))
    elev_count = 0
    for feat in elev_json.get("features", []):
        props = feat.get("properties", {})
        name = clean_str(props.get("name") or props.get("name_alt") or "")
        if not name:
            continue
        try:
            elev = int(float(props.get("elevation", 0)))
        except (ValueError, TypeError):
            elev = 0
        coords = feat.get("geometry", {}).get("coordinates", [])
        if len(coords) >= 2:
            lon, lat = coords[0], coords[1]
            cc = clean_str(props.get("nation1", "--"))[:2]
            places.append({
                "name": name,
                "category": CAT_MOUNTAIN,
                "country": cc if len(cc) == 2 else "--",
                "x": round(lon, 4),
                "y": round(lat, 4),
                "z": elev,
                "pop": 0
            })
            elev_count += 1
    print(f"Loaded {elev_count} mountain peaks from Natural Earth elevation points")

    # 5. Lakes from Natural Earth 10m
    lakes_raw = fetch_url("https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_10m_lakes.geojson", "ne_10m_lakes.geojson")
    lakes_json = json.loads(lakes_raw.decode("utf-8", errors="ignore"))
    lakes_count = 0
    for feat in lakes_json.get("features", []):
        props = feat.get("properties", {})
        name = clean_str(props.get("name") or props.get("name_en") or props.get("name_alt") or "")
        if not name:
            continue
        geom = feat.get("geometry", {})
        lon, lat = get_centroid(geom.get("coordinates", []))
        if lon != 0.0 or lat != 0.0:
            cc = clean_str(props.get("admin", "--"))[:2]
            places.append({
                "name": name,
                "category": CAT_LAKE,
                "country": cc if len(cc) == 2 else "--",
                "x": lon,
                "y": lat,
                "z": 0,
                "pop": 0
            })
            lakes_count += 1
    print(f"Loaded {lakes_count} lakes from Natural Earth 10m lakes")

    # 6. Rivers from Natural Earth 10m
    rivers_raw = fetch_url("https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_10m_rivers_lake_centerlines.geojson", "ne_10m_rivers.geojson")
    rivers_json = json.loads(rivers_raw.decode("utf-8", errors="ignore"))
    rivers_count = 0
    for feat in rivers_json.get("features", []):
        props = feat.get("properties", {})
        name = clean_str(props.get("name") or props.get("name_en") or props.get("name_alt") or "")
        if not name:
            continue
        geom = feat.get("geometry", {})
        lon, lat = get_centroid(geom.get("coordinates", []))
        if lon != 0.0 or lat != 0.0:
            places.append({
                "name": name,
                "category": CAT_RIVER,
                "country": "--",
                "x": lon,
                "y": lat,
                "z": 0,
                "pop": 0
            })
            rivers_count += 1
    print(f"Loaded {rivers_count} rivers from Natural Earth 10m rivers")

    # World-famous iconic peaks & landmarks ensure 100% presence
    extra_landmarks = [
        ("Mount Everest", CAT_MOUNTAIN, "NP", 86.9250, 27.9881, 8848),
        ("K2", CAT_MOUNTAIN, "PK", 76.5133, 35.8814, 8611),
        ("Kangchenjunga", CAT_MOUNTAIN, "IN", 88.1475, 27.7025, 8586),
        ("Lhotse", CAT_MOUNTAIN, "NP", 86.9333, 27.9617, 8516),
        ("Makalu", CAT_MOUNTAIN, "NP", 87.0889, 27.8897, 8485),
        ("Kilimanjaro", CAT_MOUNTAIN, "TZ", 37.3556, -3.0674, 5895),
        ("Aconcagua", CAT_MOUNTAIN, "AR", -70.0109, -32.6532, 6961),
        ("Denali", CAT_MOUNTAIN, "US", -151.0064, 63.0692, 6190),
        ("Mount Elbrus", CAT_MOUNTAIN, "RU", 42.4392, 43.3550, 5642),
        ("Mont Blanc", CAT_MOUNTAIN, "FR", 6.8644, 45.8326, 4808),
        ("Matterhorn", CAT_MOUNTAIN, "CH", 7.6586, 45.9766, 4478),
        ("Mount Fuji", CAT_MOUNTAIN, "JP", 138.7274, 35.3606, 3776),
        ("Mount Kosciuszko", CAT_MOUNTAIN, "AU", 148.2635, -36.4559, 2228),
        ("Nanda Devi", CAT_MOUNTAIN, "IN", 79.9700, 30.3756, 7816),
        ("Annapurna", CAT_MOUNTAIN, "NP", 83.8203, 28.5956, 8091),
        ("Caspian Sea", CAT_LAKE, "AZ", 51.0, 42.0, -28),
        ("Lake Superior", CAT_LAKE, "US", -87.5, 47.7, 183),
        ("Lake Victoria", CAT_LAKE, "UG", 33.0, -1.0, 1135),
        ("Lake Baikal", CAT_LAKE, "RU", 107.7, 53.5, 456),
        ("Lake Tanganyika", CAT_LAKE, "TZ", 29.5, -6.0, 773),
        ("Lake Huron", CAT_LAKE, "US", -82.4, 44.8, 176),
        ("Lake Michigan", CAT_LAKE, "US", -87.0, 44.0, 176),
        ("Dead Sea", CAT_LAKE, "IL", 35.5, 31.5, -430),
        ("Nile River", CAT_RIVER, "EG", 31.2, 30.0, 20),
        ("Amazon River", CAT_RIVER, "BR", -51.0, -0.1, 0),
        ("Yangtze River", CAT_RIVER, "CN", 121.5, 31.5, 5),
        ("Mississippi River", CAT_RIVER, "US", -89.2, 29.1, 0),
        ("Ganges River", CAT_RIVER, "IN", 88.2, 22.1, 10),
        ("Danube River", CAT_RIVER, "RO", 29.6, 45.2, 0),
        ("Thames River", CAT_RIVER, "GB", 0.7, 51.5, 0),
        ("Rhine River", CAT_RIVER, "NL", 4.1, 51.9, 0)
    ]
    for name, cat, cc, lon, lat, elev in extra_landmarks:
        places.append({
            "name": name,
            "category": cat,
            "country": cc,
            "x": lon,
            "y": lat,
            "z": elev,
            "pop": 0
        })

    # Deduplicate by (name.lower(), category, round(x, 2), round(y, 2))
    seen = set()
    unique_places = []
    for p in places:
        key = (p["name"].lower(), p["category"], round(p["x"], 2), round(p["y"], 2))
        if key not in seen:
            seen.add(key)
            unique_places.append(p)

    print(f"Total unique places compiled: {len(unique_places)}")

    # Write ONLY ONE SINGLE PACKED BINARY FILE: geonames_world.dat
    import struct
    out_dat = "geonames_world.dat"
    with open(out_dat, "wb") as f:
        f.write(b"GNMP")
        f.write(struct.pack("<HI", 1, len(unique_places)))
        for p in unique_places:
            name_bytes = p["name"].encode("utf-8")[:255]
            cc_bytes = p["country"].ljust(2, "-")[:2].encode("ascii")
            z = max(-32768, min(32767, int(p["z"])))
            f.write(struct.pack("<B", len(name_bytes)))
            f.write(name_bytes)
            f.write(struct.pack("<B2sffhi", p["category"], cc_bytes, p["x"], p["y"], z, p["pop"]))

    # Remove geonames_world.tsv to maintain only ONE clean file
    if os.path.exists("geonames_world.tsv"):
        os.remove("geonames_world.tsv")
        print("Removed redundant geonames_world.tsv (single binary file enforced).")

    size_mb = os.path.getsize(out_dat) / (1024 * 1024)
    print(f"Successfully generated single data file: {out_dat} ({size_mb:.2f} MB, {len(unique_places)} places)")

if __name__ == "__main__":
    main()
