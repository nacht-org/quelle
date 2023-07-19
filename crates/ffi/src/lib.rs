mod error;
mod result;

use std::{
    error::Error,
    ffi::{c_char, CStr},
    path::Path,
};

use quelle_engine::Runner;

#[derive(thiserror::Error, Debug)]
enum CustomError {
    #[error("pointer does not reference a valid engine")]
    WrongEnginePtr,
}

#[no_mangle]
pub extern "C" fn open_engine_with_path(path: *const c_char, engine_out: *mut *mut Runner) -> i32 {
    env_logger::init();
    error::capture_error(|| open_engine_with_path_private(path, engine_out))
}

fn open_engine_with_path_private(
    path: *const c_char,
    engine_out: *mut *mut Runner,
) -> Result<(), Box<dyn Error>> {
    let path = unsafe { CStr::from_ptr(path) };
    let path = path.to_str()?;

    let engine = Runner::new(Path::new(path))?;
    let engine = Box::into_raw(Box::new(engine));
    unsafe { *engine_out = engine }
    Ok(())
}

#[no_mangle]
pub extern "C" fn source_meta(engine: *mut Runner) -> i32 {
    result::capture_memloc(|| unsafe {
        let engine = engine.as_mut().ok_or(CustomError::WrongEnginePtr)?;
        let memloc = engine.meta_memloc()?;
        Ok(memloc)
    })
}

#[no_mangle]
pub extern "C" fn memloc_dealloc(engine: *mut Runner, ptr: i32, len: i32) -> i32 {
    result::capture_error(|| {
        let engine = unsafe { engine.as_mut().ok_or(CustomError::WrongEnginePtr)? };
        engine.dealloc_memory(ptr, len)?;
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn fetch_novel(engine: *mut Runner, url: *mut c_char) -> i32 {
    result::capture_result(|| {
        let url = unsafe { CStr::from_ptr(url) }.to_str()?;
        let engine = unsafe { engine.as_mut().ok_or(CustomError::WrongEnginePtr)? };

        let content = engine.fetch_novel_raw(url)?;
        Ok(content.into_bytes())
    })
}

#[no_mangle]
pub extern "C" fn fetch_chapter_content(engine: *mut Runner, url: *mut c_char) -> i32 {
    result::capture_result(|| {
        let url = unsafe { CStr::from_ptr(url) }.to_str()?;
        let engine = unsafe { engine.as_mut().ok_or(CustomError::WrongEnginePtr)? };

        let content = engine.fetch_chapter_content(url)?;
        Ok(content.into_bytes())
    })
}

#[no_mangle]
pub extern "C" fn popular_supported(engine: *mut Runner) -> i32 {
    error::capture_error_with_return(|| {
        let engine = unsafe { engine.as_mut().ok_or(CustomError::WrongEnginePtr)? };
        Ok(engine.popular_supported() as i32)
    })
}

#[no_mangle]
pub extern "C" fn popular(engine: *mut Runner, page: i32) -> i32 {
    result::capture_result(|| {
        let engine = unsafe { engine.as_mut().ok_or(CustomError::WrongEnginePtr)? };
        let novels = engine.popular(page)?;
        let content = serde_json::to_string(&novels)?;
        Ok(content.into_bytes())
    })
}

#[no_mangle]
pub extern "C" fn text_search_supported(engine: *mut Runner) -> i32 {
    error::capture_error_with_return(|| {
        let engine = unsafe { engine.as_ref().ok_or(CustomError::WrongEnginePtr)? };
        Ok(engine.text_search_supported() as i32)
    })
}

#[no_mangle]
pub extern "C" fn text_search(engine: *mut Runner, query: *mut c_char, page: i32) -> i32 {
    result::capture_result(|| {
        let query = unsafe { CStr::from_ptr(query) }.to_str()?;
        let engine = unsafe { engine.as_mut().ok_or(CustomError::WrongEnginePtr)? };
        let content = engine.text_search_raw(query, page)?;
        Ok(content.into_bytes())
    })
}
