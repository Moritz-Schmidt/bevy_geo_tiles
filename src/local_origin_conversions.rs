use crate::coord_conversions::WebMercatorConversion;
use crate::local_origin::LocalOrigin;
use crate::local_origin::MercatorAabb2d;
use bevy::math::bounding::Aabb2d;
use bevy::math::{DVec2, DVec3};
use bevy::prelude::*;

/// Trait to convert between local Bevy world coordinates and web mercator or lon/lat coordinates, given a `LocalOrigin`.
///
/// Mercator coordinates should never be stored in f32 (Vec2/Vec3) as this can lead to significant precision loss. Use DVec2/DVec3 instead.
/// For Vec2 and Vec3 the results will be DVec2/DVec3 to avoid precision loss.
pub trait LocalOriginConversion {
    type MercatorOutput;
    type Output;
    fn mercator_to_local(&self, origin: &LocalOrigin) -> Self::Output;
    fn local_to_mercator(&self, origin: &LocalOrigin) -> Self::MercatorOutput;
    fn lonlat_to_local(&self, origin: &LocalOrigin) -> Self::Output;
    fn local_to_lonlat(&self, origin: &LocalOrigin) -> Self::Output;
}

impl LocalOriginConversion for Vec2 {
    type MercatorOutput = DVec2;
    type Output = Self;
    fn mercator_to_local(&self, origin: &LocalOrigin) -> Self {
        Vec2 {
            x: (self.x as f64 - origin.mercator_origin().x) as f32,
            y: (self.y as f64 - origin.mercator_origin().y) as f32,
        }
    }

    fn local_to_mercator(&self, origin: &LocalOrigin) -> Self::MercatorOutput {
        self.as_dvec2().local_to_mercator(origin)
    }

    fn lonlat_to_local(&self, origin: &LocalOrigin) -> Self {
        self.as_dvec2()
            .lonlat_to_mercator()
            .mercator_to_local(origin)
            .as_vec2()
    }

    fn local_to_lonlat(&self, origin: &LocalOrigin) -> Self {
        self.as_dvec2()
            .local_to_mercator(origin)
            .mercator_to_lonlat()
            .as_vec2()
    }
}

impl LocalOriginConversion for DVec2 {
    type MercatorOutput = Self;
    type Output = Self;
    fn mercator_to_local(&self, origin: &LocalOrigin) -> Self {
        DVec2 {
            x: self.x - origin.mercator_origin().x,
            y: self.y - origin.mercator_origin().y,
        }
    }

    fn local_to_mercator(&self, origin: &LocalOrigin) -> Self {
        DVec2 {
            x: self.x + origin.mercator_origin().x,
            y: self.y + origin.mercator_origin().y,
        }
    }

    fn lonlat_to_local(&self, origin: &LocalOrigin) -> Self {
        self.lonlat_to_mercator().mercator_to_local(origin)
    }

    fn local_to_lonlat(&self, origin: &LocalOrigin) -> Self {
        self.local_to_mercator(origin).mercator_to_lonlat()
    }
}

impl LocalOriginConversion for Vec3 {
    type MercatorOutput = DVec3;
    type Output = Self;
    fn mercator_to_local(&self, origin: &LocalOrigin) -> Self {
        self.truncate().mercator_to_local(origin).extend(self.z)
    }

    fn local_to_mercator(&self, origin: &LocalOrigin) -> Self::MercatorOutput {
        self.truncate()
            .local_to_mercator(origin)
            .extend(self.z as f64)
    }

    fn lonlat_to_local(&self, origin: &LocalOrigin) -> Self {
        self.as_dvec3()
            .truncate()
            .lonlat_to_mercator()
            .mercator_to_local(origin)
            .as_vec2()
            .extend(self.z)
    }

    fn local_to_lonlat(&self, origin: &LocalOrigin) -> Self {
        self.as_dvec3()
            .local_to_mercator(origin)
            .mercator_to_lonlat()
            .as_vec3()
    }
}

impl LocalOriginConversion for DVec3 {
    type MercatorOutput = Self;
    type Output = Self;
    fn mercator_to_local(&self, origin: &LocalOrigin) -> Self {
        self.truncate().mercator_to_local(origin).extend(self.z)
    }

    fn local_to_mercator(&self, origin: &LocalOrigin) -> Self {
        self.truncate().local_to_mercator(origin).extend(self.z)
    }

    fn lonlat_to_local(&self, origin: &LocalOrigin) -> Self {
        self.lonlat_to_mercator().mercator_to_local(origin)
    }

    fn local_to_lonlat(&self, origin: &LocalOrigin) -> Self {
        self.local_to_mercator(origin).mercator_to_lonlat()
    }
}

impl LocalOriginConversion for Aabb2d {
    type MercatorOutput = MercatorAabb2d;
    type Output = Self;
    fn mercator_to_local(&self, origin: &LocalOrigin) -> Self {
        Aabb2d {
            min: self.min.mercator_to_local(origin),
            max: self.max.mercator_to_local(origin),
        }
    }

    fn local_to_mercator(&self, origin: &LocalOrigin) -> Self::MercatorOutput {
        MercatorAabb2d {
            min: self.min.local_to_mercator(origin),
            max: self.max.local_to_mercator(origin),
        }
    }

    fn lonlat_to_local(&self, origin: &LocalOrigin) -> Self {
        Aabb2d {
            min: self.min.lonlat_to_local(origin),
            max: self.max.lonlat_to_local(origin),
        }
    }

    fn local_to_lonlat(&self, origin: &LocalOrigin) -> Self {
        Aabb2d {
            min: self.min.local_to_lonlat(origin),
            max: self.max.local_to_lonlat(origin),
        }
    }
}

impl LocalOriginConversion for MercatorAabb2d {
    type MercatorOutput = Self;
    type Output = Aabb2d;
    fn mercator_to_local(&self, origin: &LocalOrigin) -> Self::Output {
        Aabb2d {
            min: self.min.mercator_to_local(origin).as_vec2(),
            max: self.max.mercator_to_local(origin).as_vec2(),
        }
    }

    fn local_to_mercator(&self, origin: &LocalOrigin) -> Self {
        MercatorAabb2d {
            min: self.min.local_to_mercator(origin),
            max: self.max.local_to_mercator(origin),
        }
    }

    fn lonlat_to_local(&self, origin: &LocalOrigin) -> Self::Output {
        Aabb2d {
            min: self.min.lonlat_to_local(origin).as_vec2(),
            max: self.max.lonlat_to_local(origin).as_vec2(),
        }
    }

    fn local_to_lonlat(&self, origin: &LocalOrigin) -> Self::Output {
        Aabb2d {
            min: self.min.local_to_lonlat(origin).as_vec2(),
            max: self.max.local_to_lonlat(origin).as_vec2(),
        }
    }
}
