use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::RwLock;

// Embedded binary dataset (compiled directly into the binary's .rodata section)
const EMBEDDED_DATA: &[u8] = include_bytes!("../../geonames_world.dat");

const EARTH_RADIUS_KM: f64 = 6371.0088;
const DEG_TO_RAD: f64 = std::f64::consts::PI / 180.0;
const RAD_TO_DEG: f64 = 180.0 / std::f64::consts::PI;

const GRID_COLS: usize = 36;
const GRID_ROWS: usize = 18;
const GRID_CELL_SIZE: f64 = 10.0;

#[derive(Clone, Debug)]
pub struct Place {
    pub name: String,
    pub category: u8,
    pub country: [u8; 2],
    pub x: f32, // lon
    pub y: f32, // lat
    pub z: i16, // elevation
    pub pop: i32,
}

impl Place {
    pub fn distance_to_km(&self, lon: f64, lat: f64) -> f64 {
        let lat1 = (self.y as f64) * DEG_TO_RAD;
        let lat2 = lat * DEG_TO_RAD;
        let dlat = (lat - self.y as f64) * DEG_TO_RAD;
        let dlon = (lon - self.x as f64) * DEG_TO_RAD;

        let sin_dlat = (dlat * 0.5).sin();
        let sin_dlon = (dlon * 0.5).sin();

        let a = sin_dlat * sin_dlat + lat1.cos() * lat2.cos() * sin_dlon * sin_dlon;
        let a = a.clamp(0.0, 1.0);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
        EARTH_RADIUS_KM * c
    }

    pub fn bearing_to(&self, lon: f64, lat: f64) -> f64 {
        let lat1 = (self.y as f64) * DEG_TO_RAD;
        let lat2 = lat * DEG_TO_RAD;
        let dlon = (lon - self.x as f64) * DEG_TO_RAD;

        let y = dlon.sin() * lat2.cos();
        let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * dlon.cos();
        let mut b = y.atan2(x) * RAD_TO_DEG;
        while b < 0.0 {
            b += 360.0;
        }
        while b >= 360.0 {
            b -= 360.0;
        }
        b
    }

    pub fn compass_direction_to(&self, lon: f64, lat: f64) -> &'static str {
        let b = self.bearing_to(lon, lat);
        if b >= 337.5 || b < 22.5 {
            "North"
        } else if b < 67.5 {
            "North-East"
        } else if b < 112.5 {
            "East"
        } else if b < 157.5 {
            "South-East"
        } else if b < 202.5 {
            "South"
        } else if b < 247.5 {
            "South-West"
        } else if b < 292.5 {
            "West"
        } else {
            "North-West"
        }
    }

    pub fn country_str(&self) -> String {
        String::from_utf8_lossy(&self.country).to_string()
    }

    pub fn format_row(&self, query_lon: Option<f64>, query_lat: Option<f64>) -> String {
        let cc = self.country_str();
        if let (Some(qlon), Some(qlat)) = (query_lon, query_lat) {
            let dist = self.distance_to_km(qlon, qlat);
            let bearing = self.bearing_to(qlon, qlat);
            let dir = self.compass_direction_to(qlon, qlat);
            format!(
                "{}\t{}\t{}\t{:.4}\t{:.4}\t{}\t{}\t{:.4}\t{:.2}\t{}",
                self.name, self.category, cc, self.x, self.y, self.z, self.pop, dist, bearing, dir
            )
        } else {
            format!(
                "{}\t{}\t{}\t{:.4}\t{:.4}\t{}\t{}",
                self.name, self.category, cc, self.x, self.y, self.z, self.pop
            )
        }
    }
}

pub struct GeoMap {
    pub places: Vec<Place>,
    pub grid: Vec<Vec<u32>>,
}

impl GeoMap {
    pub fn new() -> Self {
        GeoMap {
            places: Vec::new(),
            grid: vec![Vec::new(); GRID_COLS * GRID_ROWS],
        }
    }

    fn cell_id(lon: f64, lat: f64) -> usize {
        let mut col = ((lon + 180.0) / GRID_CELL_SIZE) as isize;
        let mut row = ((lat + 90.0) / GRID_CELL_SIZE) as isize;
        col = col.clamp(0, (GRID_COLS - 1) as isize);
        row = row.clamp(0, (GRID_ROWS - 1) as isize);
        (row as usize) * GRID_COLS + (col as usize)
    }

    pub fn load_from_slice(&mut self, data: &[u8]) -> Result<usize, String> {
        if data.len() < 10 {
            return Err("Binary data too small".into());
        }
        if &data[0..4] != b"GNMP" {
            return Err("Invalid magic header, expected GNMP".into());
        }

        let version = u16::from_le_bytes([data[4], data[5]]);
        if version != 1 {
            return Err(format!("Unsupported version: {version}"));
        }

        let total_count = u32::from_le_bytes([data[6], data[7], data[8], data[9]]) as usize;
        self.places.clear();
        self.places.reserve(total_count);

        for bucket in &mut self.grid {
            bucket.clear();
        }

        let mut offset = 10;
        let data_len = data.len();

        while offset < data_len {
            let name_len = data[offset] as usize;
            offset += 1;
            if offset + name_len > data_len {
                break;
            }
            let name = String::from_utf8_lossy(&data[offset..offset + name_len]).to_string();
            offset += name_len;

            // Record layout: category(1) + country(2) + x(4) + y(4) + z(2) + pop(4) = 17 bytes
            if offset + 17 > data_len {
                break;
            }
            let category = data[offset];
            let country = [data[offset + 1], data[offset + 2]];
            let x = f32::from_le_bytes(data[offset + 3..offset + 7].try_into().unwrap());
            let y = f32::from_le_bytes(data[offset + 7..offset + 11].try_into().unwrap());
            let z = i16::from_le_bytes(data[offset + 11..offset + 13].try_into().unwrap());
            let pop = i32::from_le_bytes(data[offset + 13..offset + 17].try_into().unwrap());
            offset += 17;

            let idx = self.places.len() as u32;
            let place = Place {
                name,
                category,
                country,
                x,
                y,
                z,
                pop,
            };

            let cell = Self::cell_id(x as f64, y as f64);
            self.grid[cell].push(idx);
            self.places.push(place);
        }

        Ok(self.places.len())
    }

    pub fn load_file(&mut self, path: &str) -> Result<usize, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("IO error: {e}"))?;
        self.load_from_slice(&bytes)
    }

    pub fn find_nearest(&self, lon: f64, lat: f64, cat_filter: u8) -> Option<usize> {
        if self.places.is_empty() {
            return None;
        }

        let center_col = (((lon + 180.0) / GRID_CELL_SIZE) as isize).clamp(0, (GRID_COLS - 1) as isize);
        let center_row = (((lat + 90.0) / GRID_CELL_SIZE) as isize).clamp(0, (GRID_ROWS - 1) as isize);

        let mut best_idx: Option<usize> = None;
        let mut best_dist = f64::MAX;

        for ring in 0..=4isize {
            let mut found_in_ring = false;
            for r in (center_row - ring)..=(center_row + ring) {
                if r < 0 || r >= GRID_ROWS as isize {
                    continue;
                }
                for c in (center_col - ring)..=(center_col + ring) {
                    let mut wrapped_c = c;
                    while wrapped_c < 0 {
                        wrapped_c += GRID_COLS as isize;
                    }
                    while wrapped_c >= GRID_COLS as isize {
                        wrapped_c -= GRID_COLS as isize;
                    }

                    if ring > 0
                        && r != center_row - ring
                        && r != center_row + ring
                        && c != center_col - ring
                        && c != center_col + ring
                    {
                        continue;
                    }

                    let cell = (r as usize) * GRID_COLS + (wrapped_c as usize);
                    for &idx in &self.grid[cell] {
                        let p = &self.places[idx as usize];
                        if cat_filter != 0 && p.category != cat_filter {
                            continue;
                        }
                        let d = p.distance_to_km(lon, lat);
                        if d < best_dist {
                            best_dist = d;
                            best_idx = Some(idx as usize);
                            found_in_ring = true;
                        }
                    }
                }
            }

            if found_in_ring && best_dist < (ring as f64 * GRID_CELL_SIZE * 111.0) {
                break;
            }
        }

        if best_idx.is_none() {
            for (i, p) in self.places.iter().enumerate() {
                if cat_filter != 0 && p.category != cat_filter {
                    continue;
                }
                let d = p.distance_to_km(lon, lat);
                if d < best_dist {
                    best_dist = d;
                    best_idx = Some(i);
                }
            }
        }

        best_idx
    }

    pub fn find_in_radius(
        &self,
        lon: f64,
        lat: f64,
        radius_km: f64,
        cat_filter: u8,
        max_results: usize,
    ) -> Vec<usize> {
        let mut results = Vec::new();
        if self.places.is_empty() || radius_km <= 0.0 {
            return results;
        }

        let degree_span = ((radius_km / 111.0) as isize) + 1;
        let center_col = (((lon + 180.0) / GRID_CELL_SIZE) as isize).clamp(0, (GRID_COLS - 1) as isize);
        let center_row = (((lat + 90.0) / GRID_CELL_SIZE) as isize).clamp(0, (GRID_ROWS - 1) as isize);

        for r in (center_row - degree_span)..=(center_row + degree_span) {
            if r < 0 || r >= GRID_ROWS as isize {
                continue;
            }
            for c in (center_col - degree_span)..=(center_col + degree_span) {
                let mut wrapped_c = c;
                while wrapped_c < 0 {
                    wrapped_c += GRID_COLS as isize;
                }
                while wrapped_c >= GRID_COLS as isize {
                    wrapped_c -= GRID_COLS as isize;
                }

                let cell = (r as usize) * GRID_COLS + (wrapped_c as usize);
                for &idx in &self.grid[cell] {
                    let p = &self.places[idx as usize];
                    if cat_filter != 0 && p.category != cat_filter {
                        continue;
                    }
                    if p.distance_to_km(lon, lat) <= radius_km {
                        results.push(idx as usize);
                        if max_results > 0 && results.len() >= max_results {
                            return results;
                        }
                    }
                }
            }
        }
        results
    }

    pub fn get_by_name(&self, name: &str, country: &str) -> Option<usize> {
        let target = name.to_lowercase();
        let target_cc = country.to_uppercase();
        let mut best: Option<usize> = None;
        let mut best_pop = i32::MIN;

        for (i, p) in self.places.iter().enumerate() {
            if p.name.to_lowercase() == target {
                if !target_cc.is_empty() && p.country_str() != target_cc {
                    continue;
                }
                if best.is_none() || p.pop > best_pop {
                    best = Some(i);
                    best_pop = p.pop;
                }
            }
        }
        best
    }

    pub fn search(&self, query: &str, cat_filter: u8, max_results: usize) -> Vec<usize> {
        let q = query.to_lowercase();
        let mut results = Vec::new();
        if q.is_empty() {
            return results;
        }

        for (i, p) in self.places.iter().enumerate() {
            if cat_filter != 0 && p.category != cat_filter {
                continue;
            }
            if p.name.to_lowercase().contains(&q) {
                results.push(i);
                if max_results > 0 && results.len() >= max_results {
                    break;
                }
            }
        }
        results
    }
}

// Global state
static MAP: RwLock<Option<GeoMap>> = RwLock::new(None);

thread_local! {
    static RETURN_BUFFER: RefCell<CString> = RefCell::new(CString::default());
}

fn set_return_string(s: &str) -> *const c_char {
    RETURN_BUFFER.with(|buf| {
        let c_str = CString::new(s).unwrap_or_else(|_| CString::new("").unwrap());
        *buf.borrow_mut() = c_str;
        buf.borrow().as_ptr()
    })
}

/// Automatically ensures the map is loaded in memory using the embedded dataset.
fn ensure_initialized() -> bool {
    let global_read = MAP.read().unwrap();
    if global_read.is_some() {
        return true;
    }
    drop(global_read);

    let mut global_write = MAP.write().unwrap();
    if global_write.is_none() {
        let mut map = GeoMap::new();
        match map.load_from_slice(EMBEDDED_DATA) {
            Ok(_) => {
                *global_write = Some(map);
                true
            }
            Err(_) => false,
        }
    } else {
        true
    }
}

// C ABI Exports

/// Initialize the map. If custom_path is null or empty, uses the EMBEDDED data automatically.
#[unsafe(no_mangle)]
pub extern "C" fn geo_init(custom_path_ptr: *const c_char) -> bool {
    if custom_path_ptr.is_null() {
        return ensure_initialized();
    }
    let c_str = unsafe { CStr::from_ptr(custom_path_ptr) };
    let path = match c_str.to_str() {
        Ok(s) => s.trim(),
        Err(_) => return ensure_initialized(),
    };

    if path.is_empty() {
        return ensure_initialized();
    }

    let mut map = GeoMap::new();
    match map.load_file(path) {
        Ok(_) => {
            let mut global = MAP.write().unwrap();
            *global = Some(map);
            true
        }
        Err(_) => false,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn geo_count() -> u32 {
    ensure_initialized();
    let global = MAP.read().unwrap();
    global.as_ref().map(|m| m.places.len() as u32).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn geo_find_nearest(lon: f64, lat: f64, cat_filter: i32) -> *const c_char {
    ensure_initialized();
    let global = MAP.read().unwrap();
    let map = match global.as_ref() {
        Some(m) => m,
        None => return set_return_string(""),
    };

    if let Some(idx) = map.find_nearest(lon, lat, cat_filter as u8) {
        let row = map.places[idx].format_row(Some(lon), Some(lat));
        set_return_string(&row)
    } else {
        set_return_string("")
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn geo_get_by_name(name_ptr: *const c_char, country_ptr: *const c_char) -> *const c_char {
    ensure_initialized();
    let global = MAP.read().unwrap();
    let map = match global.as_ref() {
        Some(m) => m,
        None => return set_return_string(""),
    };

    let name = if !name_ptr.is_null() {
        unsafe { CStr::from_ptr(name_ptr).to_str().unwrap_or("") }
    } else {
        ""
    };

    let country = if !country_ptr.is_null() {
        unsafe { CStr::from_ptr(country_ptr).to_str().unwrap_or("") }
    } else {
        ""
    };

    if let Some(idx) = map.get_by_name(name, country) {
        let row = map.places[idx].format_row(None, None);
        set_return_string(&row)
    } else {
        set_return_string("")
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn geo_search(query_ptr: *const c_char, cat_filter: i32, max_results: i32) -> *const c_char {
    ensure_initialized();
    let global = MAP.read().unwrap();
    let map = match global.as_ref() {
        Some(m) => m,
        None => return set_return_string(""),
    };

    let query = if !query_ptr.is_null() {
        unsafe { CStr::from_ptr(query_ptr).to_str().unwrap_or("") }
    } else {
        ""
    };

    let indices = map.search(query, cat_filter as u8, max_results as usize);
    let mut out = String::new();
    for (i, &idx) in indices.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&map.places[idx].format_row(None, None));
    }
    set_return_string(&out)
}

#[unsafe(no_mangle)]
pub extern "C" fn geo_find_in_radius(
    lon: f64,
    lat: f64,
    radius_km: f64,
    cat_filter: i32,
    max_results: i32,
) -> *const c_char {
    ensure_initialized();
    let global = MAP.read().unwrap();
    let map = match global.as_ref() {
        Some(m) => m,
        None => return set_return_string(""),
    };

    let indices = map.find_in_radius(lon, lat, radius_km, cat_filter as u8, max_results as usize);
    let mut out = String::new();
    for (i, &idx) in indices.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&map.places[idx].format_row(Some(lon), Some(lat)));
    }
    set_return_string(&out)
}
