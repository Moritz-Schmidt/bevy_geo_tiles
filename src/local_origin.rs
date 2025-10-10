use bevy::{
    math::{DVec2, DVec3, bounding::Aabb2d},
    prelude::*,
};

use crate::WebMercatorConversion;

/// Marker component for entities in local space (relative to the `LocalOrigin`).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct LocalSpace;

/// Web mercator coordinates (EPSG:3857) stored in a DVec3 (x, y, z).
///
/// The z coordinate is not used for coordinate conversions but can be used to create layers (i.e. displaying things above other things)
/// Entities with this component automatically get a `Transform` component with the local coordinates relative to the `LocalOrigin`.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct MercatorCoords(pub DVec3);

/// Geographic position stored in Web Mercator meters.
///
/// The [`MapPlugin`](crate::MapPlugin) keeps the entity's [`Transform`] synchronized with the
/// moving local origin, so you can update the mercator coordinates independently of the
/// current camera position.
impl MercatorCoords {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self(DVec3::new(x, y, z))
    }

    pub fn from_vec(vec: DVec3) -> Self {
        Self(vec)
    }

    /// Creates MercatorCoords from latitude and longitude in degrees (WGS84 / EPSG:4326).
    pub fn from_latlon(lat: f64, lon: f64) -> Self {
        let mercator = DVec2::new(lon, lat).lonlat_to_mercator();
        Self::new(mercator.x, mercator.y, 0.0)
    }

    /// Sets the vertical component (Bevy `Z`) while keeping the projected X/Y values.
    pub fn with_z(self, z: f64) -> Self {
        Self(DVec3::new(self.0.x, self.0.y, z))
    }

    /// Returns the position as a `DVec3` in Web Mercator space.
    pub fn as_dvec3(self) -> DVec3 {
        self.0
    }

    /// Returns the horizontal position as a `DVec2` in Web Mercator space.
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

/// Local origin in web mercator coordinates and the distance at which to recenter.
#[derive(Resource, Debug, Clone)]
pub struct LocalOrigin {
    mercator_origin: DVec3,
    recenter_distance: f64,
}

impl LocalOrigin {
    pub(crate) const DEFAULT_RECENTER_DISTANCE: f64 = 2_500.0;

    pub(crate) fn new(mercator_origin: DVec3) -> Self {
        Self::with_distance(mercator_origin, Self::DEFAULT_RECENTER_DISTANCE)
    }

    pub(crate) fn with_distance(mercator_origin: DVec3, recenter_distance: f64) -> Self {
        Self {
            mercator_origin,
            recenter_distance,
        }
    }

    pub(crate) fn mercator_origin(&self) -> DVec3 {
        self.mercator_origin
    }

    pub(crate) fn recenter_distance(&self) -> f64 {
        self.recenter_distance
    }

    pub(crate) fn set_mercator_origin(&mut self, origin: DVec3) {
        self.mercator_origin = origin;
    }

    pub(crate) fn shift_mercator_origin(&mut self, delta: DVec3) {
        self.mercator_origin += delta;
    }
}

/// Axis-aligned bounding box in web mercator coordinates, uses DVec2 for min and max.
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

/// Bounding box of tile coordinates.
#[derive(Debug, Clone, Copy)]
pub struct TileBounds {
    pub min: UVec2,
    pub max: UVec2,
}

impl TileBounds {
    /// range of x tile coordinates (inclusive)
    pub fn x_range(&self) -> std::ops::RangeInclusive<u32> {
        self.min.x..=self.max.x
    }

    /// range of y tile coordinates (inclusive)
    pub fn y_range(&self) -> std::ops::RangeInclusive<u32> {
        self.min.y..=self.max.y
    }
}
