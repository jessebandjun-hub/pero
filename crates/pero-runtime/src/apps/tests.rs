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
        store.uninstall(&name).unwrap_err().to_string(),
        "App is not installed"
    );
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

#[test]
fn uninstall_failure_drops_a_missing_package_from_the_catalog() {
    let workspace = Workspace::new();
    let source = workspace.package(None);
    let name = AppName::try_from("virtual-cell".to_string()).unwrap();
    let root = workspace.0.join("managed/apps");
    let mut store = AppStore::open(root.clone()).unwrap();
    store.install(&source).unwrap();
    fs::remove_dir_all(root.join(name.as_str())).unwrap();
    let UninstallError::RemovalFailed(failure) = store.uninstall(&name).unwrap_err() else {
        panic!("a missing package should be reported");
    };
    assert!(!failure.package_present);
    assert!(!failure.package_intact);
    assert!(!failure.backend_running);
    assert!(failure.retained.is_empty());
    assert!(store.list().is_empty());
    assert!(
        failure
            .to_string()
            .contains("The package is no longer on disk.")
    );
    assert!(failure.to_string().contains("Nothing is still listed."));
    assert!(failure.to_string().contains("The backend is not running."));
}

#[test]
fn uninstall_stops_backend_and_removes_the_app() {
    let workspace = Workspace::new();
    let source = workspace.package(Some("python"));
    let name = AppName::try_from("virtual-cell".to_string()).unwrap();
    let root = workspace.0.join("managed/apps");
    let mut store = AppStore::open(root.clone()).unwrap();
    store.install(&source).unwrap();
    store.set_enabled(&name, true).unwrap();
    let access = store.resolve(&name).unwrap().backend.unwrap();
    let address = access.url.strip_prefix("http://").unwrap().to_string();
    let mut stream = TcpStream::connect(&address).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .unwrap();
    write!(
        stream,
        "GET / HTTP/1.0\r\nX-Pero-App-Token: {}\r\n\r\n",
        access.token
    )
    .unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).unwrap();
    assert!(reply.contains("cell-result"));
    let observed = workspace.0.join("managed/app-data/virtual-cell/observed");
    assert!(observed.is_file());
    assert!(store.uninstall(&name).unwrap().is_empty());
    assert!(TcpStream::connect(&address).is_err());
    assert!(!root.join(name.as_str()).exists());
    assert!(observed.is_file());
    drop(store);
    assert!(AppStore::open(root).unwrap().list().is_empty());
}

#[cfg(unix)]
struct RestoreMode(PathBuf);

#[cfg(unix)]
impl Drop for RestoreMode {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(&self.0) {
            let mut permissions = metadata.permissions();
            permissions.set_mode(0o755);
            let _ = fs::set_permissions(&self.0, permissions);
        }
    }
}

#[cfg(unix)]
#[test]
fn failed_uninstall_reports_the_installation_that_remains() {
    use std::os::unix::fs::PermissionsExt;
    let workspace = Workspace::new();
    let source = workspace.package(Some("python"));
    let name = AppName::try_from("virtual-cell".to_string()).unwrap();
    let root = workspace.0.join("managed/apps");
    let mut store = AppStore::open(root.clone()).unwrap();
    store.install(&source).unwrap();
    store.set_enabled(&name, true).unwrap();
    let access = store.resolve(&name).unwrap().backend.unwrap();
    let address = access.url.strip_prefix("http://").unwrap().to_string();
    assert!(TcpStream::connect(&address).is_ok());
    // The directory itself is not writable, so removal fails before any entry
    // is deleted and the installed package stays intact.
    let directory = root.join(name.as_str());
    let mut permissions = fs::metadata(&directory).unwrap().permissions();
    permissions.set_mode(0o555);
    fs::set_permissions(&directory, permissions).unwrap();
    let _restore = RestoreMode(directory.clone());
    let UninstallError::RemovalFailed(failure) = store.uninstall(&name).unwrap_err() else {
        panic!("uninstall should report the installation that remains");
    };
    assert!(
        failure.cause.contains("Permission denied"),
        "{}",
        failure.cause
    );
    assert!(failure.package_present);
    assert!(failure.package_intact);
    assert!(!failure.backend_running);
    assert!(
        failure
            .retained
            .iter()
            .any(|app| app.name == name && app.enabled)
    );
    let listed: Vec<_> = store
        .list()
        .iter()
        .map(|app| app.name.as_str().to_string())
        .collect();
    let reported: Vec<_> = failure
        .retained
        .iter()
        .map(|app| app.name.as_str().to_string())
        .collect();
    assert_eq!(listed, reported);
    assert_eq!(
        failure.to_string(),
        format!(
            "Could not remove the App package: {}. The package is still installed. The backend is not running. Still listed: virtual-cell.",
            failure.cause
        )
    );
    assert!(directory.join("app.json").is_file());
    assert!(TcpStream::connect(&address).is_err());
    let restarted = store.resolve(&name).unwrap().backend.unwrap();
    assert_ne!(restarted.token, access.token);
    assert!(TcpStream::connect(restarted.url.strip_prefix("http://").unwrap()).is_ok());
}
