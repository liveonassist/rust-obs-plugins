# rust-obs-plugins

> Forked from [TakiMoysha/rust-obs-plugins](https://github.com/TakiMoysha/rust-obs-plugins),
> which was forked from [bennetthardwick/rust-obs-plugins](https://github.com/bennetthardwick/rust-obs-plugins).

A safe Rust wrapper around the [OBS Studio](https://github.com/obsproject/obs-studio)
plugin API for building sources, filters, and effects, plus a small set of
plugins built on top of it.

> **Status:** the wrapper API is incomplete and may change. Expect breakage.

## Compatibility

| `obs-wrapper` | `obs-sys` | OBS Studio |
| ------------- | --------- | ---------- |
| _TBD_         | _TBD_     | _TBD_      |

Verified-compatible OBS releases will be filled in as the fork stabilizes.

## Repository layout

| Path                       | Description                                       |
| -------------------------- | ------------------------------------------------- |
| `/`                        | `obs-wrapper` — the safe Rust wrapper crate       |
| `/obs-sys`                 | Raw `bindgen` bindings against `<obs/obs.h>`      |
| `/plugins/avatar-plugin`   | Renders an avatar driven by keyboard/mouse input  |
| `/scripts`                 | Python helpers (`obsws-python`) for OBS testing   |

## Usage

Add the wrapper to your plugin crate's `Cargo.toml`, replacing
`<module-name>` with your plugin's name:

```toml
[dependencies]
obs-wrapper = "0.4"

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

| Trait                 | Description                                   | Builder method             |
| :-------------------- | :-------------------------------------------- | :------------------------- |
| `GetNameSource`       | Display name of the source                    | `.enable_get_name()`       |
| `GetWidthSource`      | Source width                                  | `.enable_get_width()`      |
| `GetHeightSource`     | Source height                                 | `.enable_get_height()`     |
| `VideoRenderSource`   | Video rendering                               | `.enable_video_render()`   |
| `AudioRenderSource`   | Audio rendering                               | `.enable_audio_render()`   |
| `UpdateSource`        | Settings updated                              | `.enable_update()`         |
| `GetPropertiesSource` | Define user-configurable properties           | `.enable_get_properties()` |
| `GetDefaultsSource`   | Default values for settings                   | `.enable_get_defaults()`   |
| `VideoTickSource`     | Per-video-frame tick                          | `.enable_video_tick()`     |
| `ActivateSource`      | Source becomes active                         | `.enable_activate()`       |
| `DeactivateSource`    | Source becomes inactive                       | `.enable_deactivate()`     |
| `MouseClickSource`    | Mouse clicks                                  | `.enable_mouse_click()`    |
| `MouseMoveSource`     | Mouse movement                                | `.enable_mouse_move()`     |
| `MouseWheelSource`    | Mouse wheel                                   | `.enable_mouse_wheel()`    |
| `KeyClickSource`      | Keyboard events                               | `.enable_key_click()`      |
| `FocusSource`         | Focus changes                                 | `.enable_focus()`          |
| `FilterVideoSource`   | Filter: process video data                    | `.enable_filter_video()`   |
| `FilterAudioSource`   | Filter: process audio data                    | `.enable_filter_audio()`   |

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

| Platform | Library      | Install location                    |
| -------- | ------------ | ----------------------------------- |
| Linux    | `*.so`       | `/usr/lib/obs-plugins/`             |
| macOS    | `*.dylib`    | `~/Library/Application Support/obs-studio/plugins/` |
| Windows  | `*.dll`      | `%PROGRAMFILES%\obs-studio\obs-plugins\64bit\` |

Plugin paths vary by OBS install method (Flatpak, Snap, portable, etc.) —
check your install for the right location.

## Development

The repo ships a Nix flake with the rust toolchain, `libclang` for
`obs-sys` bindgen, and `obs-studio` for headers and linking:

```sh
cp .envrc.template .envrc   # if you use direnv
direnv allow                # or: nix develop
```

The flake points `BINDGEN_EXTRA_CLANG_ARGS` at nixpkgs' OBS headers, so
you do **not** need to check out the `obs-sys/obs` submodule for local
development. If you do want headers from a specific upstream OBS:

```sh
git submodule update --init --recursive
git submodule update --remote obs-sys/obs   # bump pinned OBS
```

## License

Like [obs-studio](https://github.com/obsproject/obs-studio), this
project is licensed under GPL-2.0. See [LICENSE](./LICENSE).
