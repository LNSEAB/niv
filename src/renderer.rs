use crate::config::*;
use com_ptr::{hresult, ComPtr};
use serde::{Deserialize, Serialize};
use winapi::shared::dxgiformat::*;
use winapi::um::{d2d1::*, d2d1_1::*, dcommon::*, dwrite::*};
use winapi::Interface;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[repr(u32)]
pub enum Interpolation {
    Nearest = D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
    Linear = D2D1_INTERPOLATION_MODE_LINEAR,
    Cubic = D2D1_INTERPOLATION_MODE_CUBIC,
    HighQualityCubic = D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextInfo {
    pub face_name: String,
    pub color: RgbaColor,
    pub size: f32,
}

pub struct Renderer {
    render_target: ComPtr<ID2D1HwndRenderTarget>,
    device_context: ComPtr<ID2D1DeviceContext>,
    text_format: ComPtr<IDWriteTextFormat>,
    text_color: RgbaColor,
}

impl Renderer {
    pub fn new(wnd: &wita::Window, text_info: TextInfo) -> anyhow::Result<Self> {
        let wnd_size = wnd.inner_size();
        let d2d1_factory = ComPtr::new(|| unsafe {
            let mut obj = std::ptr::null_mut();
            let ret = D2D1CreateFactory(
                D2D1_FACTORY_TYPE_MULTI_THREADED,
                &<ID2D1Factory as Interface>::uuidof(),
                std::ptr::null(),
                &mut obj,
            );
            hresult(obj as *mut ID2D1Factory, ret)
        })?;
        let render_target = ComPtr::new(|| unsafe {
            let mut obj = std::ptr::null_mut();
            let ret = d2d1_factory.CreateHwndRenderTarget(
                &D2D1_RENDER_TARGET_PROPERTIES {
                    _type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_R8G8B8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_UNKNOWN,
                    },
                    ..Default::default()
                },
                &D2D1_HWND_RENDER_TARGET_PROPERTIES {
                    hwnd: wnd.raw_handle() as _,
                    pixelSize: winapi::um::d2d1::D2D1_SIZE_U {
                        width: wnd_size.width as u32,
                        height: wnd_size.height as u32,
                    },
                    presentOptions: D2D1_PRESENT_OPTIONS_NONE,
                },
                &mut obj,
            );
            hresult(obj as *mut ID2D1HwndRenderTarget, ret)
        })?;
        let device_context = render_target.query_interface::<ID2D1DeviceContext>()?;
        let dwrite_factory = ComPtr::new(|| unsafe {
            let mut obj = std::ptr::null_mut();
            let ret = DWriteCreateFactory(
                DWRITE_FACTORY_TYPE_SHARED,
                &<IDWriteFactory as Interface>::uuidof(),
                &mut obj,
            );
            hresult(obj as *mut IDWriteFactory, ret)
        })?;
        let text_format = ComPtr::new(|| unsafe {
            let mut obj = std::ptr::null_mut();
            let face = text_info
                .face_name
                .encode_utf16()
                .chain(Some(0))
                .collect::<Vec<_>>();
            let locale = vec![0u16];
            let ret = dwrite_factory.CreateTextFormat(
                face.as_ptr(),
                std::ptr::null_mut(),
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                text_info.size,
                locale.as_ptr(),
                &mut obj,
            );
            hresult(obj as *mut IDWriteTextFormat, ret)
        })?;
        Ok(Self {
            render_target,
            device_context,
            text_format,
            text_color: text_info.color,
        })
    }

    pub fn device_context(&self) -> ComPtr<ID2D1DeviceContext> {
        self.device_context.clone()
    }

    pub fn resize(&mut self, size: wita::PhysicalSize<u32>) {
        unsafe {
            self.render_target.Resize(&winapi::um::d2d1::D2D1_SIZE_U {
                width: size.width,
                height: size.height,
            });
        }
    }

    pub fn set_dpi(&mut self, dpi: f32) {
        unsafe {
            self.render_target.SetDpi(dpi, dpi);
        }
    }

    pub fn render<T: AsRef<str>>(
        &self,
        clear_color: &ClearColor,
        img: Option<ComPtr<ID2D1Bitmap1>>,
        interpolation: Interpolation,
        text: Option<T>,
    ) {
        let dc = &self.device_context;
        unsafe {
            dc.BeginDraw();
            dc.Clear(&D2D1_COLOR_F {
                r: clear_color.0,
                g: clear_color.1,
                b: clear_color.2,
                a: 0.0,
            });
            if let Some(img) = img {
                let img_size = {
                    let size = img.GetSize();
                    winapi::um::d2d1::D2D1_SIZE_F {
                        width: size.width as f32,
                        height: size.height as f32,
                    }
                };
                let viewport = {
                    let size = self.render_target.GetSize();
                    winapi::um::d2d1::D2D1_SIZE_F {
                        width: size.width as f32,
                        height: size.height as f32,
                    }
                };
                let aspect_img = img_size.width / img_size.height;
                let aspect_vp = viewport.width / viewport.height;
                let size = if img_size.width <= viewport.width && img_size.height <= viewport.height
                {
                    img_size.clone()
                } else if aspect_img > aspect_vp {
                    winapi::um::d2d1::D2D1_SIZE_F {
                        width: viewport.width,
                        height: viewport.height * aspect_vp / aspect_img,
                    }
                } else {
                    winapi::um::d2d1::D2D1_SIZE_F {
                        width: viewport.width * aspect_img / aspect_vp,
                        height: viewport.height,
                    }
                };
                dc.DrawBitmap(
                    img.as_ptr() as _,
                    &winapi::um::d2d1::D2D1_RECT_F {
                        left: (viewport.width - size.width) / 2.0,
                        top: (viewport.height - size.height) / 2.0,
                        right: (viewport.width + size.width) / 2.0,
                        bottom: (viewport.height + size.height) / 2.0,
                    },
                    1.0,
                    interpolation as u32,
                    std::ptr::null(),
                    std::ptr::null(),
                );
            }
            if let Some(text) = text {
                let color = ComPtr::new(|| {
                    let mut obj = std::ptr::null_mut();
                    let ret = dc.CreateSolidColorBrush(
                        &D2D1_COLOR_F {
                            r: self.text_color.0,
                            g: self.text_color.1,
                            b: self.text_color.2,
                            a: self.text_color.3,
                        },
                        std::ptr::null_mut(),
                        &mut obj,
                    );
                    hresult(obj, ret)
                });
                if let Ok(color) = color {
                    let text = text
                        .as_ref()
                        .encode_utf16()
                        .chain(Some(0))
                        .collect::<Vec<_>>();
                    let size = self.render_target.GetSize();
                    dc.DrawText(
                        text.as_ptr(),
                        text.len() as u32,
                        self.text_format.as_ptr(),
                        &winapi::um::d2d1::D2D1_RECT_F {
                            left: 0.0,
                            top: 0.0,
                            right: size.width,
                            bottom: size.height,
                        },
                        color.as_ptr() as _,
                        D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                }
            }
            dc.EndDraw(std::ptr::null_mut(), std::ptr::null_mut());
        }
    }
}
