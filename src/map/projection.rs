//! Real-world latitude/longitude → world meters.

use bevy::prelude::*;

use super::data::LatLon;

/// Meters per degree of latitude, which is close enough to constant at map scale.
const METERS_PER_DEGREE: f64 = 111_320.0;

/// Equirectangular projection about a map origin. It is accurate for regional
/// maps of tens of kilometers. An ocean-scale map would need a better
/// projection.
#[derive(Debug, Clone, Copy)]
pub struct GeoProjection {
    origin: LatLon,
    meters_per_degree_lat: f64,
    meters_per_degree_lon: f64,
}

impl GeoProjection {
    pub fn new(origin: LatLon, distance_scale: f64) -> Self {
        let meters_per_degree_lat = METERS_PER_DEGREE * distance_scale;
        Self {
            origin,
            meters_per_degree_lat,
            meters_per_degree_lon: meters_per_degree_lat * origin.0.to_radians().cos(),
        }
    }

    /// World position at sea level. North → −Z, east → +X.
    pub fn to_world(&self, (lat, lon): LatLon) -> Vec3 {
        let east = (lon - self.origin.1) * self.meters_per_degree_lon;
        let north = (lat - self.origin.0) * self.meters_per_degree_lat;
        Vec3::new(east as f32, 0.0, -north as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PORT_ROYAL: LatLon = (17.9372, -76.8435);

    #[test]
    fn origin_maps_to_world_origin() {
        assert_eq!(
            GeoProjection::new(PORT_ROYAL, 1.0).to_world(PORT_ROYAL),
            Vec3::ZERO
        );
    }

    #[test]
    fn north_is_negative_z_and_east_is_positive_x() {
        let projection = GeoProjection::new(PORT_ROYAL, 1.0);
        let north = projection.to_world((PORT_ROYAL.0 + 0.01, PORT_ROYAL.1));
        let east = projection.to_world((PORT_ROYAL.0, PORT_ROYAL.1 + 0.01));
        assert!(north.z < 0.0 && north.x.abs() < 1e-3);
        assert!(east.x > 0.0 && east.z.abs() < 1e-3);
    }

    #[test]
    fn one_minute_of_latitude_is_one_nautical_mile() {
        let projection = GeoProjection::new(PORT_ROYAL, 1.0);
        let one_minute = projection.to_world((PORT_ROYAL.0 + 1.0 / 60.0, PORT_ROYAL.1));
        assert!(
            (one_minute.length() - 1852.0).abs() < 5.0,
            "{}",
            one_minute.length()
        );
    }

    #[test]
    fn longitude_degrees_shrink_away_from_the_equator() {
        let projection = GeoProjection::new(PORT_ROYAL, 1.0);
        let lat_step = projection
            .to_world((PORT_ROYAL.0 + 0.01, PORT_ROYAL.1))
            .length();
        let lon_step = projection
            .to_world((PORT_ROYAL.0, PORT_ROYAL.1 + 0.01))
            .length();
        let expected = 17.9372_f32.to_radians().cos();
        assert!((lon_step / lat_step - expected).abs() < 1e-4);
    }

    #[test]
    fn distance_scale_compresses_uniformly() {
        let point = (PORT_ROYAL.0 + 0.02, PORT_ROYAL.1 - 0.03);
        let full = GeoProjection::new(PORT_ROYAL, 1.0).to_world(point);
        let half = GeoProjection::new(PORT_ROYAL, 0.5).to_world(point);
        assert!(half.abs_diff_eq(full * 0.5, 1e-3));
    }
}
