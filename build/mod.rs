#[cfg(feature = "download-nuget")]
mod download;

fn main() {
    #[cfg(feature = "download-nuget")]
    download::download_nethost_from_nuget().unwrap();
}
