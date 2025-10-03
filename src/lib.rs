use bevy::{prelude::*, ui::debug};
use tilemath::{Tile,TileIterator,BBox, bbox_covered_tiles};



pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let proj = proj::Proj::new_known_crs("EPSG:4326", "EPSG:3857", None).unwrap();
    let (max_x, max_y) = proj.convert((14.261350595825427, 54.963283243087496)).unwrap();
    let (min_x, min_y) = proj.convert((5.5437553459687186, 46.2960593038139)).unwrap();
    let bbox = BBox {
        min_x,
        min_y,
        max_x,
        max_y,
    };
    let zoom = 9;
    let tiles = bbox_covered_tiles(&bbox, zoom).collect::<Vec<_>>();
    info!("Tiles to load: {:?}", tiles);
    for tile in tiles {
        info!("Loading tile: {:?}", tile);
        let tms_tile = tile.to_reversed_y();
        let url = format!("https://mapproxy.dmho.de/tms/1.0.0/thunderforest_transport/EPSG3857/{}/{}/{}.png", tms_tile.zoom - 1, tms_tile.x, tms_tile.y);
        let image: Handle<Image> = asset_server.load(url.clone());
        commands.spawn((
            Sprite::from_image(image),
            Transform::from_translation(
                Vec3::new(
                    (tms_tile.x as f32) * 256.0,
                    (tms_tile.y as f32) * 256.0,
                    0.0,
                ),
            ),
        ));
        commands.spawn((
            Sprite {
                custom_size: Some(Vec2::new(4.0, 4.0)),
                color: Color::linear_rgb(1.0, 0.0, 0.0),
                ..Default::default()
            },
            Transform::from_translation(
                Vec3::new(
                    (tms_tile.x as f32) * 256.0,
                    (tms_tile.y as f32) * 256.0,
                    1.0,
                ),
            ),
        ));
    }
}
