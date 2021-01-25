use crate::error::Error;
use com_ptr::*;
use std::path::PathBuf;
use winapi::shared::winerror::*;
use winapi::shared::wtypesbase::*;
use winapi::um::combaseapi::*;
use winapi::um::objbase::*;
use winapi::um::shobjidl::*;
use winapi::um::shobjidl_core::*;
use winapi::um::shtypes::*;

unsafe fn file_open_dialog_impl(extensions: &Vec<String>) -> Result<Option<PathBuf>, Error> {
    let dialog =
        co_create_instance::<IFileOpenDialog>(&CLSID_FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
    let ext_name = "画像ファイル"
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let ext_spec = extensions
        .iter()
        .map(|ext| format!("*.{};", ext))
        .collect::<String>()
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let dlg = COMDLG_FILTERSPEC {
        pszName: ext_name.as_ptr(),
        pszSpec: ext_spec.as_ptr(),
    };
    dialog.SetFileTypes(1, &dlg);
    let ret = dialog.Show(std::ptr::null_mut());
    if ret != S_OK {
        if ret == HRESULT_FROM_WIN32(ERROR_CANCELLED) {
            return Ok(None);
        } else {
            return Err(HResult(ret).into());
        }
    }
    let item = ComPtr::new(|| {
        let mut obj = std::ptr::null_mut();
        let ret = dialog.GetResult(&mut obj);
        hresult(obj, ret)
    })?;
    let path = {
        let mut p = std::ptr::null_mut();
        item.GetDisplayName(SIGDN_FILESYSPATH, &mut p);
        let len = (0..std::isize::MAX)
            .position(|i| *p.offset(i) == 0)
            .unwrap();
        let path = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
        CoTaskMemFree(p as *mut _);
        path
    };
    Ok(Some(path.into()))
}

pub fn file_open_dialog(extensions: &Vec<String>) -> Result<Option<PathBuf>, Error> {
    let exts = extensions.clone();
    let handle = std::thread::spawn(move || unsafe {
        CoInitializeEx(
            std::ptr::null_mut(),
            COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE,
        );
        let path = file_open_dialog_impl(&exts);
        CoUninitialize();
        path
    });
    handle.join().unwrap()
}
