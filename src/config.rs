use crate::directory;
use crate::renderer::Interpolation;
use serde::*;
use std::fs::File;
use std::io::{BufReader, BufWriter};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn from_window(wnd: &wita::Window) -> Self {
        let pos = wnd.position();
        let size = wnd.inner_size();
        Self {
            x: pos.x,
            y: pos.y,
            width: size.width as _,
            height: size.height as _,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClearColor(pub f32, pub f32, pub f32);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RgbaColor(pub f32, pub f32, pub f32, pub f32);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Method {
    Open,
    Prev,
    Next,
    PrintMemory,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyBinding {
    pub method: Method,
    pub keys: Vec<Vec<wita::VirtualKey>>,
}

impl KeyBinding {
    fn new(method: Method, keys: Vec<Vec<wita::VirtualKey>>) -> Self {
        Self { method, keys }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub window: Rect,
    pub background: ClearColor,
    pub extensions: Vec<String>,
    pub lookahead: usize,
    pub order: directory::Order,
    pub comp: directory::Comparison,
    pub interpolation: Interpolation,
    pub worker_threads: usize,
    pub bmp_cache_size: usize,
    pub image_cache_size: usize,
    pub key_bindings: Vec<KeyBinding>,
}

impl Default for Config {
    fn default() -> Self {
        const LOOKAHEAD: usize = 5;
        Self {
            window: Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 480,
            },
            background: ClearColor(0.15, 0.15, 0.15),
            extensions: vec![
                "png".into(),
                "jpg".into(),
                "jpeg".into(),
                "bmp".into(),
                "ico".into(),
                "tif".into(),
                "tiff".into(),
                "pnm".into(),
                "pbm".into(),
                "pgm".into(),
                "ppm".into(),
                "tga".into(),
            ],
            lookahead: LOOKAHEAD,
            order: directory::Order::Name,
            comp: directory::Comparison::Ascending,
            interpolation: Interpolation::HighQualityCubic,
            worker_threads: {
                let n = num_cpus::get() / 2;
                match n {
                    0 => 1,
                    _ if n >= LOOKAHEAD => LOOKAHEAD,
                    _ => n,
                }
            },
            bmp_cache_size: 512 * 1024 * 1024,
            image_cache_size: 1024 * 1024 * 1024,
            key_bindings: vec![
                KeyBinding::new(Method::Open, vec![vec![wita::VirtualKey::Char('O')]]),
                KeyBinding::new(
                    Method::Prev,
                    vec![
                        vec![wita::VirtualKey::Char('A')],
                        vec![wita::VirtualKey::Left],
                    ],
                ),
                KeyBinding::new(
                    Method::Next,
                    vec![
                        vec![wita::VirtualKey::Char('D')],
                        vec![wita::VirtualKey::Right],
                    ],
                ),
                KeyBinding::new(
                    Method::PrintMemory,
                    vec![vec![wita::VirtualKey::F(1)]]
                )
            ],
        }
    }
}

pub fn read_config(path: impl AsRef<str>) -> Option<Config> {
    let file = File::open(path.as_ref()).ok()?;
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).ok()?
}

pub fn write_config(path: impl AsRef<str>, config: &Config) -> anyhow::Result<()> {
    let file = File::create(path.as_ref())?;
    let writer = BufWriter::new(file);
    Ok(serde_json::to_writer_pretty(writer, config)?)
}
