use std::fs;
use std::io::{self, Read};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use radishlex_ime_sync::{
    HttpSyncRemoteTransport, SyncRemoteError, SyncRemoteMethod, SyncRemoteRequest,
    SyncRemoteTransport,
};

static TEMP_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(crate) struct GoSyncServer {
    child: Option<Child>,
    root: PathBuf,
    base_url: String,
}

impl GoSyncServer {
    pub(crate) fn try_spawn() -> Option<Self> {
        let port = match reserve_loopback_port() {
            Ok(port) => port,
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                eprintln!("skipping Go sync server integration: loopback bind denied by sandbox");
                return None;
            }
            Err(error) => panic!("reserve loopback port: {error}"),
        };
        let root = temp_root();
        fs::create_dir_all(root.join("objects")).expect("create temp blob dir");
        let server_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../server/sync-server");
        let binary_path = root.join("radishlex-sync-server");
        let build_status = match Command::new("go")
            .args(["build", "-o"])
            .arg(&binary_path)
            .arg("./cmd/radishlex-sync-server")
            .current_dir(&server_dir)
            .status()
        {
            Ok(status) => status,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                eprintln!("skipping Go sync server integration: go command not found");
                let _ = fs::remove_dir_all(&root);
                return None;
            }
            Err(error) => panic!("build Go sync server: {error}"),
        };
        if !build_status.success() {
            let _ = fs::remove_dir_all(&root);
            panic!("build Go sync server failed: {build_status}");
        }

        let mut child = match Command::new(&binary_path)
            .env("RADISHLEX_SYNC_LISTEN", format!("127.0.0.1:{port}"))
            .env(
                "RADISHLEX_SYNC_METADATA_PATH",
                root.join("sync-server.sqlite"),
            )
            .env("RADISHLEX_SYNC_BLOB_DIR", root.join("objects"))
            .env("RADISHLEX_SYNC_MAX_OBJECT_BYTES", "16777216")
            .env("RADISHLEX_SYNC_RECOVERY_READS_PER_HOUR", "12")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => panic!("spawn Go sync server: {error}"),
        };
        let base_url = format!("http://127.0.0.1:{port}");
        wait_until_ready(&base_url, &mut child);
        Some(Self {
            child: Some(child),
            root,
            base_url,
        })
    }

    pub(crate) fn base_url(&self) -> String {
        self.base_url.clone()
    }

    pub(crate) fn data_path(&self, file_name: &str) -> PathBuf {
        self.root.join(file_name)
    }

    pub(crate) fn stop(mut self) -> String {
        let logs = self.stop_child();
        let _ = fs::remove_dir_all(&self.root);
        logs
    }

    fn stop_child(&mut self) -> String {
        let Some(mut child) = self.child.take() else {
            return String::new();
        };
        let _ = child.kill();
        let _ = child.wait();
        read_child_stderr(&mut child)
    }
}

impl Drop for GoSyncServer {
    fn drop(&mut self) {
        let _ = self.stop_child();
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn reserve_loopback_port() -> io::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

fn temp_root() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let sequence = TEMP_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "radishlex-userdb-go-http-{}-{nanos}-{sequence}",
        std::process::id(),
    ))
}

fn wait_until_ready(base_url: &str, child: &mut Child) {
    let transport =
        HttpSyncRemoteTransport::with_timeout(base_url.to_owned(), Duration::from_millis(250))
            .expect("readiness transport");
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().expect("poll Go sync server") {
            let stderr = read_child_stderr(child);
            panic!("Go sync server exited before readiness: {status}\n{stderr}");
        }
        let response = transport.send(SyncRemoteRequest::new(
            SyncRemoteMethod::Get,
            "/api/v1/domains/readiness-domain/state",
            None,
            Vec::new(),
        ));
        match response {
            Ok(_) => return,
            Err(SyncRemoteError::Transport { .. }) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(error) => panic!("Go sync server readiness check failed: {error}"),
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let stderr = read_child_stderr(child);
            panic!("Go sync server did not become ready before timeout\n{stderr}");
        }
    }
}

fn read_child_stderr(child: &mut Child) -> String {
    let Some(mut stderr) = child.stderr.take() else {
        return String::new();
    };
    let mut output = String::new();
    let _ = stderr.read_to_string(&mut output);
    output
}
