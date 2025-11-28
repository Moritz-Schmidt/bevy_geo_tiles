use bevy::prelude::*;

use bevy_geo_tiles::{
    MapPlugin, WebMercatorConversion,
    shapes::polyline::{GeoPolyline, GeoPolylineConfig, PolylineStyle},
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(MapPlugin {
            initial_zoom: 17,
            initial_center: (13.374277, 52.522675).into(),
            ..Default::default()
        })
        .add_systems(Startup, add_polylines)
        .run();
}

fn add_polylines(mut commands: Commands) {
    commands.spawn((
        GeoPolyline {
            points: vec![
                (13.364011198282244, 52.523108591666364),
                (13.365367054939272, 52.52379990863249),
                (13.366729617118835, 52.52434703563496),
                (13.368438184261324, 52.524774548178506),
                (13.369618356227877, 52.52493037709765),
                (13.370795845985414, 52.52499401387945),
                (13.372277766466143, 52.52501277855392),
                (13.373640328645708, 52.52481534114268),
                (13.374795019626617, 52.524402548784344),
            ]
            .lonlat_to_mercator(),
        },
        GeoPolylineConfig {
            style: PolylineStyle::ConstantWidthConstantColor {
                width: 10.0,
                color: Color::linear_rgb(1.0, 0.0, 0.0),
            },
            ..Default::default()
        },
    ));

    commands.spawn((
        GeoPolyline {
            points: vec![
                (13.366831541061403, 52.520425662779544),
                (13.368011713027956, 52.5210788561568),
                (13.368827104568483, 52.52189262802074),
                (13.36964249610901, 52.522610625760066),
                (13.371080160140993, 52.52295003878816),
                (13.372753858566286, 52.5230153102237),
                (13.374277353286743, 52.52267589769991),
                (13.37565064430237, 52.52208844674982),
                (13.376122713088991, 52.521579316238636),
                (13.376423120498659, 52.52063936748485),
                (13.376787900924684, 52.520064944457104),
                (13.377753496170046, 52.51952967987579),
                (13.379491567611696, 52.5191902404207),
            ]
            .lonlat_to_mercator(),
        },
        GeoPolylineConfig {
            style: PolylineStyle::VariableWidthVariableColor {
                widths: vec![
                    50.0, 45.0, 40.0, 35.0, 30.0, 25.0, 20.0, 15.0, 10.0, 15.0, 20.0, 25.0, 30.0,
                ],
                colors: vec![
                    Color::linear_rgb(1.0, 0.0, 0.0),
                    Color::linear_rgb(1.0, 0.5, 0.0),
                    Color::linear_rgb(1.0, 1.0, 0.0),
                    Color::linear_rgb(0.5, 1.0, 0.0),
                    Color::linear_rgb(0.0, 1.0, 0.0),
                    Color::linear_rgb(0.0, 1.0, 0.5),
                    Color::linear_rgb(0.0, 1.0, 1.0),
                    Color::linear_rgb(0.0, 0.5, 1.0),
                    Color::linear_rgb(0.0, 0.0, 1.0),
                    Color::linear_rgb(0.5, 0.0, 1.0),
                    Color::linear_rgb(1.0, 0.0, 1.0),
                    Color::linear_rgb(1.0, 0.0, 0.5),
                    Color::linear_rgb(1.0, 0.0, 0.0),
                ],
            },
            ..Default::default()
        },
    ));
}
