use bevy::{
    ecs::{component::ComponentId, lifecycle::HookContext, world::DeferredWorld},
    math::{DVec2, DVec3, bounding::Aabb2d},
    prelude::*,
};

use crate::WebMercatorConversion;

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct LocalSpace;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct MercatorCoords(pub DVec3);

impl MercatorCoords {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self(DVec3::new(x, y, z))
    }

    pub fn from_vec(vec: DVec3) -> Self {
        Self(vec)
    }

    pub fn from_latlon(lat: f64, lon: f64) -> Self {
        let mercator = DVec2::new(lon, lat).lonlat_to_mercator();
        Self::new(mercator.x, mercator.y, 0.0)
    }

    pub fn with_z(self, z: f64) -> Self {
        Self(DVec3::new(self.0.x, self.0.y, z))
    }

    pub fn as_dvec3(self) -> DVec3 {
        self.0
    }

    pub fn xy(self) -> DVec2 {
        self.0.truncate()
    }
}

impl From<DVec3> for MercatorCoords {
    fn from(value: DVec3) -> Self {
        Self::from_vec(value)
    }
}

impl From<MercatorCoords> for DVec3 {
    fn from(value: MercatorCoords) -> Self {
        value.0
    }
}

#[derive(Resource, Debug, Clone)]
pub struct LocalOrigin {
    mercator_origin: DVec3,
    recenter_distance: f64,
}

impl LocalOrigin {
    pub const DEFAULT_RECENTER_DISTANCE: f64 = 2_500.0;

    pub fn new(mercator_origin: DVec3) -> Self {
        Self::with_distance(mercator_origin, Self::DEFAULT_RECENTER_DISTANCE)
    }

    pub fn with_distance(mercator_origin: DVec3, recenter_distance: f64) -> Self {
        Self {
            mercator_origin,
            recenter_distance,
        }
    }

    pub fn mercator_origin(&self) -> DVec3 {
        self.mercator_origin
    }

    pub fn recenter_distance(&self) -> f64 {
        self.recenter_distance
    }

    pub fn set_mercator_origin(&mut self, origin: DVec3) {
        self.mercator_origin = origin;
    }

    pub fn shift_mercator_origin(&mut self, delta: DVec3) {
        self.mercator_origin += delta;
    }

    pub fn mercator_to_local_vec3(&self, mercator: DVec3) -> Vec3 {
        Vec3::new(
            (mercator.x - self.mercator_origin.x) as f32,
            (mercator.y - self.mercator_origin.y) as f32,
            mercator.z as f32,
        )
    }

    pub fn mercator_to_local_vec2(&self, mercator: DVec2) -> Vec2 {
        (mercator - self.mercator_origin.truncate()).as_vec2()
    }

    pub fn local_to_mercator_vec2(&self, local: Vec2) -> DVec2 {
        local.as_dvec2() + self.mercator_origin.truncate()
    }

    pub fn local_to_mercator_vec3(&self, local: Vec3) -> DVec3 {
        DVec3::new(
            local.x as f64 + self.mercator_origin.x,
            local.y as f64 + self.mercator_origin.y,
            local.z as f64,
        )
    }

    pub fn local_aabb_to_mercator(&self, aabb: &Aabb2d) -> MercatorAabb2d {
        MercatorAabb2d {
            min: self.local_to_mercator_vec2(aabb.min),
            max: self.local_to_mercator_vec2(aabb.max),
        }
    }

    pub fn mercator_aabb_to_local(&self, mercator: MercatorAabb2d) -> Aabb2d {
        Aabb2d {
            min: self.mercator_to_local_vec2(mercator.min),
            max: self.mercator_to_local_vec2(mercator.max),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MercatorAabb2d {
    pub min: DVec2,
    pub max: DVec2,
}

impl MercatorAabb2d {
    pub fn new(min: DVec2, max: DVec2) -> Self {
        Self { min, max }
    }

    pub fn center(&self) -> DVec2 {
        (self.min + self.max) * 0.5
    }

    pub fn size(&self) -> DVec2 {
        self.max - self.min
    }

    pub fn half_size(&self) -> DVec2 {
        self.size() * 0.5
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TileBounds {
    pub min: UVec2,
    pub max: UVec2,
}

impl TileBounds {
    pub fn x_range(&self) -> std::ops::RangeInclusive<u32> {
        self.min.x..=self.max.x
    }

    pub fn y_range(&self) -> std::ops::RangeInclusive<u32> {
        self.min.y..=self.max.y
    }
}
