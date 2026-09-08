use extism_pdk::*;
use proto_pdk::*;
use serde::Deserialize;
use std::collections::HashMap;

static NAME: &str = "Terraform";

#[plugin_fn]
pub fn register_tool(Json(_): Json<RegisterToolInput>) -> FnResult<Json<RegisterToolOutput>> {
    Ok(Json(RegisterToolOutput {
        name: NAME.into(),
        type_of: PluginType::CommandLine,
        minimum_proto_version: Some(Version::new(0, 46, 0)),
        plugin_version: Version::parse(env!("CARGO_PKG_VERSION")).ok(),
        ..RegisterToolOutput::default()
    }))
}

#[plugin_fn]
pub fn detect_version_files(_: ()) -> FnResult<Json<DetectVersionOutput>> {
    Ok(Json(DetectVersionOutput {
        files: vec![".terraform-version".into()],
        ignore: vec![],
    }))
}

#[plugin_fn]
pub fn parse_version_file(
    Json(input): Json<ParseVersionFileInput>,
) -> FnResult<Json<ParseVersionFileOutput>> {
    let mut output = ParseVersionFileOutput::default();

    if input.file == ".terraform-version" {
        let content = input.content.trim();
        if !content.is_empty() {
            output.version = Some(UnresolvedVersionSpec::parse(content)?);
        }
    }

    Ok(Json(output))
}

/// Represents the HashiCorp releases index.json structure.
/// The index maps version strings to version metadata containing builds.
#[derive(Deserialize)]
struct HashiCorpIndex {
    versions: HashMap<String, HashiCorpVersion>,
}

#[derive(Deserialize)]
struct HashiCorpVersion {
    #[allow(dead_code)]
    builds: Vec<HashiCorpBuild>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct HashiCorpBuild {
    os: String,
    arch: String,
    url: String,
}

#[plugin_fn]
pub fn load_versions(Json(_): Json<LoadVersionsInput>) -> FnResult<Json<LoadVersionsOutput>> {
    let response: HashiCorpIndex =
        fetch_json("https://releases.hashicorp.com/terraform/index.json")?;

    let versions = response
        .versions
        .keys()
        .filter(|v| {
            // Filter out versions with build metadata (non-standard suffixes)
            !v.contains('+')
        })
        .cloned()
        .collect::<Vec<_>>();

    Ok(Json(LoadVersionsOutput::from(versions)?))
}

#[plugin_fn]
pub fn resolve_version(
    Json(input): Json<ResolveVersionInput>,
) -> FnResult<Json<ResolveVersionOutput>> {
    if input.initial.is_canary() {
        return Err(plugin_err!(PluginError::UnsupportedCanary {
            tool: NAME.into()
        }));
    }

    Ok(Json(ResolveVersionOutput::default()))
}

#[plugin_fn]
pub fn download_prebuilt(
    Json(input): Json<DownloadPrebuiltInput>,
) -> FnResult<Json<DownloadPrebuiltOutput>> {
    let env = get_host_environment()?;

    check_supported_os_and_arch(
        NAME,
        env,
        permutations! [
            HostOS::Linux => [HostArch::X64, HostArch::Arm64, HostArch::X86, HostArch::Arm],
            HostOS::MacOS => [HostArch::X64, HostArch::Arm64],
            HostOS::Windows => [HostArch::X64, HostArch::X86],
        ],
    )?;

    let version = &input.context.version;

    if version.is_canary() {
        return Err(plugin_err!(PluginError::UnsupportedCanary {
            tool: NAME.into()
        }));
    }

    let os = match env.os {
        HostOS::Linux => "linux",
        HostOS::MacOS => "darwin",
        HostOS::Windows => "windows",
        _ => unreachable!(),
    };

    let arch = match env.arch {
        HostArch::X64 => "amd64",
        HostArch::Arm64 => "arm64",
        HostArch::X86 => "386",
        HostArch::Arm => "arm",
        _ => unreachable!(),
    };

    let filename = format!("terraform_{version}_{os}_{arch}.zip");
    let checksum_filename = format!("terraform_{version}_SHA256SUMS");

    Ok(Json(DownloadPrebuiltOutput {
        download_url: format!("https://releases.hashicorp.com/terraform/{version}/{filename}"),
        download_name: Some(filename),
        checksum_url: Some(format!(
            "https://releases.hashicorp.com/terraform/{version}/{checksum_filename}"
        )),
        checksum_public_key: Some("https://www.hashicorp.com/.well-known/pgp-key.txt".into()),
        ..DownloadPrebuiltOutput::default()
    }))
}

#[plugin_fn]
pub fn locate_executables(
    Json(_): Json<LocateExecutablesInput>,
) -> FnResult<Json<LocateExecutablesOutput>> {
    let env = get_host_environment()?;

    Ok(Json(LocateExecutablesOutput {
        exes: HashMap::from_iter([(
            "terraform".into(),
            ExecutableConfig::new_primary(env.os.get_exe_name("terraform")),
        )]),
        ..LocateExecutablesOutput::default()
    }))
}
