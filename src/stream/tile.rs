#[derive(Clone, Debug, Default)]
pub struct TileFeatureRefs {
    pub buildings: Vec<usize>,
    pub roads: Vec<usize>,
    pub railways: Vec<usize>,
    pub waters: Vec<usize>,
    pub landuses: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileCoord {
    pub x: i32,
    pub z: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileRect {
    pub min_x: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_z: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileAabb {
    pub min: glam::Vec3,
    pub max: glam::Vec3,
}

impl TileCoord {
    pub fn from_world(x: f32, z: f32, tile_size: f32) -> Self {
        Self {
            x: (x / tile_size).floor() as i32,
            z: (z / tile_size).floor() as i32,
        }
    }

    pub fn rect(self, tile_size: f32) -> TileRect {
        let min_x = self.x as f32 * tile_size;
        let min_z = self.z as f32 * tile_size;
        TileRect {
            min_x,
            min_z,
            max_x: min_x + tile_size,
            max_z: min_z + tile_size,
        }
    }

    pub fn center(self, tile_size: f32) -> glam::Vec3 {
        let r = self.rect(tile_size);
        glam::Vec3::new((r.min_x + r.max_x) * 0.5, 0.0, (r.min_z + r.max_z) * 0.5)
    }
}

impl TileRect {
    pub fn intersects_bbox(&self, min_x: f32, min_z: f32, max_x: f32, max_z: f32) -> bool {
        self.min_x <= max_x && self.max_x >= min_x && self.min_z <= max_z && self.max_z >= min_z
    }
}

pub fn tiles_for_bbox(
    min_x: f32,
    min_z: f32,
    max_x: f32,
    max_z: f32,
    tile_size: f32,
) -> Vec<TileCoord> {
    let start = TileCoord::from_world(min_x, min_z, tile_size);
    let end = TileCoord::from_world(max_x, max_z, tile_size);
    let mut out = Vec::new();
    for z in start.z..=end.z {
        for x in start.x..=end.x {
            out.push(TileCoord { x, z });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coord_from_world_handles_positive_and_negative_positions() {
        assert_eq!(
            TileCoord::from_world(0.0, 0.0, 1000.0),
            TileCoord { x: 0, z: 0 }
        );
        assert_eq!(
            TileCoord::from_world(999.9, -0.1, 1000.0),
            TileCoord { x: 0, z: -1 }
        );
        assert_eq!(
            TileCoord::from_world(-0.1, -1000.0, 1000.0),
            TileCoord { x: -1, z: -1 }
        );
    }

    #[test]
    fn bounds_are_one_tile_wide() {
        let rect = TileCoord { x: 2, z: -3 }.rect(1000.0);
        assert_eq!(rect.min_x, 2000.0);
        assert_eq!(rect.max_x, 3000.0);
        assert_eq!(rect.min_z, -3000.0);
        assert_eq!(rect.max_z, -2000.0);
    }

    #[test]
    fn bbox_to_tiles_includes_all_touched_tiles() {
        let tiles = tiles_for_bbox(900.0, -1100.0, 2100.0, 100.0, 1000.0);
        assert!(tiles.contains(&TileCoord { x: 0, z: -2 }));
        assert!(tiles.contains(&TileCoord { x: 2, z: 0 }));
        assert_eq!(tiles.len(), 9);
    }

    #[test]
    fn feature_refs_default_to_empty_vectors() {
        let refs = TileFeatureRefs::default();
        assert!(refs.buildings.is_empty());
        assert!(refs.roads.is_empty());
        assert!(refs.railways.is_empty());
        assert!(refs.waters.is_empty());
        assert!(refs.landuses.is_empty());
    }
}
