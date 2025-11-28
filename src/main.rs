use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy::sprite;
use bevy::{log::LogPlugin, prelude::*};
use bevy_geo_tiles::shapes::polyline::{
    GeoPolyline, GeoPolylineConfig, KeepDisplayWidth, PolylineStyle,
};
use bevy_geo_tiles::{KeepDisplaySize, MapPlugin, MercatorCoords, WebMercatorConversion};

use bevy_geo_tiles::shapes::polygon::GeoPolygon;

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
        .add_systems(Startup, (create_marker, test))
        //.add_systems(Update, (update_polyline,))
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

fn test(mut commands: Commands) {
    commands.spawn(GeoPolygon {
        points: vec![
            (13.40944, 52.52000),
            (13.40941, 52.52016),
            (13.40864, 52.52035),
            (13.40905, 52.52054),
            (13.41007, 52.52029),
        ]
        .lonlat_to_mercator(),
        fill_color: Some(Color::linear_rgba(1.0, 0.0, 0.0, 0.5)),
    });

    commands.spawn((
        GeoPolyline {
            points: vec![
                (52.51776793273815, 13.399284510002412),
                (52.51989357688242, 13.404683167174046),
                (52.52392718698061, 13.412106320785039),
                (52.52199177788207, 13.416636222764344),
            ]
            .latlon_to_mercator(),
        },
        GeoPolylineConfig {
            style: PolylineStyle::VariableWidthVariableColor {
                colors: vec![
                    Color::linear_rgba(1.0, 0.0, 0.0, 1.0),
                    Color::linear_rgba(1.0, 1.0, 0.0, 1.0),
                    Color::linear_rgba(0.0, 1.0, 1.0, 1.0),
                    Color::linear_rgba(1.0, 0.0, 1.0, 1.0),
                ],
                widths: vec![50.0, 100.0, 200.0, 150.0],
            },
            ..Default::default()
        },
        KeepDisplayWidth,
    ));
}
