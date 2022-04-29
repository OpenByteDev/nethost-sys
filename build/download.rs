use std::{
    borrow::Cow,
    env,
    error::Error,
    fs::{create_dir_all, File},
    io::{self, Cursor, Read},
    path::{Path, PathBuf},
    str::FromStr,
};

use platforms::{Arch, Env, OS};
use semver::Version;
use serde::{Deserialize, Serialize};
use zip::ZipArchive;

#[derive(Debug, Serialize, Deserialize)]
struct ResourceIndex<'a> {
    resources: Vec<Resource<'a>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Resource<'a> {
    #[serde(rename = "@id")]
    url: Cow<'a, str>,
    #[serde(rename = "@type")]
    r#type: Cow<'a, str>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PackageInfoIndex<'a> {
    #[serde(rename = "items")]
    pages: Vec<PackageInfoCatalogPage<'a>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PackageInfoCatalogPage<'a> {
    #[serde(rename = "@id")]
    url: Cow<'a, str>,
    lower: Cow<'a, str>,
    upper: Cow<'a, str>,
    items: Vec<PackageInfoCatalogPageEntry<'a>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PackageInfoCatalogPageEntry<'a> {
    #[serde(rename = "catalogEntry")]
    inner: PackageInfoCatalogEntry<'a>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PackageInfoCatalogEntry<'a> {
    #[serde(rename = "@id")]
    url: Cow<'a, str>,
    listed: bool,
    #[serde(rename = "packageContent")]
    content: Cow<'a, str>,
    version: Cow<'a, str>,
}

#[cfg_attr(all(feature = "download-nuget-system", feature = "download-nuget-target"), allow(unreachable_code, unused_variables))]
pub fn download_nethost_from_nuget() -> Result<PathBuf, Box<dyn std::error::Error>> {
    #[cfg(all(feature = "download-nuget-system", feature = "download-nuget-target"))]
    panic!("Only one of the 'download-nuget-system' and 'download-nuget-target' features can be enabled at the same time.");
    #[cfg(not(feature = "download-nuget-target"))]
    let platform_triple = current_platform::CURRENT_PLATFORM;
    #[cfg(feature = "download-nuget-target")]
    let platform_triple = build_target::target_triple().unwrap();

    let platform = platforms::Platform::find(&platform_triple).unwrap();

    #[rustfmt::skip]
    let target = match (platform.target_os, platform.target_arch, platform.target_env) {
        (OS::Windows, Arch::X86,     _) => "win-x86",
        (OS::Windows, Arch::X86_64,  _) => "win-x64",
        (OS::Windows, Arch::Arm,     _) => "win-arm",
        (OS::Windows, Arch::AArch64, _) => "win-arm64",
        (OS::Linux,   Arch::X86_64,  Env::Musl) => "linux-musl-x64",
        (OS::Linux,   Arch::Arm,     Env::Musl) => "linux-musl-arm",
        (OS::Linux,   Arch::AArch64, Env::Musl) => "linux-musl-arm64",
        (OS::Linux,   Arch::X86_64,  _) => "linux-x64",
        (OS::Linux,   Arch::Arm,     _) => "linux-arm",
        (OS::Linux,   Arch::AArch64, _) => "linux-arm64",
        (OS::MacOS,   Arch::X86_64,  _) => "osx-x64",
        _ => panic!("platform not supported."),
    };

    let runtime_dir = Path::new(&env::var("OUT_DIR")?)
        .join("nethost")
        .join(target);
    if !runtime_dir.exists() || runtime_dir.read_dir()?.next().is_none() {
        create_dir_all(&runtime_dir)?;
        download_nethost(target, &runtime_dir)?;
    }

    println!("cargo:rerun-if-changed={}", runtime_dir.to_str().unwrap());
    println!("cargo:rustc-link-search={}", runtime_dir.to_str().unwrap());

    Ok(runtime_dir)
}

pub fn download_nethost(target: &str, target_path: &Path) -> Result<(), Box<dyn Error>> {
    let client = reqwest::blocking::Client::new();

    let index = client
        .get("https://api.nuget.org/v3/index.json")
        .send()
        .expect("Failed to query nuget.org index for nethost package. Are you connected to the internet?")
        .json::<ResourceIndex>()
        .expect("Failed to parse json response from nuget.org2.");
    let registrations_base_url = index
        .resources
        .into_iter()
        .find(|res| res.r#type == "RegistrationsBaseUrl")
        .expect("Unable to find nuget.org query endpoint.")
        .url;

    let package_info = client
        .get(format!(
            "{}runtime.{}.microsoft.netcore.dotnetapphost/index.json",
            registrations_base_url, target
        ))
        .send()
        .expect("Failed to find package on nuget.org.")
        .json::<PackageInfoIndex>()
        .expect("Failed to parse json response from nuget.org.")
        .pages
        .into_iter()
        .max_by_key(|page| Version::from_str(page.upper.as_ref()).unwrap())
        .expect("Unable to find package page.")
        .items
        .into_iter()
        .map(|e| e.inner)
        .filter(|e| e.listed)
        .max_by_key(|e| Version::from_str(e.version.as_ref()).unwrap())
        .unwrap();

    let mut package_content_response = client
        .get(package_info.content.as_ref())
        .send()
        .expect("Failed to download nethost nuget package.");

    let mut buf: Vec<u8> = Vec::new();
    package_content_response.read_to_end(&mut buf)?;

    let reader = Cursor::new(buf);
    let mut archive = ZipArchive::new(reader)?;

    let runtime_dir_path = format!("runtimes/{}/native", target);

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;

        let out_path = match file.enclosed_name() {
            Some(path) => path,
            None => continue,
        };

        if !out_path.starts_with(&runtime_dir_path) {
            continue;
        }

        if let Some(ext) = out_path.extension() {
            if !(ext == "a" || ext == "lib" || ext == "pdb") {
                continue;
            }
        } else {
            continue;
        }

        if let Some(name) = out_path.file_stem() {
            if !name.to_string_lossy().contains("nethost") {
                continue;
            }
        } else {
            continue;
        }

        let mut out_file = File::create(target_path.join(out_path.components().last().unwrap()))?;
        io::copy(&mut file, &mut out_file)?;
    }

    Ok(())
}
