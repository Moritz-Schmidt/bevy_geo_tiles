# Bevy Geo Tiles
Open street map (or any other slippy map / TMS tile source) integration for Bevy.

This project is work in progress. Expect breaking changes, incomplete features and bugs.

## Features
- Load and display map tiles as Bevy textures
- Basic 2D camera controls (pan and zoom)
- Support for custom tile sources
- File system caching of downloaded tiles
- Basic support for markers, polylines, and polygons
- local-origin for improved precision (avoiding f32 float precision issues at very large coordinates)
- Coordinate conversion between WGS84, Web Mercator and bevy world coordinates
- Tile-loading in a separate thread to avoid blocking the main thread
- Each tile is an individual ECS entity allowing bevy to handle things like frustum culling automatically.

### Optional features
- `bevy_pancam` - Use [bevy_pancam](https://crates.io/crates/bevy_pancam) for camera controls instead of the minimalistic built-in controls.
- `shapes` - Enable drawing polylines and polygons using [lyon](https://crates.io/crates/lyon).
- `debug_draw` - Enable displaying Bevy, Web-Mercator and WGS84 coordinates at the mouse cursor for debugging purposes.

## Quick start
Add the crate to `Cargo.toml` and register the [`MapPlugin`] alongside Bevy’s default plugins:

```
use bevy::prelude::*;
use bevy_geo_tiles::MapPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(MapPlugin::default())
        .run();
}
```
For more examples, see the [`examples`](./examples) folder.

## Coordinate systems
- **Mercator space** uses `DVec2`/`DVec3` in meters relative to the Web Mercator map projection.
- **Local space** is Bevy’s world coordinate system (floating point `Vec2`/`Vec3`). 

The [`LocalOrigin`] resource tracks the current offset between the two and recenters automatically when the camera drifts too far from the origin.

See [`MapPlugin`] for configuration options, including tile server customization and cache settings.

## Limitations
- only supports 2D views (orthographic camera).
- no support for WASM targets (tile fetching and file system caching need to be adapted, PRs welcome).
- only supports 256x256 raster tiles.

## Compatibility
| bevy  | bevy_geo_tiles      |
|-------|---------------------|
| 0.17  | 0.1                 |


## License
This project is dual-licensed:
- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

## Contribution
Contributions are welcome! Please open issues or pull requests on the [GitHub repository](https://github.com/Moritz-Schmidt/bevy_geo_tiles/).