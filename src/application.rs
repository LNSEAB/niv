use crate::config::*;
use crate::dialog::file_open_dialog;
use crate::directory::Directory;
use crate::images::ImageManager;
use crate::renderer::*;
use log::{debug, error};
use std::fs::File;
use std::path::Path;
use winapi::um::combaseapi::CoInitializeEx;
use winapi::um::objbase::{COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
use winapi::um::winuser::*;

fn get_keyboard_delay() -> std::time::Duration {
    unsafe {
        let mut value = 0;
        SystemParametersInfoW(SPI_GETKEYBOARDDELAY, 0, &mut value as *mut _ as _, 0);
        std::time::Duration::from_millis((value + 1) * 250)
    }
}

pub struct Application {
    wnd: wita::Window,
    config: Config,
    images: ImageManager,
    renderer: Renderer,
    dir: Option<Directory>,
    pressed_keys: Vec<wita::VirtualKey>,
    keyboard_delay: std::time::Duration,
    pressed_time: std::time::Instant,
    print_memory: bool,
}

impl Application {
    pub fn new() -> anyhow::Result<Self> {
        simplelog::CombinedLogger::init(vec![
            simplelog::TermLogger::new(
                simplelog::LevelFilter::Debug,
                simplelog::Config::default(),
                simplelog::TerminalMode::Mixed,
            ),
            simplelog::WriteLogger::new(
                simplelog::LevelFilter::Info,
                simplelog::Config::default(),
                File::create("niv.log")?,
            ),
        ])
        .unwrap();
        unsafe {
            CoInitializeEx(
                std::ptr::null_mut(),
                COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE,
            );
        }
        let config = read_config("./config.json").unwrap_or_default();
        let wnd = wita::WindowBuilder::new()
            .title("niv")
            .position(wita::ScreenPosition::new(config.window.x, config.window.y))
            .inner_size(wita::PhysicalSize::new(
                config.window.width,
                config.window.height,
            ))
            .accept_drag_files(true)
            .build();
        wnd.disable_ime();
        let images = ImageManager::new(
            config.worker_threads,
            config.bmp_cache_size,
            config.image_cache_size,
        )?;
        let text_info = TextInfo {
            face_name: "Yu Gothic".into(),
            color: RgbaColor(1.0, 1.0, 1.0, 1.0),
            size: 14.0,
        };
        let renderer = Renderer::new(&wnd, text_info)?;
        let dir = None;
        Ok(Application {
            wnd,
            config,
            images,
            renderer,
            dir,
            pressed_keys: vec![],
            keyboard_delay: get_keyboard_delay(),
            pressed_time: std::time::Instant::now(),
            print_memory: false,
        })
    }
}

impl Application {
    fn open_entity(&mut self, path: impl AsRef<Path>) {
        let path = path.as_ref();
        let (dir_path, file) = if path.is_file() {
            (path.parent().unwrap(), Some(path))
        } else if path.is_dir() {
            (path, None)
        } else {
            return;
        };
        self.images.clear();
        self.dir = Some(Directory::new(
            dir_path,
            &self.config.extensions,
            self.config.order,
            self.config.comp,
            self.config.lookahead as isize,
            file,
        ));
        if let Some(current) = self.dir.as_ref().unwrap().current() {
            let wnd = self.wnd.proxy();
            let dc = self.renderer.device_context();
            self.images.load(dc, current, move |_| wnd.redraw());
        }
    }

    fn set_title(&self) {
        let (num, path) = if let Some(dir) = self.dir.as_ref() {
            let num = format!("{}/{}", dir.index() + 1, dir.len());
            let path = dir
                .current()
                .map_or(String::new(), |path| path.to_string_lossy().to_string());
            (num, path)
        } else {
            (String::new(), String::new())
        };
        self.wnd.set_title(&format!("niv {} {}", num, path))
    }
}

impl wita::EventHandler for Application {
    fn key_input(&mut self, wnd: &wita::Window, _: wita::KeyCode, state: wita::KeyState, prev_pressed: bool) {
        match state {
            wita::KeyState::Pressed => {
                if !prev_pressed {
                    self.pressed_time = std::time::Instant::now();
                }
                self.pressed_keys = wita::keyboard_state();
                self.pressed_keys.retain(|key| {
                    if let wita::VirtualKey::Other(i) = key {
                        *i < 233
                    } else {
                        true
                    }
                });
                let method = self.config.key_bindings.iter().find_map(|kb| {
                    kb.keys
                        .iter()
                        .find(|kk| kk.iter().all(|k| self.pressed_keys.contains(k)))
                        .map(|_| kb.method)
                });
                if let Some(method) = method {
                    if matches!(method, Method::PrintMemory) {
                        self.print_memory = !self.print_memory;
                    } else {
                        if let Some(dir) = self.dir.as_mut() {
                            let path = match method {
                                Method::Prev => dir.prev().first().cloned(),
                                Method::Next => dir.next().first().cloned(),
                                _ => None,
                            };
                            if let Some(path) = path {
                                let t = std::time::Instant::now();
                                if t - self.pressed_time <= self.keyboard_delay {
                                    let wnd = self.wnd.proxy();
                                    self.images.load(
                                        self.renderer.device_context(),
                                        &path,
                                        move |_| wnd.redraw(),
                                    );
                                    debug!("pressed key: load: {}", path.to_string_lossy());
                                }
                            }
                        }
                    }
                }
                self.set_title();
                self.wnd.redraw();
            }
            wita::KeyState::Released => {
                let method = self.config.key_bindings.iter().find_map(|kb| {
                    kb.keys
                        .iter()
                        .find(|kk| kk.iter().all(|k| self.pressed_keys.contains(k)))
                        .map(|_| kb.method)
                });
                if let Some(method) = method {
                    match method {
                        Method::Open => {
                            let path =
                                file_open_dialog(&self.config.extensions).unwrap_or_else(|e| {
                                    error!("open_dialog: {}", e);
                                    None
                                });
                            if let Some(path) = path {
                                debug!("open_dialog: {}", path.to_string_lossy());
                                self.open_entity(path);
                            }
                        }
                        Method::Prev | Method::Next => {
                            if let Some(dir) = self.dir.as_mut() {
                                let dc = self.renderer.device_context();
                                let path = dir.current().unwrap();
                                let proxy = wnd.proxy();
                                self.images.load(dc, &path, move |_| proxy.redraw());
                                debug!("released key: load: {}", path.to_string_lossy());
                            }
                        }
                        _ => (),
                    }
                }
                self.set_title();
                self.wnd.redraw();
            }
        }
    }

    fn drop_files(&mut self, wnd: &wita::Window, paths: &[&Path], _: wita::PhysicalPosition<f32>) {
        self.open_entity(paths[0]);
        self.set_title();
        wnd.redraw();
    }

    fn resizing(&mut self, _: &wita::Window, size: wita::PhysicalSize<u32>) {
        self.renderer.resize(size);
    }

    fn dpi_changed(&mut self, wnd: &wita::Window) {
        self.renderer.set_dpi(wnd.dpi() as f32);
    }

    fn draw(&mut self, _: &wita::Window) {
        let img = self
            .dir
            .as_ref()
            .and_then(|d| d.current())
            .and_then(|path| {
                let img = self.images.get(path);
                if let Err(e) = img {
                    error!("{}", e);
                    return None;
                }
                img.unwrap()
            });
        let text = if self.print_memory {
            Some(format!(
                "bmp: {}/{}(MB)\nimage: {}/{}(MB)",
                self.images.bmp_cache_size() as f32 / 1024.0 / 1024.0,
                self.config.bmp_cache_size as f32 / 1024.0 / 1024.0,
                self.images.image_cache_size() as f32 / 1024.0 / 1024.0,
                self.config.image_cache_size as f32 / 1024.0 / 1024.0
            ))
        } else {
            None
        };
        self.renderer.render(
            &self.config.background,
            img,
            self.config.interpolation,
            text
        );
    }

    fn closed(&mut self, wnd: &wita::Window) {
        self.config.window = Rect::from_window(wnd);
        if let Err(e) = write_config("./config.json", &self.config) {
            error!("write_config error: {}", e);
        }
    }
}
