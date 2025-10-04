use bevy::{
    camera::ViewportConversionError,
    ecs::system::SystemParam,
    math::{DVec2, DVec3, bounding::Aabb2d},
    prelude::*,
};
use miniproj::Projection;
use miniproj_ops::popvis_pseudo_mercator::PopVisPseudoMercatorProjection;
use tilemath::BBox;

// Inlined miniproj::get_projection(3857).unwrap()
const WEB_MERCATOR: PopVisPseudoMercatorProjection = PopVisPseudoMercatorProjection {
    ellipsoid_a: 6378137f64,
    lon_orig: 0f64,
    false_e: 0f64,
    false_n: 0f64,
};
#[derive(SystemParam)]
pub struct Convert<'w, 's, MainCamMarker: Component> {
    camera: Single<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamMarker>>,
}

impl<'w, 's, MainCamMarker: Component> Convert<'w, 's, MainCamMarker> {
    pub fn viewport_to_world_2d(&self, viewport_pos: Vec2) -> Result<Vec2> {
        Ok(self
            .camera
            .0
            .viewport_to_world_2d(self.camera.1, viewport_pos)?)
    }

    pub fn world_to_viewport(&self, world_pos: Vec3) -> Result<Vec2> {
        Ok(self.camera.0.world_to_viewport(self.camera.1, world_pos)?)
    }

    pub fn viewport_to_latlon(&self, viewport_pos: Vec2) -> Result<Vec2> {
        Ok(self.viewport_to_world_2d(viewport_pos)?.world_to_lonlat())
    }

    pub fn latlon_to_viewport(&self, latlon: Vec2) -> Result<Vec2> {
        self.world_to_viewport(latlon.extend(0.0).lonlat_to_world())
    }
}

pub trait WebMercatorConversion {
    fn world_to_lonlat(&self) -> Self;

    fn lonlat_to_world(&self) -> Self;
}

impl WebMercatorConversion for DVec2 {
    fn world_to_lonlat(&self) -> Self {
        DVec2::from(WEB_MERCATOR.projected_to_deg(self.x, self.y))
    }

    fn lonlat_to_world(&self) -> Self {
        DVec2::from(WEB_MERCATOR.deg_to_projected(self.x, self.y))
    }
}

impl WebMercatorConversion for Vec2 {
    fn world_to_lonlat(&self) -> Self {
        self.as_dvec2().world_to_lonlat().as_vec2()
    }

    fn lonlat_to_world(&self) -> Self {
        self.as_dvec2().lonlat_to_world().as_vec2()
    }
}

impl WebMercatorConversion for Vec3 {
    fn world_to_lonlat(&self) -> Self {
        self.truncate().world_to_lonlat().extend(self.z)
    }

    fn lonlat_to_world(&self) -> Self {
        self.truncate().lonlat_to_world().extend(self.z)
    }
}
impl WebMercatorConversion for DVec3 {
    fn world_to_lonlat(&self) -> Self {
        self.truncate().world_to_lonlat().extend(self.z)
    }

    fn lonlat_to_world(&self) -> Self {
        self.truncate().lonlat_to_world().extend(self.z)
    }
}

impl WebMercatorConversion for Aabb2d {
    fn world_to_lonlat(&self) -> Self {
        Aabb2d {
            max: self.max.world_to_lonlat(),
            min: self.min.world_to_lonlat(),
        }
    }
    fn lonlat_to_world(&self) -> Self {
        Aabb2d {
            max: self.max.lonlat_to_world(),
            min: self.min.lonlat_to_world(),
        }
    }
}

pub trait ToBBox {
    fn lonlat_to_bbox(&self) -> BBox;
    fn world_to_bbox(&self) -> BBox;
}
impl ToBBox for Aabb2d {
    fn lonlat_to_bbox(&self) -> BBox {
        self.lonlat_to_world().world_to_bbox()
    }
    fn world_to_bbox(&self) -> BBox {
        BBox {
            min_x: self.min.x as f64,
            min_y: self.min.y as f64,
            max_x: self.max.x as f64,
            max_y: self.max.y as f64,
        }
    }
}
