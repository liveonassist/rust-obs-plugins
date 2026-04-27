use obs_rs::{obs_register_module, obs_rs_sys, obs_string, prelude::*, properties::*, source::*};
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic;

pub mod input_capture;
pub mod loader;

use loader::{Avatar, AvatarLoader, ImageData};

/// Кэш текстур для предотвращения повторной загрузки
struct TextureCache {
    /// Карта путь -> текстура OBS (raw pointer)
    textures: HashMap<PathBuf, *mut obs_rs_sys::gs_texture_t>,
}

unsafe impl Send for TextureCache {}
unsafe impl Sync for TextureCache {}

impl TextureCache {
    fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    /// Получить текстуру или создать новую из ImageData
    /// Должно вызываться только в графическом контексте (video_render)
    fn get_or_create(&mut self, image: &ImageData) -> Option<*mut obs_rs_sys::gs_texture_t> {
        if !self.textures.contains_key(&image.path) {
            unsafe {
                let data_ptr = image.data.as_ptr();
                let mut data_ptr_ptr = data_ptr;

                let texture = obs_rs_sys::gs_texture_create(
                    image.width,
                    image.height,
                    obs_rs_sys::gs_color_format_GS_RGBA,
                    1,
                    &mut data_ptr_ptr as *mut *const u8,
                    0,
                );

                if !texture.is_null() {
                    self.textures.insert(image.path.clone(), texture);
                }
            }
        }

        self.textures.get(&image.path).copied()
    }

    /// Очистить кэш
    fn clear(&mut self) {
        unsafe {
            for (_, texture) in self.textures.drain() {
                obs_rs_sys::gs_texture_destroy(texture);
            }
        }
    }
}

impl Drop for TextureCache {
    fn drop(&mut self) {
        self.clear();
    }
}

/// Главный источник аватара
#[allow(dead_code)] // a few fields are state held for OBS lifetime guarantees and aren't read directly
struct AvatarSource {
    /// Ссылка на источник
    source: SourceRef,

    /// Avatar loader с кэшированием
    loader: AvatarLoader,

    /// Кэш текстур OBS
    texture_cache: TextureCache,

    /// Загруженный аватар
    avatar: Option<Avatar>,

    /// Текущий активный режим
    current_mode: String,

    /// Текущее выражение лица (None = нет лица)
    current_face: Option<String>,

    /// Состояние рук (левая и правая): текущий кадр анимации
    left_hand_frame: usize,
    right_hand_frame: usize,

    /// Нажатые клавиши (для анимации)
    pressed_keys: std::collections::HashSet<String>,

    /// Текущий уровень аудио (0.0 - 1.0)
    audio_level: f32,

    /// Флаг для определения, говорит ли аватар
    is_speaking: bool,

    /// Порог для определения речи
    speech_threshold: f32,

    /// Path to avatar_config.json
    avatar_path: PathBuf,

    /// Ширина и высота canvas
    width: u32,
    height: u32,

    /// Input capture для перехвата клавиш (только для Wayland)
    #[cfg(all(target_os = "linux", feature = "wayland"))]
    input_capture: Option<input_capture::InputCapture>,
}

impl Sourceable for AvatarSource {
    fn get_id() -> ObsString {
        obs_string!("avatar_source")
    }

    fn get_type() -> SourceType {
        SourceType::Input
    }

    fn create(create: &mut CreatableSourceContext<Self>, source: SourceRef) -> Self {
        let settings = &create.settings;

        // Получаем путь к директории аватара
        let avatar_path = settings
            .get::<Cow<'_, str>>(obs_string!("avatar_path"))
            .map(|s| PathBuf::from(s.as_ref()))
            .unwrap_or_else(|| PathBuf::from("./assets/bongo_cat"));

        println!("Avatar path: {}", avatar_path.display());
        let width = settings.get(obs_string!("width")).unwrap_or(1280);
        let height = settings.get(obs_string!("height")).unwrap_or(768);
        let speech_threshold = settings
            .get(obs_string!("speech_threshold"))
            .unwrap_or(0.15);

        let current_mode = settings
            .get::<Cow<'_, str>>(obs_string!("mode"))
            .map(|s| s.to_string())
            .unwrap_or_else(|| "keyboard".to_string());

        // Загружаем аватар из конфиг-файла
        let avatar = if avatar_path.is_file() {
            println!("Loading avatar from config file: {}", avatar_path.display());
            match Avatar::load_from_config(&avatar_path) {
                Ok(av) => {
                    println!("✓\tAvatar loaded successfully!");
                    println!("\tName: {}", av.name);
                    println!("\tAvailable modes: {:?}", av.available_modes);
                    println!("\tFace images: {} loaded", av.face_images.len());
                    println!("\tModes loaded: {}", av.modes.len());

                    // Детальная информация о текущем режиме
                    if let Some(mode) = av.get_mode(&current_mode) {
                        println!("\n  Current mode '{}' details:", current_mode);
                        let current_face: Option<String> = None; // This variable is not used elsewhere, so it's fine to define it here.
                        println!("    Current face: {:?}", current_face);
                        println!("    Background: {}", mode.background.is_some());
                        println!("    Cat background: {}", mode.cat_background.is_some());
                        println!("    Left hand: {}", mode.left_hand.is_some());
                        if let Some(ref lh) = mode.left_hand {
                            println!("      - up_image: {}", lh.up_image.path.display());
                            println!("      - frame_images: {}", lh.frame_images.len());
                        }
                        println!("    Right hand: {}", mode.right_hand.is_some());
                        if let Some(ref rh) = mode.right_hand {
                            println!("      - up_image: {}", rh.up_image.path.display());
                            println!("      - frame_images: {}", rh.frame_images.len());
                        }
                        println!("    Key images: {} keys", mode.key_images.len());
                        for key in mode.key_images.keys() {
                            println!("      - {}", key);
                        }
                    } else {
                        eprintln!("  ✗ WARNING: Current mode '{}' not found!", current_mode);
                        eprintln!("     Available modes: {:?}", av.available_modes);
                    }

                    Some(av)
                }
                Err(e) => {
                    eprintln!("✗ Failed to load avatar from config: {:?}", e);
                    None
                }
            }
        } else if avatar_path.is_dir() {
            println!("Loading avatar from directory: {}", avatar_path.display());
            match Avatar::load_from_file(&avatar_path) {
                Ok(av) => {
                    println!("✓ Avatar loaded successfully!");
                    println!("  Name: {}", av.name);
                    println!("  Available modes: {:?}", av.available_modes);
                    Some(av)
                }
                Err(e) => {
                    eprintln!("✗ Failed to load avatar from directory: {:?}", e);
                    None
                }
            }
        } else {
            eprintln!(
                "✗ Avatar path is neither file nor directory: {}",
                avatar_path.display()
            );
            None
        };

        if avatar.is_none() {
            eprintln!("Failed to load avatar from: {:?}", avatar_path);
        }

        Self {
            source,
            loader: AvatarLoader::new(),
            texture_cache: TextureCache::new(),
            avatar,
            current_mode,
            current_face: None, // По умолчанию нет лица,
            left_hand_frame: 0,
            right_hand_frame: 0,
            pressed_keys: std::collections::HashSet::new(),
            audio_level: 0.0,
            is_speaking: false,
            speech_threshold,
            avatar_path,
            width,
            height,

            #[cfg(all(target_os = "linux", feature = "wayland"))]
            input_capture: {
                match input_capture::InputCapture::new() {
                    Ok(capture) => {
                        println!("✓ Input capture initialized (polling mode)");
                        Some(capture)
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to initialize input capture: {:?}", e);
                        None
                    }
                }
            },
        }
    }
}

impl GetNameSource for AvatarSource {
    fn get_name() -> ObsString {
        obs_string!("Avatar Source")
    }
}

impl GetWidthSource for AvatarSource {
    fn get_width(&mut self) -> u32 {
        self.width
    }
}

impl GetHeightSource for AvatarSource {
    fn get_height(&mut self) -> u32 {
        self.height
    }
}

impl GetPropertiesSource for AvatarSource {
    fn get_properties(&mut self) -> Properties {
        let mut properties = Properties::new();

        // Path to avatar config.json
        properties.add(
            obs_string!("avatar_path"),
            obs_string!("Avatar JSON file"),
            PathProp::new(PathType::File),
        );

        // Текущий режим (текстовое поле)
        properties.add(
            obs_string!("mode"),
            obs_string!("Current Mode (e.g., keyboard, standard)"),
            TextProp::new(TextType::Default),
        );

        // Размеры canvas
        properties.add(
            obs_string!("width"),
            obs_string!("Canvas Width"),
            NumberProp::new_int().with_range(100u32..=3840),
        );

        properties.add(
            obs_string!("height"),
            obs_string!("Canvas Height"),
            NumberProp::new_int().with_range(100u32..=2160),
        );

        // Порог для определения речи
        properties.add(
            obs_string!("speech_threshold"),
            obs_string!("Speech Detection Threshold"),
            NumberProp::new_float(0.01)
                .with_range(0.0..=1.0)
                .with_slider(),
        );

        // Скорость анимации
        properties.add(
            obs_string!("animation_speed"),
            obs_string!("Animation Speed"),
            NumberProp::new_float(0.1)
                .with_range(0.1..=20.0)
                .with_slider(),
        );

        properties
    }
}

impl UpdateSource for AvatarSource {
    fn update(&mut self, settings: &mut DataObj, _context: &mut GlobalContext) {
        // Обновляем путь к аватару и перезагружаем если изменился
        if let Some(path) = settings.get::<Cow<'_, str>>(obs_string!("avatar_path")) {
            let new_path = PathBuf::from(path.as_ref());
            if new_path != self.avatar_path {
                self.avatar_path = new_path.clone();
                // Очищаем кэш текстур
                self.texture_cache.clear();

                // Перезагружаем аватар
                self.avatar = if new_path.is_file() {
                    Avatar::load_from_config(&new_path).ok()
                } else if new_path.is_dir() {
                    Avatar::load_from_file(&new_path).ok()
                } else {
                    None
                };

                if self.avatar.is_none() {
                    eprintln!("Failed to reload avatar from: {:?}", new_path);
                }
            }
        }

        // Обновляем текущий режим
        if let Some(mode) = settings.get::<Cow<'_, str>>(obs_string!("mode")) {
            self.current_mode = mode.to_string();
        }

        if let Some(width) = settings.get(obs_string!("width")) {
            self.width = width;
        }

        if let Some(height) = settings.get(obs_string!("height")) {
            self.height = height;
        }

        if let Some(threshold) = settings.get(obs_string!("speech_threshold")) {
            self.speech_threshold = threshold;
        }
    }
}

impl VideoTickSource for AvatarSource {
    fn video_tick(&mut self, _seconds: f32) {
        // Опрашиваем input capture (Wayland)
        #[cfg(all(target_os = "linux", feature = "wayland"))]
        if let Some(ref mut capture) = self.input_capture {
            let events = capture.poll();
            for event in events {
                match event {
                    input_capture::InputEvent::KeyPress(key) => {
                        println!("🎹 Key PRESSED: {} (0x{:04X})", key, key);
                        self.pressed_keys.insert(key.to_string());

                        // Показываем распространенные клавиши
                        match key {
                            1 => println!("   → ESC"),
                            28 => println!("   → ENTER"),
                            57 => println!("   → SPACE"),
                            30 => println!("   → A"),
                            48 => println!("   → B"),
                            _ => {}
                        }
                    }
                    input_capture::InputEvent::KeyRelease(key) => {
                        println!("🎹 Key RELEASED: {} (0x{:04X})", key, key);
                        self.pressed_keys.remove(&key.to_string());
                    }
                    // if !running.load(Ordering::Relaxed) {
                    //     break;
                    // }
                    _ => {}
                }
            }
        }

        // Обновляем состояние речи на основе уровня аудио
        self.is_speaking = self.audio_level > self.speech_threshold;

        // TODO: Анимация рук на основе нажатых клавиш
        // TODO: Анимация рта при речи
    }
}

impl VideoRenderSource for AvatarSource {
    fn video_render(&mut self, _context: &mut GlobalContext, _render: &mut VideoRenderContext) {
        // Деструктуризация для раздельного заимствования полей
        let Self {
            texture_cache,
            avatar,
            current_mode,
            current_face,
            pressed_keys,
            ..
        } = self;

        let Some(avatar) = avatar.as_ref() else {
            return;
        };

        let Some(mode) = avatar.get_mode(current_mode) else {
            static LOGGED_NO_MODE: atomic::AtomicBool = atomic::AtomicBool::new(false);
            if !LOGGED_NO_MODE.load(atomic::Ordering::Relaxed) {
                eprintln!(
                    "✗ Mode '{}' not found. Available modes: {:?}",
                    current_mode, avatar.available_modes
                );
                LOGGED_NO_MODE.store(true, atomic::Ordering::Relaxed);
            }
            return;
        };

        // Отладочный вывод один раз
        static FIRST_RENDER: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(true);
        if FIRST_RENDER.load(std::sync::atomic::Ordering::Relaxed) {
            println!("\n=== AVATAR RENDERING ===");
            println!("Mode: {}", current_mode);
            println!("Face: {:?}", current_face);
            println!("Has background: {}", mode.background.is_some());
            println!("Has cat_background: {}", mode.cat_background.is_some());
            println!("Has left_hand: {}", mode.left_hand.is_some());
            println!("Has right_hand: {}", mode.right_hand.is_some());
            println!("Left hand key frames: {}", mode.left_hand_key_frames.len());
            println!(
                "Right hand key frames: {}",
                mode.right_hand_key_frames.len()
            );
            println!("Key images: {}", mode.key_images.len());
            println!("=========================\n");
            FIRST_RENDER.store(false, std::sync::atomic::Ordering::Relaxed);
        }

        // Хелпер для рисования спрайта
        // Изолируем unsafe в отдельную функцию для ясности
        let draw_sprite = |texture_cache: &mut TextureCache, image: &ImageData, x: f32, y: f32| {
            if let Some(tex_ptr) = texture_cache.get_or_create(image) {
                // Unsafe блок изолирован и понятен что делает
                unsafe {
                    // ✅ ИСПОЛЬЗУЕМ obs_source_draw КАК В C++ ВЕРСИИ
                    // Это правильный способ для source (не filter)
                    obs_rs_sys::obs_source_draw(
                        tex_ptr, x as i32, // x position
                        y as i32, // y position
                        0,        // cx (0 = use texture width)
                        0,        // cy (0 = use texture height)
                        false,    // flip vertically
                    );
                }
            }
        };

        // ===== РЕНДЕРИМ ВСЕ СЛОИ (безопасная логика) =====

        // 1. Отрисовка фона
        if let Some(ref bg) = mode.background {
            draw_sprite(texture_cache, bg, 0.0, 0.0);
        }

        // 2. Отрисовка тела кота
        if let Some(ref cat) = mode.cat_background {
            draw_sprite(texture_cache, cat, 0.0, 0.0);
        }

        // 3. Отрисовка лица
        if let Some(face_name) = current_face
            && let Some(face) = avatar.face_images.get(face_name)
        {
            draw_sprite(texture_cache, face, 0.0, 0.0);
        }

        // 4. Отрисовка нажатых клавиш (перед руками, чтобы руки были сверху)
        for (key_str, key_image) in &mode.key_images {
            // Пытаемся распарсить строку ключа как keycode
            if let Ok(key_code) = key_str.parse::<u32>() {
                // Проверяем, нажата ли эта клавиша
                if pressed_keys.contains(&key_code.to_string()) {
                    draw_sprite(texture_cache, key_image, 0.0, 0.0);
                }
            }
        }

        // 5. Определяем, какие руки нажаты и какие кадры использовать
        let mut left_hand_pressed_key: Option<u32> = None;
        let mut right_hand_pressed_key: Option<u32> = None;

        // Проверяем все нажатые клавиши
        for key_str in pressed_keys.iter() {
            if let Ok(key_code) = key_str.parse::<u32>() {
                // Проверяем левую руку
                if mode.left_hand_key_frames.contains_key(&key_code) {
                    left_hand_pressed_key = Some(key_code);
                }

                // Проверяем правую руку
                if mode.right_hand_key_frames.contains_key(&key_code) {
                    right_hand_pressed_key = Some(key_code);
                }
            }
        }

        // 6. Отрисовка левой руки с анимацией нажатия клавиш
        if let Some(ref hand) = mode.left_hand {
            // Если есть нажатая клавиша с кадром анимации, используем его
            if let Some(key_code) = left_hand_pressed_key {
                if let Some(frame_image) = mode.left_hand_key_frames.get(&key_code) {
                    draw_sprite(texture_cache, frame_image, 0.0, 0.0);
                } else {
                    // Fallback на поднятую руку
                    draw_sprite(texture_cache, &hand.up_image, 0.0, 0.0);
                }
            } else {
                // Рука поднята (нет нажатых клавиш)
                draw_sprite(texture_cache, &hand.up_image, 0.0, 0.0);
            }
        }

        // 7. Отрисовка правой руки с анимацией нажатия клавиш
        if let Some(ref hand) = mode.right_hand {
            // Если есть нажатая клавиша с кадром анимации, используем его
            if let Some(key_code) = right_hand_pressed_key {
                if let Some(frame_image) = mode.right_hand_key_frames.get(&key_code) {
                    draw_sprite(texture_cache, frame_image, 0.0, 0.0);
                } else {
                    // Fallback на поднятую руку
                    draw_sprite(texture_cache, &hand.up_image, 0.0, 0.0);
                }
            } else {
                // Рука поднята (нет нажатых клавиш)
                draw_sprite(texture_cache, &hand.up_image, 0.0, 0.0);
            }
        }

        // Отладочный вывод (реже)
        use std::sync::atomic::{AtomicUsize, Ordering};
        static FRAME_COUNT: AtomicUsize = AtomicUsize::new(0);
        let frame = FRAME_COUNT.fetch_add(1, Ordering::Relaxed);
        if frame.is_multiple_of(300) {
            println!("✓ Rendered frame {}", frame);
        }
    }
}

impl KeyClickSource for AvatarSource {
    fn key_click(&mut self, event: obs_rs_sys::obs_key_event, pressed: bool) {
        let Some(ref avatar) = self.avatar else {
            return;
        };

        // Простой маппинг vkey -> string
        let key_str = match event.native_vkey {
            48..=57 => format!("{}", (event.native_vkey - 48) as u8 as char), // 0-9
            65..=90 => format!("{}", (event.native_vkey) as u8 as char).to_lowercase(), // a-z
            112..=123 => format!("f{}", event.native_vkey - 111),             // f1-f12
            27 => "escape".to_string(),
            _ => "unknown".to_string(),
        };

        if pressed {
            // Добавляем в набор нажатых клавиш
            self.pressed_keys.insert(key_str.clone());

            // Логика переключения лиц по клавишам 1-4
            let face_id = match key_str.as_str() {
                "1" => Some("f1"),
                "2" => Some("f2"),
                "3" => Some("f3"),
                "4" => Some("f4"),
                "0" | "escape" => None, // Сброс лица
                _ => None,
            };

            if let Some(fid) = face_id {
                // Проверяем существует ли такое лицо
                if avatar.face_images.contains_key(fid) {
                    println!("Switching to face: {}", fid);
                    self.current_face = Some(fid.to_string());
                }
            } else if key_str == "0" || key_str == "escape" {
                println!("Clearing face");
                self.current_face = None;
            }

            // Проверяем, есть ли это выражение лица (из конфига)
            if let Some(_face_img) = avatar.get_face_by_key(&key_str) {
                self.current_face = Some(key_str.clone());
            }

            // Проверяем, есть ли это клавиша в текущем режиме
            if let Some(mode) = avatar.get_mode(&self.current_mode)
                && let Some(_key_img) = mode.key_images.get(&key_str)
            {
                // TODO: Показать анимацию нажатия клавиши
                // TODO: Анимировать руки
            }
        } else {
            // Убираем из набора нажатых клавиш
            self.pressed_keys.remove(&key_str);
        }
    }
}

impl MouseClickSource for AvatarSource {
    fn mouse_click(
        &mut self,
        _event: obs_rs_sys::obs_mouse_event,
        button: MouseButton,
        pressed: bool,
        _click_count: u8,
    ) {
        if !pressed {
            return;
        }

        // TODO: Добавить логику реакции на клики мыши
        match button {
            MouseButton::Left => {
                // Например, показать указывающий жест
                // self.point_gesture();
            }
            MouseButton::Right => {
                // Другая реакция
            }
            _ => {}
        }
    }
}

impl MouseMoveSource for AvatarSource {
    fn mouse_move(&mut self, _event: obs_rs_sys::obs_mouse_event, _leave: bool) {
        // TODO: Добавить логику отслеживания мыши глазами аватара
        // let mouse_x = event.x;
        // let mouse_y = event.y;

        // self.look_at(mouse_x, mouse_y);
    }
}

// impl FilterAudioSource для обработки аудио входа
// Если вы хотите, чтобы это был фильтр, а не источник
// Раскомментируйте этот блок и измените get_type() на SourceType::Filter

/*
impl FilterAudioSource for AvatarSource {
    fn filter_audio(&mut self, audio: &mut AudioDataContext) {
        // Вычисляем уровень аудио для определения речи
        if let Some(channel_data) = audio.get_channel_as_mut_slice(0) {
            let mut sum = 0.0;
            for sample in channel_data.iter() {
                sum += sample.abs();
            }

            self.audio_level = sum / channel_data.len() as f32;
        }
    }
}
*/

// Plugin Module
struct AvatarModule {
    context: ModuleRef,
}

impl Module for AvatarModule {
    fn new(context: ModuleRef) -> Self {
        Self { context }
    }

    fn get_ctx(&self) -> &ModuleRef {
        &self.context
    }

    fn load(&mut self, load_context: &mut LoadContext) -> bool {
        let source = load_context
            .create_source_builder::<AvatarSource>()
            .enable_get_name()
            .enable_get_width()
            .enable_get_height()
            .enable_get_properties()
            .enable_update()
            .enable_video_tick()
            .enable_video_render()
            .enable_key_click()
            .enable_mouse_click()
            .enable_mouse_move()
            // TODO: Uncomment when FilterAudioSource is implemented
            // .enable_filter_audio()
            .build();

        load_context.register_source(source);

        true
    }

    fn unload(&mut self) {
        println!("Avatar Plugin: Unloading module...");
        // Note: Resources (textures, input devices) are automatically cleaned up
        // when AvatarSource instances are dropped by OBS.
        // No manual cleanup is required here for the current architecture.
        println!("Avatar Plugin: Module unloaded successfully.");
    }

    fn description() -> ObsString {
        obs_string!(
            "A virtual avatar with animated PNG parts that respond to keyboard, mouse, and audio input events."
        )
    }

    fn name() -> ObsString {
        obs_string!("Avatar Plugin")
    }

    fn author() -> ObsString {
        obs_string!("TakiMoysha")
    }
}

obs_register_module!(AvatarModule);
