use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy::math::DVec2;
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
        .add_plugins(DefaultPlugins)
        .add_plugins(FpsOverlayPlugin::default())
        .add_plugins(MapPlugin {
            initial_zoom: 10,
            initial_center: (13.4064, 52.51977).into(),
            tile_source: "https://tile.openstreetmap.org/{z}/{x}/{y}.png".to_string(),
            reverse_y: false,
            zoom_offset: 0,
            ..Default::default()
        })
        .add_systems(Startup, (create_marker, spawn_many_shapes))
        //.add_systems(Update, (update_polyline,))
        .run();
}

fn spawn_many_shapes(mut commands: Commands) {
    for i in 0..10_000 {
        commands.spawn((
            GeoPolyline {
                points: vec![
                    (52.51776793273815, 13.399284510002412),
                    (52.51989357688242, 13.404683167174046),
                    (52.52392718698061, 13.412106320785039),
                    (52.52199177788207, 13.416636222764344),
                ]
                .latlon_to_mercator()
                .iter()
                .map(|x| DVec2 {
                    x: x.x + (i % 100) as f64 * 200.0,
                    y: x.y + (i / 100) as f64 * 1000.0,
                })
                .collect(),
            },
            GeoPolylineConfig {
                style: PolylineStyle::VariableWidthVariableColor {
                    colors: vec![
                        Color::linear_rgba(1.0, 0.0, 0.0, 1.0),
                        Color::linear_rgba(1.0, 1.0, 0.0, 1.0),
                        Color::linear_rgba(0.0, 1.0, 1.0, 1.0),
                        Color::linear_rgba(1.0, 0.0, 1.0, 1.0),
                    ],
                    widths: vec![10.0, 20.0, 20.0, 15.0],
                },
                ..Default::default()
            },
        ));
    }
}
