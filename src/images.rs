use crate::error::Error;
use com_ptr::*;
use image::RgbaImage;
use std::collections::hash_map::DefaultHasher;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use winapi::shared::dxgiformat::*;
use winapi::um::{d2d1_1::*, dcommon::*};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct PathHash(u64);

trait GetSize {
    fn get_size(&self) -> usize;
}

impl GetSize for ComPtr<ID2D1Bitmap1> {
    fn get_size(&self) -> usize {
        unsafe {
            let size = self.GetPixelSize();
            (size.width * size.height * 4) as usize
        }
    }
}

impl GetSize for RgbaImage {
    fn get_size(&self) -> usize {
        self.as_raw().len()
    }
}

#[derive(Debug)]
struct Cache<T: GetSize> {
    buffer: VecDeque<(PathHash, T)>,
    size: usize,
    target_size: usize,
}

impl<T: GetSize> Cache<T> {
    fn new(target_size: usize) -> Self {
        Self {
            buffer: VecDeque::new(),
            size: 0,
            target_size,
        }
    }

    fn size(&self) -> usize {
        self.size
    }

    fn clear(&mut self) {
        self.buffer.clear();
    }

    fn find(&self, path: PathHash) -> Option<&T> {
        self.buffer
            .iter()
            .find(|(p, _)| *p == path)
            .map(|(_, obj)| obj)
    }

    fn push(&mut self, path: PathHash, obj: T) {
        if self.find(path).is_some() {
            return;
        }
        let push_size = obj.get_size();
        while self.size + push_size > self.target_size {
            let item = self.buffer.pop_front().unwrap();
            self.size -= item.1.get_size();
        }
        self.buffer.push_back((path, obj));
        self.size += push_size;
    }
}

fn to_path_hash(path: impl AsRef<Path>) -> PathHash {
    let mut hasher = DefaultHasher::new();
    path.as_ref().hash(&mut hasher);
    PathHash(hasher.finish())
}

type BitmapCache = Arc<Mutex<Cache<ComPtr<ID2D1Bitmap1>>>>;
type ImageCache = Arc<Mutex<Cache<RgbaImage>>>;

async fn load_image(
    dc: ComPtr<ID2D1DeviceContext>,
    path: PathBuf,
    path_hash: PathHash,
    bmp_cache: BitmapCache,
    image_cache: ImageCache,
) -> Result<(), Error> {
    let mut bmp_cache = bmp_cache.lock().await;
    if bmp_cache.find(path_hash).is_some() {
        return Ok(());
    }
    let mut image_cache = image_cache.lock().await;
    let img = match image_cache.find(path_hash) {
        Some(img) => img,
        None => {
            image_cache.push(path_hash, image::open(path)?.to_rgba8());
            image_cache.find(path_hash).unwrap()
        }
    };
    let bmp = ComPtr::new(|| unsafe {
        let mut obj = std::ptr::null_mut();
        let size = img.dimensions();
        let ret = dc.CreateBitmap(
            winapi::um::d2d1::D2D1_SIZE_U {
                width: size.0,
                height: size.1,
            },
            img.as_raw().as_ptr() as _,
            size.0 * 4,
            &D2D1_BITMAP_PROPERTIES1 {
                bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                colorContext: std::ptr::null_mut(),
            },
            &mut obj,
        );
        hresult(obj, ret)
    })?;
    bmp_cache.push(path_hash, bmp);
    Ok(())
}

#[derive(Debug)]
pub struct ImageManager {
    runtime: tokio::runtime::Runtime,
    bmp_cache: BitmapCache,
    image_cache: ImageCache,
    errors: Arc<Mutex<Vec<(PathHash, Arc<Error>)>>>,
}

impl ImageManager {
    pub fn new(
        worker_threads: usize,
        bmp_target_size: usize,
        image_target_size: usize,
    ) -> anyhow::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(worker_threads)
            .build()?;
        Ok(Self {
            runtime,
            bmp_cache: Arc::new(Mutex::new(Cache::new(bmp_target_size))),
            image_cache: Arc::new(Mutex::new(Cache::new(image_target_size))),
            errors: Arc::new(Mutex::new(vec![])),
        })
    }

    pub fn clear(&self) {
        self.runtime.block_on(async {
            self.bmp_cache.lock().await.clear();
            self.image_cache.lock().await.clear();
        });
    }

    pub fn bmp_cache_size(&self) -> usize {
        self.runtime.block_on(async {
            let cache = self.bmp_cache.lock().await;
            cache.size()
        })
    }

    pub fn image_cache_size(&self) -> usize {
        self.runtime.block_on(async {
            let cache = self.image_cache.lock().await;
            cache.size()
        })
    }

    pub fn load(
        &self,
        dc: ComPtr<ID2D1DeviceContext>,
        path: &Path,
        complete: impl FnOnce(PathBuf) + Send + 'static,
    ) {
        self.runtime.block_on(async {
            let path_hash = to_path_hash(path);
            let path = path.to_path_buf();
            let bmp_cache = self.bmp_cache.clone();
            let image_cache = self.image_cache.clone();
            let errors = self.errors.clone();
            self.runtime.spawn(async move {
                let img = load_image(dc, path.clone(), path_hash, bmp_cache, image_cache).await;
                if let Err(e) = img {
                    let mut errors = errors.lock().await;
                    let e = Arc::new(e);
                    if let Some(elem) = errors.iter_mut().find(|(p, _)| *p == path_hash) {
                        elem.1 = e;
                    } else {
                        errors.push((path_hash, e));
                    }
                }
                complete(path);
            });
        });
    }

    pub fn get(&self, path: &Path) -> Result<Option<ComPtr<ID2D1Bitmap1>>, Arc<Error>> {
        self.runtime.block_on(async {
            let path_hash = to_path_hash(path);
            let bmp_cache = self.bmp_cache.lock().await;
            let errors = self.errors.lock().await;
            if let Some(e) = errors.iter().find(|(p, _)| *p == path_hash).map(|(_, e)| e) {
                Err(e.clone())
            } else {
                Ok(bmp_cache.find(path_hash).cloned())
            }
        })
    }
}
