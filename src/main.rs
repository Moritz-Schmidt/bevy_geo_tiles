use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy::mesh::Polyline2dMeshBuilder;
use bevy::sprite;
use bevy::{log::LogPlugin, prelude::*};
use bevy_geo_tiles::{KeepDisplaySize, MapPlugin, MercatorCoords, Tile};

fn main() {
    App::new()
        // Configure settings with defaults
        .add_plugins(
            DefaultPlugins
                .set(LogPlugin {
                    filter:
                        "info,wgpu_core=warn,wgpu_hal=warn,naga=warn,ureq=info,bevy_asset=debug,bevy_geo_tiles=debug"
                            .into(),
                    level: bevy::log::Level::DEBUG,
                    ..Default::default()
                })
                .set(AssetPlugin {
                    meta_check: bevy::asset::AssetMetaCheck::Never,
                    ..Default::default()
                }),
        )
        .add_plugins(FpsOverlayPlugin::default())
        .add_plugins(MapPlugin {
            initial_zoom: 10,
            initial_center: (13.4064, 52.51977).into(),
            tile_source: "https://tile.openstreetmap.org/{z}/{x}/{y}.png".to_string(),
            reverse_y: false,
            zoom_offset: 0,
            ..Default::default()
        })
        .add_systems(Startup, create_marker)
        
        .run();
}

fn create_marker(mut commands: Commands) {
    commands.spawn((
        sprite::Sprite {
            color: Color::linear_rgb(1.0, 0.0, 0.0),
            custom_size: Some(Vec2::splat(1.0)),
            ..Default::default()
        },
        MercatorCoords::from_latlon(52.51977, 13.4064).with_z(5.0),
        KeepDisplaySize,
    ));
}
