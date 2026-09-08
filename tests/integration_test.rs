use proto_pdk_test_utils::*;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

fn ensure_wasm_built() {
    static BUILD_RESULT: OnceLock<Result<(), String>> = OnceLock::new();

    let result = BUILD_RESULT.get_or_init(|| {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

        let output = Command::new("cargo")
            .args([
                "build",
                "--target",
                "wasm32-wasip1",
                "--release",
                "--features",
                "wasm",
                "--quiet",
            ])
            .current_dir(manifest_dir)
            .output()
            .map_err(|error| format!("Failed to run cargo build for wasm target: {error}"))?;

        if !output.status.success() {
            return Err(format!(
                "Failed to build wasm plugin before tests (status: {}). stderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    });

    if let Err(error) = result {
        panic!("{error}");
    }
}

mod terraform_tool {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn resolves_latest_alias() {
        ensure_wasm_built();
        let sandbox = create_empty_proto_sandbox();
        let plugin = sandbox.create_plugin("terraform-test").await;
        let mut spec = ToolSpec::parse("latest").unwrap();

        flow::resolve::Resolver::new(&plugin.tool)
            .resolve_version(&mut spec, false)
            .await
            .unwrap();

        assert_ne!(spec.get_resolved_version(), "latest");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn resolve_version_or_alias() {
        ensure_wasm_built();
        let sandbox = create_empty_proto_sandbox();
        let plugin = sandbox.create_plugin("terraform-test").await;

        for (input, expected) in [("1.5.0", "1.5.0"), ("1.9.0", "1.9.0")] {
            let mut spec = ToolSpec::parse(input).unwrap();

            flow::resolve::Resolver::new(&plugin.tool)
                .resolve_version(&mut spec, false)
                .await
                .unwrap();

            assert_eq!(spec.get_resolved_version(), expected);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    #[should_panic(expected = "FailedVersionResolve")]
    async fn errors_invalid_alias() {
        ensure_wasm_built();
        let sandbox = create_empty_proto_sandbox();
        let plugin = sandbox.create_plugin("terraform-test").await;

        flow::resolve::Resolver::new(&plugin.tool)
            .resolve_version(&mut ToolSpec::parse("unknown").unwrap(), false)
            .await
            .unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    #[should_panic(expected = "FailedVersionResolve")]
    async fn errors_invalid_version() {
        ensure_wasm_built();
        let sandbox = create_empty_proto_sandbox();
        let plugin = sandbox.create_plugin("terraform-test").await;

        flow::resolve::Resolver::new(&plugin.tool)
            .resolve_version(&mut ToolSpec::parse("99.99.99").unwrap(), false)
            .await
            .unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn registers_tool_metadata() {
        ensure_wasm_built();
        let sandbox = create_empty_proto_sandbox();
        let plugin = sandbox.create_plugin("terraform-test").await;

        let output = plugin
            .register_tool(RegisterToolInput {
                id: Id::new("terraform-test").unwrap(),
            })
            .await;

        assert_eq!(output.name, "Terraform");
        assert_eq!(output.type_of, PluginType::CommandLine);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn loads_versions_from_hashicorp() {
        ensure_wasm_built();
        let sandbox = create_empty_proto_sandbox();
        let plugin = sandbox.create_plugin("terraform-test").await;

        let output = plugin.load_versions(LoadVersionsInput::default()).await;

        assert!(!output.versions.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sets_latest_alias() {
        ensure_wasm_built();
        let sandbox = create_empty_proto_sandbox();
        let plugin = sandbox.create_plugin("terraform-test").await;

        let output = plugin.load_versions(LoadVersionsInput::default()).await;

        assert!(output.latest.is_some());
        assert!(output.aliases.contains_key("latest"));
        assert_eq!(output.aliases.get("latest"), output.latest.as_ref());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn detects_version_files() {
        ensure_wasm_built();
        let sandbox = create_empty_proto_sandbox();
        let plugin = sandbox.create_plugin("terraform-test").await;

        let output = plugin
            .detect_version_files(DetectVersionInput::default())
            .await;

        assert_eq!(output.files, vec![".terraform-version".to_string()]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn parses_terraform_version_file() {
        ensure_wasm_built();
        let sandbox = create_empty_proto_sandbox();
        let plugin = sandbox.create_plugin("terraform-test").await;

        let output = plugin
            .parse_version_file(ParseVersionFileInput {
                content: "1.5.7\n".into(),
                file: ".terraform-version".into(),
                ..Default::default()
            })
            .await;

        assert_eq!(
            output.version.unwrap(),
            UnresolvedVersionSpec::parse("1.5.7").unwrap()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn returns_none_for_empty_version_file() {
        ensure_wasm_built();
        let sandbox = create_empty_proto_sandbox();
        let plugin = sandbox.create_plugin("terraform-test").await;

        let output = plugin
            .parse_version_file(ParseVersionFileInput {
                content: "".into(),
                file: ".terraform-version".into(),
                ..Default::default()
            })
            .await;

        assert_eq!(output.version, None);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn supports_linux_amd64() {
        ensure_wasm_built();
        let sandbox = create_empty_proto_sandbox();
        let plugin = sandbox
            .create_plugin_with_config("terraform-test", |config| {
                config.host(HostOS::Linux, HostArch::X64);
            })
            .await;

        assert_eq!(
            plugin
                .download_prebuilt(DownloadPrebuiltInput {
                    context: PluginContext {
                        version: VersionSpec::parse("1.5.7").unwrap(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .await,
            DownloadPrebuiltOutput {
                download_url:
                    "https://releases.hashicorp.com/terraform/1.5.7/terraform_1.5.7_linux_amd64.zip"
                        .into(),
                download_name: Some("terraform_1.5.7_linux_amd64.zip".into()),
                checksum_url: Some(
                    "https://releases.hashicorp.com/terraform/1.5.7/terraform_1.5.7_SHA256SUMS"
                        .into()
                ),
                checksum_public_key: Some(
                    "https://www.hashicorp.com/.well-known/pgp-key.txt".into()
                ),
                ..Default::default()
            }
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn supports_linux_arm64() {
        ensure_wasm_built();
        let sandbox = create_empty_proto_sandbox();
        let plugin = sandbox
            .create_plugin_with_config("terraform-test", |config| {
                config.host(HostOS::Linux, HostArch::Arm64);
            })
            .await;

        assert_eq!(
            plugin
                .download_prebuilt(DownloadPrebuiltInput {
                    context: PluginContext {
                        version: VersionSpec::parse("1.5.7").unwrap(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .await,
            DownloadPrebuiltOutput {
                download_url:
                    "https://releases.hashicorp.com/terraform/1.5.7/terraform_1.5.7_linux_arm64.zip"
                        .into(),
                download_name: Some("terraform_1.5.7_linux_arm64.zip".into()),
                checksum_url: Some(
                    "https://releases.hashicorp.com/terraform/1.5.7/terraform_1.5.7_SHA256SUMS"
                        .into()
                ),
                checksum_public_key: Some(
                    "https://www.hashicorp.com/.well-known/pgp-key.txt".into()
                ),
                ..Default::default()
            }
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn supports_macos_arm64() {
        ensure_wasm_built();
        let sandbox = create_empty_proto_sandbox();
        let plugin = sandbox
            .create_plugin_with_config("terraform-test", |config| {
                config.host(HostOS::MacOS, HostArch::Arm64);
            })
            .await;

        assert_eq!(
            plugin
                .download_prebuilt(DownloadPrebuiltInput {
                    context: PluginContext {
                        version: VersionSpec::parse("1.5.7").unwrap(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .await,
            DownloadPrebuiltOutput {
                download_url: "https://releases.hashicorp.com/terraform/1.5.7/terraform_1.5.7_darwin_arm64.zip".into(),
                download_name: Some("terraform_1.5.7_darwin_arm64.zip".into()),
                checksum_url: Some("https://releases.hashicorp.com/terraform/1.5.7/terraform_1.5.7_SHA256SUMS".into()),
                checksum_public_key: Some("https://www.hashicorp.com/.well-known/pgp-key.txt".into()),
                ..Default::default()
            }
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn supports_macos_x64() {
        ensure_wasm_built();
        let sandbox = create_empty_proto_sandbox();
        let plugin = sandbox
            .create_plugin_with_config("terraform-test", |config| {
                config.host(HostOS::MacOS, HostArch::X64);
            })
            .await;

        assert_eq!(
            plugin
                .download_prebuilt(DownloadPrebuiltInput {
                    context: PluginContext {
                        version: VersionSpec::parse("1.5.7").unwrap(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .await,
            DownloadPrebuiltOutput {
                download_url: "https://releases.hashicorp.com/terraform/1.5.7/terraform_1.5.7_darwin_amd64.zip".into(),
                download_name: Some("terraform_1.5.7_darwin_amd64.zip".into()),
                checksum_url: Some("https://releases.hashicorp.com/terraform/1.5.7/terraform_1.5.7_SHA256SUMS".into()),
                checksum_public_key: Some("https://www.hashicorp.com/.well-known/pgp-key.txt".into()),
                ..Default::default()
            }
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn supports_windows_x64() {
        ensure_wasm_built();
        let sandbox = create_empty_proto_sandbox();
        let plugin = sandbox
            .create_plugin_with_config("terraform-test", |config| {
                config.host(HostOS::Windows, HostArch::X64);
            })
            .await;

        assert_eq!(
            plugin
                .download_prebuilt(DownloadPrebuiltInput {
                    context: PluginContext {
                        version: VersionSpec::parse("1.5.7").unwrap(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .await,
            DownloadPrebuiltOutput {
                download_url: "https://releases.hashicorp.com/terraform/1.5.7/terraform_1.5.7_windows_amd64.zip".into(),
                download_name: Some("terraform_1.5.7_windows_amd64.zip".into()),
                checksum_url: Some("https://releases.hashicorp.com/terraform/1.5.7/terraform_1.5.7_SHA256SUMS".into()),
                checksum_public_key: Some("https://www.hashicorp.com/.well-known/pgp-key.txt".into()),
                ..Default::default()
            }
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn locates_unix_bin() {
        ensure_wasm_built();
        let sandbox = create_empty_proto_sandbox();
        let plugin = sandbox
            .create_plugin_with_config("terraform-test", |config| {
                config.host(HostOS::Linux, HostArch::X64);
            })
            .await;

        assert_eq!(
            plugin
                .locate_executables(LocateExecutablesInput {
                    context: PluginContext {
                        version: VersionSpec::parse("1.5.7").unwrap(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .await
                .exes
                .get("terraform")
                .unwrap()
                .exe_path,
            Some("terraform".into())
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn locates_windows_bin() {
        ensure_wasm_built();
        let sandbox = create_empty_proto_sandbox();
        let plugin = sandbox
            .create_plugin_with_config("terraform-test", |config| {
                config.host(HostOS::Windows, HostArch::X64);
            })
            .await;

        assert_eq!(
            plugin
                .locate_executables(LocateExecutablesInput {
                    context: PluginContext {
                        version: VersionSpec::parse("1.5.7").unwrap(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .await
                .exes
                .get("terraform")
                .unwrap()
                .exe_path,
            Some("terraform.exe".into())
        );
    }
}
