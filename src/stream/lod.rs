#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TileLod {
    Near = 0,
    Mid = 1,
    Far = 2,
}

#[derive(Clone, Copy, Debug)]
pub struct LodConfig {
    pub near_to_mid: f32,
    pub mid_to_near: f32,
    pub mid_to_far: f32,
    pub far_to_mid: f32,
}

impl Default for LodConfig {
    fn default() -> Self {
        Self {
            near_to_mid: 2200.0,
            mid_to_near: 1800.0,
            mid_to_far: 5500.0,
            far_to_mid: 4500.0,
        }
    }
}

impl LodConfig {
    pub fn select(&self, distance: f32, previous: TileLod) -> TileLod {
        match previous {
            TileLod::Near if distance > self.near_to_mid => TileLod::Mid,
            TileLod::Mid if distance < self.mid_to_near => TileLod::Near,
            TileLod::Mid if distance > self.mid_to_far => TileLod::Far,
            TileLod::Far if distance < self.far_to_mid => TileLod::Mid,
            other => other,
        }
    }

    pub fn terrain_spacing(lod: TileLod) -> f32 {
        match lod {
            TileLod::Near => 10.0,
            TileLod::Mid => 50.0,
            TileLod::Far => 100.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lod_selects_by_distance() {
        let cfg = LodConfig::default();
        assert_eq!(cfg.select(1000.0, TileLod::Near), TileLod::Near);
        assert_eq!(cfg.select(3000.0, TileLod::Near), TileLod::Mid);
        assert_eq!(cfg.select(6000.0, TileLod::Mid), TileLod::Far);
    }

    #[test]
    fn lod_hysteresis_prevents_threshold_flicker() {
        let cfg = LodConfig::default();
        assert_eq!(cfg.select(2100.0, TileLod::Near), TileLod::Near);
        assert_eq!(cfg.select(1900.0, TileLod::Mid), TileLod::Mid);
        assert_eq!(cfg.select(5200.0, TileLod::Mid), TileLod::Mid);
        assert_eq!(cfg.select(4700.0, TileLod::Far), TileLod::Far);
    }
}
