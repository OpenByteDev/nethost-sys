use std::{
    borrow::Cow,
    env,
    error::Error,
    fs::{create_dir_all, File},
    io::{self, Cursor, Read},
    path::{Path, PathBuf},
    str::FromStr,
};

use build_target::{Arch, Env, Os};
use semver::Version;
use serde::Deserialize;
use zip::ZipArchive;

#[derive(Debug, Deserialize)]
struct ResourceIndex<'a> {
    resources: Vec<Resource<'a>>,
}

#[derive(Debug, Deserialize)]
struct Resource<'a> {
    #[serde(rename = "@id")]
    url: Cow<'a, str>,
    #[serde(rename = "@type")]
    r#type: Cow<'a, str>,
}

#[derive(Debug, Deserialize)]
struct PackageInfoIndex<'a> {
    #[serde(rename = "items")]
    pages: Vec<PackageInfoCatalogPageReference<'a>>,
}

#[derive(Debug, Deserialize)]
struct PackageInfoCatalogPageReference<'a> {
    #[serde(rename = "@id")]
    url: Cow<'a, str>,
    // lower: Cow<'a, str>,
    upper: Cow<'a, str>,
}

#[derive(Debug, Clone)]
enum PackageInfoCatalogPageResponse<'a> {
    Root(PackageInfoCatalogRoot<'a>),
    Page(PackageInfoCatalogPage<'a>),
}

impl<'de, 'a> serde::Deserialize<'de> for PackageInfoCatalogPageResponse<'a> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        let types = match value.get("@type") {
            Some(serde_json::Value::Array(types)) => types
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<Vec<_>>(),
            Some(serde_json::Value::String(r#type)) => vec![r#type.as_str()],
            f => unreachable!("encountered invalid @type field {f:?}"),
        };

        Ok(if types.contains(&"catalog:CatalogPage") {
            PackageInfoCatalogPage::deserialize(value)
                .map(Self::Page)
                .unwrap()
        } else if types.contains(&"catalog:CatalogRoot") {
            PackageInfoCatalogRoot::deserialize(value)
                .map(Self::Root)
                .unwrap()
        } else {
            unreachable!()
        })
    }
}

#[derive(Debug, Deserialize, Clone)]
struct PackageInfoCatalogRoot<'a> {
    items: Vec<PackageInfoCatalogPage<'a>>,
}

#[derive(Debug, Deserialize, Clone)]
struct PackageInfoCatalogPage<'a> {
    items: Vec<PackageInfoCatalogPageEntry<'a>>,
}

#[derive(Debug, Deserialize, Clone)]
struct PackageInfoCatalogPageEntry<'a> {
    #[serde(rename = "catalogEntry")]
    inner: PackageInfoCatalogEntry<'a>,
}

#[derive(Debug, Deserialize, Clone)]
struct PackageInfoCatalogEntry<'a> {
    // #[serde(rename = "@id")]
    // url: Cow<'a, str>,
    listed: bool,
    #[serde(rename = "packageContent")]
    content: Cow<'a, str>,
    version: Cow<'a, str>,
}

pub fn download_nethost_from_nuget() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let os = Os::target().unwrap();
    let arch = Arch::target().unwrap();
    let env = Env::target().unwrap();

    #[rustfmt::skip]
    let target = match (&os, arch, env) {
        (Os::Windows, Arch::X86,     Env::MSVC) => "win-x86",
        (Os::Windows, Arch::X86_64,  Env::MSVC) => "win-x64",
        (Os::Windows, Arch::ARM,     _) => "win-arm",
        (Os::Windows, Arch::AARCH64, _) => "win-arm64",
        (Os::Linux,   Arch::X86_64,  Env::Musl) => "linux-musl-x64",
        (Os::Linux,   Arch::ARM,     Env::Musl) => "linux-musl-arm",
        (Os::Linux,   Arch::AARCH64, Env::Musl) => "linux-musl-arm64",
        (Os::Linux,   Arch::X86_64,  _) => "linux-x64",
        (Os::Linux,   Arch::ARM,     _) => "linux-arm",
        (Os::Linux,   Arch::AARCH64, _) => "linux-arm64",
        (Os::MacOs,   Arch::X86_64,  _) => "osx-x64",
        (Os::MacOs,   Arch::AARCH64, _) => "osx-arm64",
        _ => panic!("platform not supported."),
    };

    let runtime_dir = Path::new(&env::var("OUT_DIR")?)
        .join("nethost")
        .join(target);
    if !runtime_dir.exists() || runtime_dir.read_dir()?.next().is_none() {
        create_dir_all(&runtime_dir)?;
        download_nethost(target, &runtime_dir)?;
    }

    cargo_emit::rerun_if_changed!(runtime_dir.to_str().unwrap());
    cargo_emit::rustc_link_search!(runtime_dir.to_str().unwrap());

    Ok(runtime_dir)
}

pub fn download_nethost(target: &str, target_path: &Path) -> Result<(), Box<dyn Error>> {
    let client = reqwest::blocking::Client::new();

    let index = client
        .get("https://api.nuget.org/v3/index.json")
        .send()
        .expect("Failed to query nuget.org index for nethost package. Are you connected to the internet?")
        .json::<ResourceIndex>()
        .expect("Failed to parse json response from nuget.org.");
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
        .expect("Unable to find package page.");

    let package_response = client
        .get(format!("{}", package_info.url))
        .send()
        .expect("Failed to retrieve package page.")
        .json::<PackageInfoCatalogPageResponse>()
        .expect("Failed to parse json page response from nuget.org.");

    let package_pages = match package_response {
        PackageInfoCatalogPageResponse::Page(page) => vec![page],
        PackageInfoCatalogPageResponse::Root(root) => root.items,
    };

    let package_page = package_pages
        .into_iter()
        .flat_map(|page| page.items.into_iter())
        .map(|e| e.inner)
        .filter(|e| e.listed)
        .max_by_key(|e| Version::from_str(e.version.as_ref()).unwrap())
        .unwrap();

    let mut package_content_response = client
        .get(package_page.content.as_ref())
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
