use std::path::Path;

use coreclr_hosting_shared::StatusCode;

#[test]
fn returned_path_exists() {
    let mut buffer = Vec::new();
    let mut buffer_size = 0;
    let result = unsafe { nethost_sys::get_hostfxr_path(buffer.as_mut_ptr(), &mut buffer_size, core::ptr::null()) };
    assert_eq!(result, StatusCode::HostApiBufferTooSmall as i32);

    let result = unsafe { nethost_sys::get_hostfxr_path(buffer.as_mut_ptr(), &mut buffer_size, core::ptr::null()) };
    assert_eq!(result, StatusCode::Success as i32);
    

    #[cfg(windows)]
    let path = String::from_utf16(&buffer).unwrap();
    #[cfg(not(windows))]
    let path = String::from_utf8(&buffer).unwrap();

    assert!(Path::new(&path).exists());
}
