# obs-rs

> Forked from [TakiMoysha/rust-obs-plugins](https://github.com/TakiMoysha/rust-obs-plugins),
> which was forked from [bennetthardwick/rust-obs-plugins](https://github.com/bennetthardwick/rust-obs-plugins).

A safe Rust wrapper around the [OBS Studio](https://github.com/obsproject/obs-studio)
plugin API for building sources, filters, and effects, plus a small set of
plugins built on top of it.

> **Status:** the wrapper API is incomplete and may change. Expect breakage.

## Compatibility

This crate supports the OBS Studio v30, v31, and v32 release lines. A single
crate version targets a _range_ of supported OBS versions; you pick which one
to build against via a Cargo feature on `obs-rs` (and, on NixOS, the
matching dev shell).

| `obs-rs` | `obs-sys-rs` | OBS Studio line | Cargo feature      | Nix dev shell               |
| -------- | ------------ | --------------- | ------------------ | --------------------------- |
| 0.5.x    | 0.4.x        | v30 (≥ 30.2.3)  | `obs-30`           | `nix develop .#obs-v30`     |
| 0.5.x    | 0.4.x        | v31 (≥ 31.0.3)  | `obs-31`           | `nix develop .#obs-v31`     |
| 0.5.x    | 0.4.x        | v32 (≥ 32.1.0)  | `obs-32` (default) | `nix develop` (= `obs-v32`) |

The default feature is `obs-32` — the latest stable OBS major. To build against
an older major, set `default-features = false` and pick exactly one `obs-XX`
feature in your plugin's `Cargo.toml`:

```toml
[dependencies]
obs-rs = { version = "0.5", default-features = false, features = ["obs-31"] }
```

`obs-sys-rs`'s `build.rs` hard-errors if the selected feature doesn't match the
OBS major version checked out in `obs-sys-rs/obs-v{N}`, or — when detectable —
the major version of the libobs your machine will link against. Detection
sources, in order: `OBS_LIBRARY_MAJOR_VER` env var (always honored); on Linux
`pkg-config --modversion libobs` then `libobs.so.<major>` symlinks under
`LD_LIBRARY_PATH` / standard lib dirs; on macOS the bundled
`Info.plist` of the OBS install; on Windows the `DisplayVersion` value under
the OBS Studio uninstall registry key.

## Repository layout

| Path                     | Description                                      |
| ------------------------ | ------------------------------------------------ |
| `/`                      | `obs-rs` — the safe Rust wrapper crate           |
| `/obs-sys-rs`            | Raw `bindgen` bindings against `<obs/obs.h>`     |
| `/plugins/avatar-plugin` | Renders an avatar driven by keyboard/mouse input |
| `/scripts`               | Python helpers (`obsws-python`) for OBS testing  |

## Usage

Add the wrapper to your plugin crate's `Cargo.toml`, replacing
`<module-name>` with your plugin's name:

```toml
[dependencies]
obs-rs = "0.4"

[lib]
name = "<module-name>"
crate-type = ["cdylib"]
```

The shape of a plugin is:

1. Create a struct that implements `Module`.
2. Create a struct that holds your source/filter state.
3. Implement the source traits you need.
4. Enable those traits in the module's `load` method.

```rust
use obs_wrapper::{
    prelude::*,
    source::*,
    obs_register_module,
    obs_string,
};

struct TestModule {
    context: ModuleRef,
}

struct TestSource;

impl Sourceable for TestSource {
    fn get_id() -> ObsString { obs_string!("test_source") }
    fn get_type() -> SourceType { SourceType::Filter }
    fn create(_create: &mut CreatableSourceContext<Self>, _source: SourceContext) -> Self { Self }
}

impl GetNameSource for TestSource {
    fn get_name() -> ObsString { obs_string!("Test Source") }
}

impl Module for TestModule {
    fn new(context: ModuleRef) -> Self { Self { context } }
    fn get_ctx(&self) -> &ModuleRef { &self.context }

    fn description() -> ObsString { obs_string!("A test source") }
    fn name() -> ObsString { obs_string!("Test Module") }
    fn author() -> ObsString { obs_string!("you") }

    fn load(&mut self, load_context: &mut LoadContext) -> bool {
        let source = load_context
            .create_source_builder::<TestSource>()
            .enable_get_name()
            .build();
        load_context.register_source(source);
        true
    }
}

obs_register_module!(TestModule);
```

> If your plugin spawns threads, signal them to stop from the `unload` method.

### Source traits

Each trait must be enabled on the source builder via the matching
`.enable_*()` call so OBS sees the corresponding callback.

| Trait                 | Description                         | Builder method             |
| :-------------------- | :---------------------------------- | :------------------------- |
| `GetNameSource`       | Display name of the source          | `.enable_get_name()`       |
| `GetWidthSource`      | Source width                        | `.enable_get_width()`      |
| `GetHeightSource`     | Source height                       | `.enable_get_height()`     |
| `VideoRenderSource`   | Video rendering                     | `.enable_video_render()`   |
| `AudioRenderSource`   | Audio rendering                     | `.enable_audio_render()`   |
| `UpdateSource`        | Settings updated                    | `.enable_update()`         |
| `GetPropertiesSource` | Define user-configurable properties | `.enable_get_properties()` |
| `GetDefaultsSource`   | Default values for settings         | `.enable_get_defaults()`   |
| `VideoTickSource`     | Per-video-frame tick                | `.enable_video_tick()`     |
| `ActivateSource`      | Source becomes active               | `.enable_activate()`       |
| `DeactivateSource`    | Source becomes inactive             | `.enable_deactivate()`     |
| `MouseClickSource`    | Mouse clicks                        | `.enable_mouse_click()`    |
| `MouseMoveSource`     | Mouse movement                      | `.enable_mouse_move()`     |
| `MouseWheelSource`    | Mouse wheel                         | `.enable_mouse_wheel()`    |
| `KeyClickSource`      | Keyboard events                     | `.enable_key_click()`      |
| `FocusSource`         | Focus changes                       | `.enable_focus()`          |
| `FilterVideoSource`   | Filter: process video data          | `.enable_filter_video()`   |
| `FilterAudioSource`   | Filter: process audio data          | `.enable_filter_audio()`   |

### Property types (`GetPropertiesSource`)

- `NumberProp` — integer or float; can render as a slider.
- `BoolProp` — checkbox.
- `TextProp` — text input (default, password, multiline).
- `ColorProp` — color picker.
- `PathProp` — file or directory picker.
- `ListProp` — dropdown (via `props.add_list`).
- `FontProp` — font selection.
- `EditableListProp` — editable list of strings or files.

### Logging

Use `obs_wrapper::log` rather than `println!` so output lands in the OBS
log:

```rust
obs_wrapper::log::info!("audio level: {}", self.audio_level);
```

## Building and installing a plugin

```sh
cargo build --release
```

Then copy the resulting shared library into OBS's plugins directory:

| Platform | Library   | Install location                                    |
| -------- | --------- | --------------------------------------------------- |
| Linux    | `*.so`    | `/usr/lib/obs-plugins/`                             |
| macOS    | `*.dylib` | `~/Library/Application Support/obs-studio/plugins/` |
| Windows  | `*.dll`   | `%PROGRAMFILES%\obs-studio\obs-plugins\64bit\`      |

Plugin paths vary by OBS install method (Flatpak, Snap, portable, etc.) —
check your install for the right location.

## Development

The repo ships a Nix flake with one dev shell per supported OBS major. Each
shell pins both the libraries (`libobs`, `libobs-frontend-api`) and the
matching `obs-studio` package version, plus the rust toolchain and
`libclang` for `obs-sys-rs` bindgen:

```sh
cp .envrc.template .envrc          # if you use direnv
direnv allow                       # or: nix develop  (= obs-v32)
nix develop .#obs-v30              # target OBS v30
nix develop .#obs-v31              # target OBS v31
nix develop .#obs-v32              # target OBS v32 (also the default)
```

Each shell exports `OBS_LIBRARY_MAJOR_VER=<N>` so `obs-sys-rs`'s build script can
verify the linked libobs matches the selected `obs-XX` feature without you
having to think about it.

OBS **headers** come from per-version git submodules under `obs-sys-rs/`, not
from nixpkgs — the submodule pin is the OBS version this repo builds
against. Each major has its own pinned submodule:

```sh
git submodule update --init obs-sys-rs/obs-v30          # only v30
git submodule update --init obs-sys-rs/obs-v31          # only v31
git submodule update --init obs-sys-rs/obs-v32          # only v32
git submodule update --init --recursive              # all three
```

To bump a major's pin to a newer release in the same line:

```sh
git -C obs-sys-rs/obs-v32 fetch --tags
git -C obs-sys-rs/obs-v32 checkout <tag>                 # e.g. 32.1.3
git add obs-sys-rs/obs-v32 && git commit
```

If the submodule for the _default_ major (`obs-v32`) isn't checked out,
`obs-sys-rs`'s `build.rs` falls back to its pre-generated bindings — fine for
casual builds, but the compatibility table above only applies to the pinned
submodule revision. Non-default majors (`obs-30`, `obs-31`) require their
submodule to be initialized; there is no fallback for them.

## License

Like [obs-studio](https://github.com/obsproject/obs-studio), this
project is licensed under GPL-2.0. See [LICENSE](./LICENSE).
