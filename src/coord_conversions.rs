use bevy::{
    ecs::system::SystemParam,
    math::{DVec2, DVec3, bounding::Aabb2d},
    prelude::*,
};
use miniproj::Projection;
use miniproj_ops::popvis_pseudo_mercator::PopVisPseudoMercatorProjection;
use tilemath::{BBox, Tile as TileMathTile};

use crate::local_origin::{LocalOrigin, MercatorAabb2d, TileBounds};

const WEB_MERCATOR_EXTENT: f64 = 20037508.342789244;

// Inlined miniproj::get_projection(3857).unwrap()
const WEB_MERCATOR: PopVisPseudoMercatorProjection = PopVisPseudoMercatorProjection {
    ellipsoid_a: 6378137f64,
    lon_orig: 0f64,
    false_e: 0f64,
    false_n: 0f64,
};
#[derive(SystemParam)]
pub struct ViewportConv<'w, 's, MainCamMarker: Component> {
    camera: Single<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamMarker>>,
    origin: Res<'w, LocalOrigin>,
}

impl<'w, 's, MainCamMarker: Component> ViewportConv<'w, 's, MainCamMarker> {
    pub fn viewport_to_mercator_2d(&self, viewport_pos: Vec2) -> Result<DVec2> {
        let local = self
            .camera
            .0
            .viewport_to_world_2d(self.camera.1, viewport_pos)?;
        Ok(self.origin.local_to_mercator_vec2(local))
    }

    pub fn mercator_to_viewport(&self, mercator_pos: DVec3) -> Result<Vec2> {
        let local = self.origin.mercator_to_local_vec3(mercator_pos);
        Ok(self.camera.0.world_to_viewport(self.camera.1, local)?)
    }

    pub fn viewport_to_latlon(&self, viewport_pos: Vec2) -> Result<DVec2> {
        Ok(self
            .viewport_to_mercator_2d(viewport_pos)?
            .mercator_to_lonlat())
    }

    pub fn latlon_to_viewport(&self, latlon: impl Into<DVec2>) -> Result<Vec2> {
        let latlon = latlon.into();
        self.mercator_to_viewport(latlon.extend(0.0).lonlat_to_mercator())
    }

    pub fn visible_mercator_aabb(&self) -> Result<MercatorAabb2d> {
        if let Some(viewport) = self.camera.0.logical_viewport_rect() {
            let local_bounds = Aabb2d::from_point_cloud(
                Isometry2d::IDENTITY,
                &[
                    self.camera
                        .0
                        .viewport_to_world_2d(self.camera.1, viewport.max)?,
                    self.camera
                        .0
                        .viewport_to_world_2d(self.camera.1, viewport.min)?,
                ],
            );
            Ok(self.origin.local_aabb_to_mercator(&local_bounds))
        } else {
            todo!()
        }
    }

    pub fn viewport_center_mercator(&self) -> Result<DVec2> {
        Ok(self.visible_mercator_aabb()?.center())
    }
}

pub trait WebMercatorConversion {
    fn mercator_to_lonlat(&self) -> Self;
    fn lonlat_to_mercator(&self) -> Self;
}

impl WebMercatorConversion for DVec2 {
    fn mercator_to_lonlat(&self) -> Self {
        DVec2::from(WEB_MERCATOR.projected_to_deg(self.x, self.y))
    }

    fn lonlat_to_mercator(&self) -> Self {
        DVec2::from(WEB_MERCATOR.deg_to_projected(self.x, self.y))
    }
}

impl WebMercatorConversion for Vec2 {
    fn mercator_to_lonlat(&self) -> Self {
        self.as_dvec2().mercator_to_lonlat().as_vec2()
    }

    fn lonlat_to_mercator(&self) -> Self {
        self.as_dvec2().lonlat_to_mercator().as_vec2()
    }
}

impl WebMercatorConversion for Vec3 {
    fn mercator_to_lonlat(&self) -> Self {
        self.truncate().mercator_to_lonlat().extend(self.z)
    }

    fn lonlat_to_mercator(&self) -> Self {
        self.truncate().lonlat_to_mercator().extend(self.z)
    }
}
impl WebMercatorConversion for DVec3 {
    fn mercator_to_lonlat(&self) -> Self {
        self.truncate().mercator_to_lonlat().extend(self.z)
    }

    fn lonlat_to_mercator(&self) -> Self {
        self.truncate().lonlat_to_mercator().extend(self.z)
    }
}

impl WebMercatorConversion for Aabb2d {
    fn mercator_to_lonlat(&self) -> Self {
        Aabb2d {
            max: self.max.mercator_to_lonlat(),
            min: self.min.mercator_to_lonlat(),
        }
    }
    fn lonlat_to_mercator(&self) -> Self {
        Aabb2d {
            max: self.max.lonlat_to_mercator(),
            min: self.min.lonlat_to_mercator(),
        }
    }
}

impl WebMercatorConversion for MercatorAabb2d {
    fn mercator_to_lonlat(&self) -> Self {
        MercatorAabb2d {
            max: self.max.mercator_to_lonlat(),
            min: self.min.mercator_to_lonlat(),
        }
    }

    fn lonlat_to_mercator(&self) -> Self {
        MercatorAabb2d {
            max: self.max.lonlat_to_mercator(),
            min: self.min.lonlat_to_mercator(),
        }
    }
}

pub trait ToBBox {
    fn lonlat_to_bbox(&self) -> BBox;
    fn mercator_to_bbox(&self) -> BBox;
}
impl ToBBox for Aabb2d {
    fn lonlat_to_bbox(&self) -> BBox {
        self.lonlat_to_mercator().mercator_to_bbox()
    }
    fn mercator_to_bbox(&self) -> BBox {
        BBox {
            min_x: self.min.x as f64,
            min_y: self.min.y as f64,
            max_x: self.max.x as f64,
            max_y: self.max.y as f64,
        }
    }
}

impl ToBBox for MercatorAabb2d {
    fn lonlat_to_bbox(&self) -> BBox {
        self.lonlat_to_mercator().mercator_to_bbox()
    }

    fn mercator_to_bbox(&self) -> BBox {
        BBox {
            min_x: self.min.x,
            min_y: self.min.y,
            max_x: self.max.x,
            max_y: self.max.y,
        }
    }
}

pub trait ToTileCoords {
    type Output;
    fn mercator_to_tile_coords(&self, zoom: u8) -> Self::Output;
    fn lonlat_to_tile_coords(&self, zoom: u8) -> Self::Output;
}
impl ToTileCoords for DVec2 {
    type Output = UVec2;

    fn mercator_to_tile_coords(&self, zoom: u8) -> Self::Output {
        let scale = (1 << zoom) as f64;
        let limit = 2u32.pow(zoom as u32) - 1;

        let norm = (self + WEB_MERCATOR_EXTENT) / (2. * WEB_MERCATOR_EXTENT);
        let scaled = (norm * scale).floor().as_uvec2();
        UVec2::new(scaled.x.clamp(0, limit), scaled.y % (limit + 1))
    }

    fn lonlat_to_tile_coords(&self, zoom: u8) -> Self::Output {
        self.lonlat_to_mercator().mercator_to_tile_coords(zoom)
    }
}

impl ToTileCoords for Vec2 {
    type Output = UVec2;
    fn mercator_to_tile_coords(&self, zoom: u8) -> Self::Output {
        self.as_dvec2().mercator_to_tile_coords(zoom)
    }

    fn lonlat_to_tile_coords(&self, zoom: u8) -> Self::Output {
        self.as_dvec2().lonlat_to_tile_coords(zoom)
    }
}

impl ToTileCoords for MercatorAabb2d {
    type Output = TileBounds;

    fn mercator_to_tile_coords(&self, zoom: u8) -> Self::Output {
        let max = self.max;
        let min = self.min;
        let scale = (1 << zoom) as f64;
        let limit = 2u32.pow(zoom as u32) - 1;
        let norm_max = (max + WEB_MERCATOR_EXTENT) / (2.0 * WEB_MERCATOR_EXTENT);
        let tile_max = norm_max * scale;
        let max_coords = (tile_max.ceil() - 1.0)
            .max(DVec2::ZERO)
            .as_uvec2()
            .min(UVec2::splat(limit));

        TileBounds {
            min: min.mercator_to_tile_coords(zoom),
            max: max_coords,
        }
    }

    fn lonlat_to_tile_coords(&self, zoom: u8) -> Self::Output {
        self.lonlat_to_mercator().mercator_to_tile_coords(zoom)
    }
}

pub fn tile_to_mercator_aabb(tile: TileMathTile) -> MercatorAabb2d {
    let tile_size = (2.0 * WEB_MERCATOR_EXTENT) / (1u32 << tile.zoom) as f64;

    let min = DVec2::new(
        tile.x as f64 * tile_size - WEB_MERCATOR_EXTENT,
        -WEB_MERCATOR_EXTENT + tile.y as f64 * tile_size,
    );
    let max = DVec2::new(
        (tile.x + 1) as f64 * tile_size - WEB_MERCATOR_EXTENT,
        -WEB_MERCATOR_EXTENT + (tile.y as f64 + 1.) * tile_size,
    );

    MercatorAabb2d { min, max }
}
