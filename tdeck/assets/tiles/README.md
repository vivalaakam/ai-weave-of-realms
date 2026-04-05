# T-Deck Tile Exports

These BMP files are exported from the shared Godot tileset at `godot/assets/tileset.png`.

- Source atlas layout: 14 tiles in a single `896x64` RGBA strip
- Export size: one `32x32` bitmap per tile
- Export format: Windows BMP v3 (`24-bit`)
- Transparency handling: all non-opaque pixels are flattened onto `#FF00FF` magenta

This keeps the visual tile content aligned with the Godot assets while producing files that are easier to consume on the T-Deck side.

## Tile Mapping

- `Meadow.bmp`
- `Forest.bmp`
- `Mountain.bmp`
- `Water.bmp`
- `City.bmp`
- `CityEntrance.bmp`
- `Road.bmp`
- `River.bmp`
- `Bridge.bmp`
- `Village.bmp`
- `Merchant.bmp`
- `Ruins.bmp`
- `Gold.bmp`
- `Resource.bmp`
