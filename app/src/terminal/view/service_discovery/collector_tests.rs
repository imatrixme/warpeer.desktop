use tempfile::tempdir;

use super::*;

#[test]
fn classifies_common_local_service_runtimes() {
    assert_eq!(classify_runtime("node", "node server.js"), Some("Node.js"));
    assert_eq!(classify_runtime("node", "node vite"), Some("Vite"));
    assert_eq!(classify_runtime("bun", "bun run dev"), Some("Bun"));
    assert_eq!(
        classify_runtime("workerd", "workerd serve"),
        Some("Workers")
    );
    assert_eq!(classify_runtime("cargo", "cargo run"), None);
}

#[test]
fn parses_and_deduplicates_lsof_listening_ports() {
    let ports =
        parse_lsof_ports(b"p42\ncnode\nn*:3000\nn127.0.0.1:3000\np77\ncworkerd\nn[::1]:8787\n");

    assert_eq!(
        ports,
        vec![
            ServicePort {
                port: 3000,
                pid: 42,
                environment: "Node.js".to_string(),
            },
            ServicePort {
                port: 8787,
                pid: 77,
                environment: "Workers".to_string(),
            },
        ]
    );
}

#[test]
fn reads_package_scripts_in_server_first_order() {
    let directory = tempdir().unwrap();
    std::fs::write(
        directory.path().join("package.json"),
        r#"{"name":"console","packageManager":"pnpm@9.0.0","scripts":{"test":"vitest","start":"node server.js","dev":"vite"}}"#,
    )
    .unwrap();

    let package = read_package_project(directory.path()).unwrap();

    assert_eq!(package.name.as_deref(), Some("console"));
    assert_eq!(package.manager, PackageManager::Pnpm);
    assert_eq!(
        package
            .scripts
            .iter()
            .map(|script| script.name.as_str())
            .collect::<Vec<_>>(),
        vec!["dev", "start", "test"]
    );
}

#[test]
fn package_manager_builds_expected_script_arguments() {
    assert_eq!(PackageManager::Npm.script_args("dev"), ["run", "dev"]);
    assert_eq!(PackageManager::Yarn.script_args("dev"), ["dev"]);
}
