use com_ptr::HResult;
use image::error::ImageError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("ファイルが見つかりません")]
    FileNotFound,
    #[error("読み込もうとしたファイルの形式をサポートしていません")]
    Unsupported,
    #[error("HRESULTエラー: (0x{:<08x}){0}", .0.code())]
    HResult(HResult),
    #[error("エラー: {0}")]
    Other(anyhow::Error),
}

impl From<std::io::Error> for Error {
    fn from(src: std::io::Error) -> Error {
        match src.kind() {
            std::io::ErrorKind::NotFound => Error::FileNotFound,
            _ => Error::Other(src.into()),
        }
    }
}

impl From<ImageError> for Error {
    fn from(src: ImageError) -> Error {
        match src {
            ImageError::Unsupported(_) => Error::Unsupported,
            ImageError::IoError(e) => e.into(),
            e @ _ => Error::Other(e.into()),
        }
    }
}

impl From<HResult> for Error {
    fn from(src: HResult) -> Error {
        Error::HResult(src)
    }
}
