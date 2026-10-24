# RogueRing

A boss rush roguelite for ELDEN RING, in the style of Slay the Spire, solo or in co-op. Pick a
character, cross a map of fights, events, shops and graces, collect blessings that change how you
fight, and try to beat the lord of each act in a single run. Combat is the game's own; everything
around it is the mod.

Download, install and co-op instructions are on [Nexus Mods](https://www.nexusmods.com/eldenring/mods/11459).
The mod uses its own save (`RogueRing.sl2`, created from a template the first time) and never touches
your normal save.

## Requirements

- ELDEN RING on Steam, with Shadow of the Erdtree.
- [me3](https://github.com/garyttierney/me3) to load the mod.
- An XInput controller. The mod screens don't support keyboard yet.
- For co-op, [Seamless Co-op](https://www.nexusmods.com/eldenring/mods/510) 2.0.1 (not included: each
  player puts its `SeamlessCoop` folder next to `Play RogueRing (Seamless).bat`).

## Building from source

Windows only. You need Rust (stable, MSVC), the .NET 10 SDK, Python 3.12 with Pillow, and the game
installed.

1. Third party code goes in `tools/vendor/` (not in this repo):

   ```
   git clone https://github.com/vswarte/fromsoftware-rs tools/vendor/fromsoftware-rs   (commit 59fbd3b)
   git clone https://github.com/vawser/Smithbox tools/vendor/Smithbox                  (commit 5a251110)
   git clone https://github.com/soulsmods/EldenRingHKS tools/vendor/EldenRingHKS       (commit d88d644)
   ```

2. Build the C# tools and copy `oo2core_6_win64.dll` from the game folder next to each one
   (`tools/Extractor/bin/Release/net10.0/` and `tools/ModBuilder/bin/Release/net10.0/`):

   ```
   dotnet build -c Release tools/Extractor
   dotnet build -c Release tools/ModBuilder
   ```

3. Extract what the mod needs from your copy of the game (read only) and turn it into images:

   ```
   tools/Extractor/bin/Release/net10.0/Extractor.exe "<ELDEN RING>/Game" extract event chr/c0000 map/mapstudio/m45_00_00_00 menu/hi/00_solo menu/hi/01_common menu/71_maptile
   copy "<ELDEN RING>/Game/regulation.bin" extract/
   tools/Extractor/bin/Release/net10.0/Extractor.exe ui .
   python tools/imagenes.py
   python tools/recursos_menu.py
   python tools/iconos_mapa.py
   python tools/map_background.py 2
   ```

4. Optional: the Monk's step and the Samurai's deflects use Bloodborne and Sekiro animations from
   Blubber's [Player Animation Repositories Collection](https://www.nexusmods.com/eldenring/mods/3027).
   Put the packs in `recursos/blubber_hunter` and `recursos/blubber_sekiro`. Without them the mod
   keeps the game's animations.

5. Build the mod and the DLL:

   ```
   tools/ModBuilder/bin/Release/net10.0/ModBuilder.exe .
   cargo build --release --manifest-path dll/Cargo.toml --no-default-features -p rogue-harness
   ```

6. Package it: `python tools/empaquetar.py <version>` builds `dist/RogueRing-<version>.zip` and checks
   it (missing images, test code in the DLL, stray or personal files).

The default build of the DLL includes a test bench that reads commands from a file next to it; the
player build (`--no-default-features`) does not. `docs/ARQUITECTURA.md` explains how the pieces fit
(in Spanish).

## Tests

```
cargo test --release --manifest-path dll/Cargo.toml -p rogue-core -p rogue-ui
python tools/auditoria.py
```

The first checks the run logic and that the data files agree with each other (every blessing, item,
event and encounter). The second looks for leftovers before publishing.

## License

RogueRing's own code and data are under the [MIT license](LICENSE): take the idea and build on it.
Credit is appreciated, not required. Game files and third party code keep their own licenses.

## Credits

See [CREDITS.md](CREDITS.md).
