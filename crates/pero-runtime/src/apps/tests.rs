use super::*;
use std::io::{Read, Write};
use std::net::TcpStream;

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("pero-apps-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn package(&self, backend: Option<&str>) -> PathBuf {
        let source = self.0.join("source");
        fs::create_dir_all(source.join("dist")).unwrap();
        fs::write(source.join("dist/index.html"), "<h1>Cell atlas</h1>").unwrap();
        let mut manifest = serde_json::json!({"schemaVersion":1,"name":"virtual-cell","version":"0.1.0",
            "displayName":"Virtual Cell","contributes":{"panels":[{"id":"main","title":"Virtual Cell","entry":"dist/index.html"}]}});
        if let Some(kind) = backend {
            fs::create_dir(source.join("backend")).unwrap();
            fs::write(
                source.join("backend/server.py"),
                r#"
import json, os, signal
from http.server import BaseHTTPRequestHandler
from socketserver import TCPServer
from pathlib import Path
class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_): pass
    def do_GET(self):
        if self.headers.get('X-Pero-App-Token') != os.environ['PERO_APP_TOKEN']:
            self.send_response(403); self.end_headers(); return
        Path(os.environ['PERO_APP_DATA'], 'observed').write_text('request')
        self.send_response(200); self.end_headers(); self.wfile.write(b'cell-result')
server = TCPServer(('127.0.0.1', 0), Handler)
print(json.dumps({'port':server.server_address[1]}), flush=True)
server.serve_forever()
"#,
            )
            .unwrap();
            let entry = if kind == "node" {
                fs::write(source.join("backend/server.mjs"), r#"
import {createServer} from 'node:http';
import {writeFileSync} from 'node:fs';
import {join} from 'node:path';
const server = createServer((req, res) => {
  if (req.headers['x-pero-app-token'] !== process.env.PERO_APP_TOKEN) { res.writeHead(403); res.end(); return; }
  writeFileSync(join(process.env.PERO_APP_DATA, 'observed'), 'request');
  res.end('cell-result');
});
server.listen(0, '127.0.0.1', () => console.log(JSON.stringify({port:server.address().port})));
"#).unwrap();
                "backend/server.mjs"
            } else {
                "backend/server.py"
            };
            manifest["backend"] = serde_json::json!({"type":kind,"entry":entry});
        }
        fs::write(
            source.join("app.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        source
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn installation_and_retained_data_lifecycle() {
    let workspace = Workspace::new();
    let source = workspace.package(None);
    let root = workspace.0.join("managed/apps");
    let name = AppName::try_from("virtual-cell".to_string()).unwrap();
    let mut store = AppStore::open(root.clone()).unwrap();
    assert!(store.list().is_empty());
    assert!(!store.install(&source).unwrap()[0].enabled);
    assert!(store.resolve(&name).is_err());
    assert!(store.install(&source).is_err());
    store.set_enabled(&name, true).unwrap();
    assert!(
        store
            .resolve(&name)
            .unwrap()
            .root
            .join("dist/index.html")
            .exists()
    );
    let data = workspace.0.join("managed/app-data/virtual-cell");
    fs::create_dir_all(&data).unwrap();
    fs::write(data.join("draft.json"), "user draft").unwrap();
    drop(store);
    let mut store = AppStore::open(root).unwrap();
    assert!(store.list()[0].enabled);
    store.set_enabled(&name, false).unwrap();
    assert!(store.resolve(&name).is_err());
    store.uninstall(&name).unwrap();
    assert!(store.list().is_empty());
    assert_eq!(
        fs::read_to_string(data.join("draft.json")).unwrap(),
        "user draft"
    );
    store.install(&source).unwrap();
    assert!(!store.list()[0].enabled);
}

#[test]
fn builtin_uninstall_is_not_reversed_on_restart() {
    let workspace = Workspace::new();
    let source = workspace.package(None);
    let catalog = workspace.0.join("builtin");
    fs::create_dir(&catalog).unwrap();
    fs::rename(&source, catalog.join("virtual-cell")).unwrap();
    let root = workspace.0.join("managed/apps");
    let mut store = AppStore::open(root.clone()).unwrap();
    store.seed_builtins(&catalog).unwrap();
    assert!(store.list()[0].enabled);
    store
        .uninstall(&AppName::try_from("virtual-cell".to_string()).unwrap())
        .unwrap();
    drop(store);
    let mut store = AppStore::open(root).unwrap();
    store.seed_builtins(&catalog).unwrap();
    assert!(store.list().is_empty());
}

#[test]
fn rejected_package_does_not_publish_partial_install() {
    let workspace = Workspace::new();
    let source = workspace.package(None);
    fs::remove_file(source.join("dist/index.html")).unwrap();
    let root = workspace.0.join("managed/apps");
    let mut store = AppStore::open(root.clone()).unwrap();
    assert!(store.install(&source).is_err());
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    #[cfg(unix)]
    {
        fs::write(source.join("dist/index.html"), "hello").unwrap();
        std::os::unix::fs::symlink("/etc/passwd", source.join("leak")).unwrap();
        assert!(store.install(&source).is_err());
        assert_eq!(fs::read_dir(root).unwrap().count(), 0);
    }
}

#[test]
fn backend_lifecycle() {
    for kind in ["python", "node"] {
        let workspace = Workspace::new();
        let source = workspace.package(Some(kind));
        let name = AppName::try_from("virtual-cell".to_string()).unwrap();
        let mut store = AppStore::open(workspace.0.join("managed/apps")).unwrap();
        store.install(&source).unwrap();
        store.set_enabled(&name, true).unwrap();
        let access = store.resolve(&name).unwrap().backend.unwrap();
        let address = access.url.strip_prefix("http://").unwrap();
        let request = |token: &str| {
            let mut stream = TcpStream::connect(address).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            write!(
                stream,
                "GET / HTTP/1.0\r\nX-Pero-App-Token: {token}\r\n\r\n"
            )
            .unwrap();
            let mut reply = String::new();
            stream.read_to_string(&mut reply).unwrap();
            reply
        };
        assert!(request("wrong").contains("403"));
        assert!(request(&access.token).contains("cell-result"));
        assert!(
            workspace
                .0
                .join("managed/app-data/virtual-cell/observed")
                .is_file()
        );
        store.set_enabled(&name, false).unwrap();
        assert!(TcpStream::connect(address).is_err());
        store.set_enabled(&name, true).unwrap();
        assert_ne!(
            store.resolve(&name).unwrap().backend.unwrap().token,
            access.token
        );
        drop(store);
    }
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    let mut total = (0usize, 0u64);
    super::copy_package(from, to, &mut total).unwrap();
}

fn rewrite(dir: &Path, version: &str, body: &str, name: &str) {
    let path = dir.join("app.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["version"] = version.into();
    manifest["name"] = name.into();
    fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    fs::write(dir.join("dist/index.html"), body).unwrap();
}

#[test]
fn update_replaces_only_the_specified_version() {
    let workspace = Workspace::new();
    let source = workspace.package(Some("python"));
    fs::write(source.join("dist/index.html"), "running-0.1.0").unwrap();
    let root = workspace.0.join("managed/apps");
    let name = AppName::try_from("virtual-cell".to_string()).unwrap();
    let mut store = AppStore::open(root.clone()).unwrap();
    store.install(&source).unwrap();
    store.set_enabled(&name, true).unwrap();
    let data = workspace.0.join("managed/app-data/virtual-cell");
    fs::create_dir_all(&data).unwrap();
    fs::write(data.join("draft.json"), "user draft").unwrap();
    let access = store.resolve(&name).unwrap();
    let token = access.backend.as_ref().unwrap().token.clone();
    let address = access
        .backend
        .as_ref()
        .unwrap()
        .url
        .strip_prefix("http://")
        .unwrap()
        .to_string();
    let request = |token: &str| {
        let mut stream = TcpStream::connect(&address).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        write!(
            stream,
            "GET / HTTP/1.0\r\nX-Pero-App-Token: {token}\r\n\r\n"
        )
        .unwrap();
        let mut reply = String::new();
        stream.read_to_string(&mut reply).unwrap();
        reply
    };
    assert!(request(&token).contains("cell-result"));

    let missing = AppName::try_from("missing-app".to_string()).unwrap();
    assert!(store
        .update(&missing, "0.2.0", &source)
        .unwrap_err()
        .contains("missing-app is not installed"));
    let invalid = store.update(&name, "", &source).unwrap_err();
    assert!(invalid.contains("invalid version"), "{invalid}");
    assert!(
        invalid.contains("Kept installed version 0.1.0"),
        "{invalid}"
    );

    let broken = workspace.0.join("broken");
    copy_tree(&source, &broken);
    rewrite(&broken, "0.2.0", "running-0.2.0", "virtual-cell");
    fs::remove_file(broken.join("dist/index.html")).unwrap();
    let missing_entry = store.update(&name, "0.2.0", &broken).unwrap_err();
    assert!(missing_entry.contains("dist/index.html"), "{missing_entry}");
    assert!(
        missing_entry.contains("Kept installed version 0.1.0"),
        "{missing_entry}"
    );

    let wrong_version = workspace.0.join("wrong-version");
    copy_tree(&source, &wrong_version);
    rewrite(&wrong_version, "0.9.0", "running-0.9.0", "virtual-cell");
    let mismatch = store.update(&name, "0.2.0", &wrong_version).unwrap_err();
    assert!(mismatch.contains("package version is 0.9.0"), "{mismatch}");
    assert!(mismatch.contains("to 0.2.0"), "{mismatch}");
    assert!(
        mismatch.contains("Kept installed version 0.1.0"),
        "{mismatch}"
    );

    let other = workspace.0.join("other-app");
    copy_tree(&source, &other);
    rewrite(&other, "0.2.0", "other", "other-app");
    let identity = store.update(&name, "0.2.0", &other).unwrap_err();
    assert!(
        identity.contains("package identity is other-app"),
        "{identity}"
    );
    assert!(
        identity.contains("Kept installed version 0.1.0"),
        "{identity}"
    );

    #[cfg(unix)]
    {
        let link = workspace.0.join("source-link");
        std::os::unix::fs::symlink(&source, &link).unwrap();
        let linked = store.update(&name, "0.1.0", &link).unwrap_err();
        assert!(linked.contains("symlink"), "{linked}");
        assert!(linked.contains("Kept installed version 0.1.0"), "{linked}");
    }

    assert_eq!(store.resolve(&name).unwrap().backend.unwrap().token, token);
    assert_eq!(store.list()[0].version, "0.1.0");
    assert!(store.list()[0].enabled);
    assert_eq!(
        fs::read_to_string(root.join("virtual-cell/dist/index.html")).unwrap(),
        "running-0.1.0"
    );
    assert_eq!(
        fs::read_to_string(data.join("draft.json")).unwrap(),
        "user draft"
    );
    assert!(!root.join(".rollback-virtual-cell").exists());

    let next = workspace.0.join("v2");
    copy_tree(&source, &next);
    rewrite(&next, "0.2.0", "running-0.2.0", "virtual-cell");
    let listed = store.update(&name, "0.2.0", &next).unwrap();
    assert_eq!(listed[0].version, "0.2.0");
    assert!(listed[0].enabled);
    assert!(TcpStream::connect(&address).is_err());
    let updated = store.resolve(&name).unwrap();
    assert_ne!(updated.backend.unwrap().token, token);
    assert_eq!(
        fs::read_to_string(updated.root.join(&updated.entry)).unwrap(),
        "running-0.2.0"
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("virtual-cell/app.json")).unwrap()).unwrap();
    assert_eq!(manifest["version"], "0.2.0");
    assert_eq!(store.list()[0].version, manifest["version"]);
    assert_eq!(
        fs::read_to_string(data.join("draft.json")).unwrap(),
        "user draft"
    );
    assert!(!root.join(".rollback-virtual-cell").exists());
    drop(store);

    let store = AppStore::open(root).unwrap();
    assert_eq!(store.list()[0].version, "0.2.0");
    assert!(store.list()[0].enabled);
}

#[test]
fn interrupted_update_recovery() {
    let workspace = Workspace::new();
    let source = workspace.package(None);
    let root = workspace.0.join("managed/apps");
    let name = AppName::try_from("virtual-cell".to_string()).unwrap();
    let mut store = AppStore::open(root.clone()).unwrap();
    store.install(&source).unwrap();
    store.set_enabled(&name, true).unwrap();
    drop(store);

    let live = root.join("virtual-cell");
    let rollback = root.join(".rollback-virtual-cell");
    let parked = root.join("parked");
    fs::rename(&live, &parked).unwrap();
    fs::create_dir(&live).unwrap();
    fs::write(live.join("partial.txt"), "incomplete").unwrap();
    fs::rename(&parked, &rollback).unwrap();
    let store = AppStore::open(root.clone()).unwrap();
    assert_eq!(store.list()[0].version, "0.1.0");
    assert!(store.list()[0].enabled);
    assert!(live.join(".installation.json").is_file());
    assert!(!live.join("partial.txt").exists());
    assert!(!rollback.exists());
    drop(store);

    fs::create_dir_all(rollback.join("dist")).unwrap();
    fs::write(rollback.join("dist/index.html"), "stale").unwrap();
    fs::write(
        rollback.join("app.json"),
        r#"{"schemaVersion":1,"name":"virtual-cell","version":"0.0.1","displayName":"Virtual Cell","contributes":{"panels":[{"id":"main","title":"Virtual Cell","entry":"dist/index.html"}]}}"#,
    )
    .unwrap();
    fs::write(rollback.join(".installation.json"), r#"{"enabled":false}"#).unwrap();
    let store = AppStore::open(root).unwrap();
    assert_eq!(store.list()[0].version, "0.1.0");
    assert!(store.list()[0].enabled);
    assert!(!rollback.exists());
}
