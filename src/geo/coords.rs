const METRES_PER_DEG_LAT: f64 = 111_320.0;

#[derive(Clone, Copy, Debug)]
pub struct CoordConverter {
    pub origin_lat: f64,
    pub origin_lon: f64,
}

impl CoordConverter {
    pub fn new(origin_lat: f64, origin_lon: f64) -> Self {
        Self {
            origin_lat,
            origin_lon,
        }
    }

    pub fn to_world_xz(&self, lat: f64, lon: f64) -> (f32, f32) {
        let metres_per_deg_lon = METRES_PER_DEG_LAT * self.origin_lat.to_radians().cos();
        let x = ((lon - self.origin_lon) * metres_per_deg_lon) as f32;
        let z = -((lat - self.origin_lat) * METRES_PER_DEG_LAT) as f32;
        (x, z)
    }

    pub fn world_xz_to_lat_lon(&self, x: f32, z: f32) -> (f64, f64) {
        let metres_per_deg_lon = METRES_PER_DEG_LAT * self.origin_lat.to_radians().cos();
        let lat = self.origin_lat - (z as f64) / METRES_PER_DEG_LAT;
        let lon = self.origin_lon + (x as f64) / metres_per_deg_lon;
        (lat, lon)
    }

    pub fn bbox_centre(
        &self,
        min_lat: f64,
        min_lon: f64,
        max_lat: f64,
        max_lon: f64,
    ) -> (f32, f32) {
        self.to_world_xz((min_lat + max_lat) / 2.0, (min_lon + max_lon) / 2.0)
    }

    pub fn bbox_world_size(
        &self,
        min_lat: f64,
        min_lon: f64,
        max_lat: f64,
        max_lon: f64,
    ) -> (f32, f32) {
        let metres_per_deg_lon = METRES_PER_DEG_LAT * self.origin_lat.to_radians().cos();
        let width = ((max_lon - min_lon) * metres_per_deg_lon) as f32;
        let depth = ((max_lat - min_lat) * METRES_PER_DEG_LAT) as f32;
        (width.abs(), depth.abs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_xz_to_lat_lon_round_trips_world_conversion() {
        let conv = CoordConverter::new(38.63863, -121.7526);
        let expected_lat = 38.65671;
        let expected_lon = -121.72179;

        let (x, z) = conv.to_world_xz(expected_lat, expected_lon);
        let (lat, lon) = conv.world_xz_to_lat_lon(x, z);

        assert!((lat - expected_lat).abs() < 1e-7, "lat = {lat}");
        assert!((lon - expected_lon).abs() < 1e-7, "lon = {lon}");
    }
}
