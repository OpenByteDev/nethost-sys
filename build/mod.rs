#[cfg(feature = "download-nuget")]
mod download;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "download-nuget")]
    download::download_nethost_from_nuget()?;

    Ok(())
}
