#[derive(Clone, Debug, Default)]
pub struct TileFeatureRefs {
    pub buildings: Vec<usize>,
    pub roads: Vec<usize>,
    pub railways: Vec<usize>,
    pub waters: Vec<usize>,
    pub waterways: Vec<usize>,
    pub landuses: Vec<usize>,
    pub point_features: Vec<usize>,
    pub street_signs: Vec<usize>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TileDebugState {
    Queued,
    Generating,
    Uploaded,
    Visible,
    Culled,
    Evicted,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TileDebugEntry {
    pub coord: TileCoord,
    pub state: TileDebugState,
}

impl TileDebugState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Generating => "generating",
            Self::Uploaded => "uploaded",
            Self::Visible => "visible",
            Self::Culled => "culled",
            Self::Evicted => "evicted",
            Self::Failed => "failed",
        }
    }

    fn is_loaded(self) -> bool {
        matches!(self, Self::Uploaded | Self::Visible | Self::Culled)
    }
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

pub fn classify_loaded_tile_state(
    coord: TileCoord,
    tile_size: f32,
    camera_position: glam::Vec3,
    stream_radius: f32,
) -> TileDebugState {
    let center = coord.center(tile_size);
    let delta = glam::vec2(center.x - camera_position.x, center.z - camera_position.z);
    if delta.length() <= stream_radius {
        TileDebugState::Visible
    } else {
        TileDebugState::Culled
    }
}

pub fn update_loaded_tile_debug_states(
    entries: &mut [TileDebugEntry],
    tile_size: f32,
    camera_position: glam::Vec3,
    stream_radius: f32,
) {
    for entry in entries {
        if entry.state.is_loaded() {
            entry.state =
                classify_loaded_tile_state(entry.coord, tile_size, camera_position, stream_radius);
        }
    }
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
        assert!(refs.waterways.is_empty());
        assert!(refs.landuses.is_empty());
        assert!(refs.point_features.is_empty());
        assert!(refs.street_signs.is_empty());
    }

    #[test]
    fn loaded_tile_debug_state_tracks_camera_stream_radius() {
        let near = TileCoord { x: 0, z: 0 };
        let far = TileCoord { x: 3, z: 0 };

        assert_eq!(
            classify_loaded_tile_state(near, 100.0, glam::vec3(25.0, 0.0, 25.0), 150.0),
            TileDebugState::Visible
        );
        assert_eq!(
            classify_loaded_tile_state(far, 100.0, glam::vec3(25.0, 0.0, 25.0), 150.0),
            TileDebugState::Culled
        );
    }

    #[test]
    fn tile_debug_entries_keep_failure_and_generation_states_when_reclassified() {
        let mut entries = vec![
            TileDebugEntry {
                coord: TileCoord { x: 0, z: 0 },
                state: TileDebugState::Uploaded,
            },
            TileDebugEntry {
                coord: TileCoord { x: 1, z: 0 },
                state: TileDebugState::Generating,
            },
            TileDebugEntry {
                coord: TileCoord { x: 2, z: 0 },
                state: TileDebugState::Failed,
            },
        ];

        update_loaded_tile_debug_states(&mut entries, 100.0, glam::Vec3::ZERO, 120.0);

        assert_eq!(entries[0].state, TileDebugState::Visible);
        assert_eq!(entries[1].state, TileDebugState::Generating);
        assert_eq!(entries[2].state, TileDebugState::Failed);
    }
}
