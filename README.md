# Bevy Geo Tiles (WIP)

Open street map (or any other slippy map / TMS tile source) integration for Bevy.

This project is a work in progress and currently in an early alpha state. Expect breaking changes, incomplete features and bugs.

## Features
- Load and display map tiles as Bevy textures
- Basic 2D camera controls (pan and zoom)
- Support for custom tile sources
- File system caching of downloaded tiles
- Basic support for markers, polylines, and polygons
- local-origin for improved precision (avoiding f32 float precision issues at very large coordinates)
- Coordinate conversion between WGS84, Web Mercator and bevy world coordinates
- Basic frustum culling of tiles outside the camera view
- Tile-loading in a separate thread to avoid blocking the main thread

## License
This project is dual-licensed:
- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)