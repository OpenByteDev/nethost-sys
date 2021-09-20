#![no_std]
#![allow(non_camel_case_types)]

//! FFI bindings for [nethost](https://github.com/dotnet/runtime/blob/main/docs/design/features/host-components.md#components-of-the-hosting).

/// The char type used in `nethost` and `hostfxr`. Defined as [`u16`] on windows and as [`c_char`](std::os::raw::c_char) otherwise.
#[cfg(all(not(windows), std))]
pub type char_t = std::os::raw::c_char;
/// The char type used in `nethost` and `hostfxr`. Defined as [`u16`] on windows and as [`c_char`](rs_ctypes::c_char) otherwise.
#[cfg(all(not(windows), not(std)))]
pub type char_t = rs_ctypes::c_char;
/// The char type used in `nethost` and `hostfxr`. Defined as [`u16`] on windows and as [`c_char`](std::os::raw::c_char) otherwise.
#[cfg(all(windows, std))]
pub type char_t = u16;
/// The char type used in `nethost` and `hostfxr`. Defined as [`u16`] on windows and as [`c_char`](rs_ctypes::c_char) otherwise.
#[cfg(all(windows, not(std)))]
pub type char_t = u16;

/// Equivalent to `size_t` in C.
pub type size_t = usize; // TODO: use `std::os::raw::c_size_t` instead once stabilized.

// for some reason we need the link attribute here for unix, but the rustc argument in build.rs for windows.
// #[cfg_attr(windows, link(name = "libnethost"))]
#[cfg_attr(unix, link(name = "nethost", kind = "static"))]
#[cfg_attr(
    all(unix, not(target_os = "macos")),
    link(name = "stdc++", kind = "dylib")
)]
#[cfg_attr(target_os = "macos", link(name = "c++", kind = "dylib"))]
extern "system" {
    /// Get the path to the `hostfxr` library.
    ///
    /// # Arguments
    ///  * `buffer`:
    ///     Buffer that will be populated with the hostfxr path, including a null terminator.
    ///  * `buffer_size`:
    ///     * \[in\] Size of buffer in [`char_t`] units.
    ///     * \[out\] Size of buffer used in [`char_t`] units. If the input value is too small
    ///           or `buffer` is [`null`](core::ptr::null()), this is populated with the minimum required size
    ///           in [`char_t`] units for a buffer to hold the hostfxr path
    ///
    /// * `get_hostfxr_parameters`:
    ///     Optional. Parameters that modify the behaviour for locating the hostfxr library.
    ///     If [`null`](core::ptr::null()), hostfxr is located using the enviroment variable or global registration
    ///
    /// # Return value
    ///  * `0` on success, otherwise failure
    ///  * `0x80008098` - `buffer` is too small (`HostApiBufferTooSmall`)
    ///
    /// # Remarks
    /// The full search for the hostfxr library is done on every call. To minimize the need
    /// to call this function multiple times, pass a large buffer (e.g. [`MAX_PATH`](https://docs.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation?tabs=cmd)).
    pub fn get_hostfxr_path(
        buffer: *mut char_t,
        buffer_size: *mut size_t,
        parameters: *const get_hostfxr_parameters,
    ) -> i32;
}

/// Parameters for [`get_hostfxr_path`].
#[repr(C)]
pub struct get_hostfxr_parameters {
    /// Size of the struct.
    /// This is used for versioning.
    pub size: size_t,
    /// Path to the compenent's assembly.
    /// If specified, hostfxr is located as if the `assembly_path` is the apphost
    pub assembly_path: *const char_t,
    /// Path to directory containing the dotnet executable.
    /// If specified, hostfxr is located as if an application is started using
    /// `dotnet app.dll`, which means it will be searched for under the `dotnet_root`
    /// path and the `assembly_path` is ignored.
    pub dotnet_root: *const char_t,
}
