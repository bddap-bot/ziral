use serde_json::json;
use ziral_records::{Batch, Receiver};

fn batch(session: &str, start: usize, inputs: serde_json::Value) -> Batch {
    serde_json::from_value(
        json!({"session":session,"build":"b".repeat(40),"seed":0,"start":start,"inputs":inputs}),
    )
    .unwrap()
}

#[tokio::test]
async fn prefixes_are_private_idempotent_append_only_and_credential_free() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path().join("records");
    let receiver = Receiver::new(directory.clone());
    let id = "a".repeat(36);
    assert!(
        receiver
            .store(batch("../escape", 0, json!([[0, "Refill"]])))
            .await
            .is_err()
    );
    assert!(!directory.exists());
    assert_eq!(
        receiver
            .store(batch(&id, 0, json!([[0, "Refill"]])))
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        receiver
            .store(batch(&id, 0, json!([[0, "Refill"], [1, "Refill"]])))
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        receiver
            .store(batch(&id, 0, json!([[0, "Refill"]])))
            .await
            .unwrap(),
        2
    );
    let path = directory.join(format!("{id}.json"));
    let before = std::fs::read(&path).unwrap();
    let mut changed_build = batch(&id, 2, json!([[2, "Refill"]]));
    changed_build.build = "c".repeat(40);
    assert_eq!(
        receiver.store(changed_build).await.unwrap_err().to_string(),
        "session changed"
    );
    assert!(
        receiver
            .store(batch(&id, 0, json!([[0, "Drag"]])))
            .await
            .is_err()
    );
    assert!(
        receiver
            .store(batch(&id, 3, json!([[3, "Refill"]])))
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let record: serde_json::Value = serde_json::from_slice(&before).unwrap();
    assert_eq!(record.as_object().unwrap().len(), 4);
    assert_eq!(record["inputs"], json!([[0, "Refill"], [1, "Refill"]]));
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(directory).unwrap().permissions().mode() & 0o777,
        0o700
    );
}

#[tokio::test]
async fn every_storage_bound_rejects_without_changing_the_record() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path().join("records");
    let receiver = Receiver::new(directory.clone());
    let id = "a".repeat(36);
    receiver
        .store(batch(&id, 0, json!([[0, "Refill"]])))
        .await
        .unwrap();
    let path = directory.join(format!("{id}.json"));
    let before = std::fs::read(&path).unwrap();
    assert!(
        receiver
            .store(batch(&id, 1, json!(["x".repeat(32 * 1024 * 1024)])))
            .await
            .unwrap_err()
            .to_string()
            .contains("record full")
    );
    let legacy = directory.join("legacy");
    std::fs::create_dir(&legacy).unwrap();
    let filler = legacy.join("old.json");
    std::fs::File::create(&filler)
        .unwrap()
        .set_len(1024 * 1024 * 1024)
        .unwrap();
    assert!(
        receiver
            .store(batch(&id, 1, json!([[1, "Refill"]])))
            .await
            .unwrap_err()
            .to_string()
            .contains("storage full")
    );
    std::fs::remove_file(filler).unwrap();
    for n in 0..1023 {
        std::fs::write(legacy.join(format!("{n}.json")), b"{}").unwrap();
    }
    assert!(
        receiver
            .store(batch(&"c".repeat(36), 0, json!([[0, "Refill"]])))
            .await
            .unwrap_err()
            .to_string()
            .contains("too many sessions")
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[tokio::test]
async fn ban_is_durable_and_does_not_change_other_ids_or_identity() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path().join("records");
    let id = "a".repeat(36);
    let receiver = Receiver::new(directory.clone());
    receiver
        .store(batch(&id, 0, json!([[0, "Refill"]])))
        .await
        .unwrap();
    let path = directory.join(format!("{id}.json"));
    let before = std::fs::read(&path).unwrap();
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_ziral-records"))
        .current_dir(temporary.path())
        .args(["ban", &id, "--directory", "records"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!temporary.path().join("ziral-records.key").exists());
    let receiver = Receiver::new(directory);
    assert_eq!(
        receiver
            .store(batch(&id, 1, json!([[1, "Refill"]])))
            .await
            .unwrap_err()
            .to_string(),
        "session banned"
    );
    assert_eq!(
        receiver
            .store(batch(&"c".repeat(36), 0, json!([[0, "Refill"]])))
            .await
            .unwrap(),
        1
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
}

fn local_address(server: &iroh::Endpoint) -> iroh::EndpointAddr {
    let mut address = server.bound_sockets()[0];
    address.set_ip(if address.is_ipv4() {
        std::net::Ipv4Addr::LOCALHOST.into()
    } else {
        std::net::Ipv6Addr::LOCALHOST.into()
    });
    iroh::EndpointAddr::new(server.id()).with_ip_addr(address)
}

async fn exchange(server: &iroh::Endpoint, bytes: &[u8]) -> serde_json::Value {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let client = iroh::Endpoint::builder(iroh::endpoint::presets::N0)
            .relay_mode(iroh::RelayMode::Disabled)
            .clear_address_lookup()
            .bind()
            .await
            .unwrap();
        let connection = client
            .connect(local_address(server), ziral_records::DOMAIN)
            .await
            .unwrap();
        let (mut send, mut recv) = connection.open_bi().await.unwrap();
        send.write_all(&(bytes.len() as u32).to_le_bytes())
            .await
            .unwrap();
        if bytes.len() <= 1_000_000 {
            send.write_all(bytes).await.unwrap();
        }
        send.finish().unwrap();
        let mut length = [0; 4];
        recv.read_exact(&mut length).await.unwrap();
        let mut answer = vec![0; u32::from_le_bytes(length) as usize];
        recv.read_exact(&mut answer).await.unwrap();
        let answer = serde_json::from_slice(&answer).unwrap();
        client.close().await;
        answer
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn relay_ack_follows_storage_and_rejections_are_logged_at_info() {
    use std::io::Write;
    #[derive(Clone)]
    struct Log(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
    impl Write for Log {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let log = Log(Default::default());
    let writer = log.clone();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(move || writer.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path().join("records");
    let server = iroh::Endpoint::builder(iroh::endpoint::presets::N0)
        .relay_mode(iroh::RelayMode::Disabled)
        .clear_address_lookup()
        .bind()
        .await
        .unwrap();
    let router = iroh::protocol::Router::builder(server.clone())
        .accept(ziral_records::DOMAIN, Receiver::new(directory.clone()))
        .spawn();
    let idle = iroh::Endpoint::builder(iroh::endpoint::presets::N0)
        .relay_mode(iroh::RelayMode::Disabled)
        .clear_address_lookup()
        .bind()
        .await
        .unwrap();
    let _idle_connection = idle
        .connect(local_address(&server), ziral_records::DOMAIN)
        .await
        .unwrap();
    let id = "a".repeat(36);
    let body =
        json!({"session":id,"build":"b".repeat(40),"seed":0,"start":0,"inputs":[[0,"Refill"]]});
    assert_eq!(
        exchange(&server, body.to_string().as_bytes()).await,
        json!({"next":1})
    );
    let record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join(format!("{id}.json"))).unwrap())
            .unwrap();
    assert_eq!(record["inputs"], body["inputs"]);
    for invalid in [b"null".to_vec(), b"{".to_vec(), vec![b'x'; 1_000_001]] {
        assert!(exchange(&server, &invalid).await.get("error").is_some());
    }
    ziral_records::ban(directory, &id).await.unwrap();
    assert!(
        exchange(&server, body.to_string().as_bytes())
            .await
            .get("error")
            .is_some()
    );
    let text = String::from_utf8(log.0.lock().unwrap().clone()).unwrap();
    for reason in ["invalid batch JSON", "batch too large", "session banned"] {
        assert!(text.contains(reason), "{text}");
    }
    assert_eq!(text.matches("record upload rejected").count(), 4);
    log.0.lock().unwrap().clear();
    idle.close().await;
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let text = String::from_utf8(log.0.lock().unwrap().clone()).unwrap();
            if text.contains("record connection rejected") && text.contains("reason=") {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("idle connection rejection must include its reason at info level");
    router.shutdown().await.unwrap();
}

#[test]
fn identity_is_stable_and_private() {
    let temporary = tempfile::tempdir().unwrap();
    let command = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_ziral-records"))
            .current_dir(temporary.path())
            .args(args)
            .output()
            .unwrap()
    };
    let first = command(&["identity"]);
    let second = command(&["identity"]);
    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(String::from_utf8(first.stdout).unwrap().trim().len(), 64);
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(temporary.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        command(&["--key-file", "./ziral-records.key", "identity"])
            .status
            .success()
    );
    assert_eq!(
        std::fs::metadata(temporary.path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    assert!(
        command(&["--key-file", "nested/private/key", "identity"])
            .status
            .success()
    );
    for directory in ["nested", "nested/private"] {
        assert_eq!(
            std::fs::metadata(temporary.path().join(directory))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
    let key = temporary.path().join("ziral-records.key");
    assert_eq!(
        std::fs::metadata(&key).unwrap().permissions().mode() & 0o777,
        0o600
    );
    std::fs::set_permissions(key, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(!command(&["identity"]).status.success());
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn buffered_write_errors_cannot_replace_or_acknowledge_a_record() {
    if let Ok(directory) = std::env::var("ZIRAL_TEST_WRITE_FAILURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        let id = "a".repeat(36);
        let path = directory.join(format!("{id}.json"));
        let before = json!({"session":id,"build":"b".repeat(40),"seed":0,"inputs":[[0,"Refill"]]})
            .to_string();
        std::fs::write(&path, &before).unwrap();
        let failure = std::env::var("ZIRAL_TEST_IO_FAILURE").unwrap();
        let error = Receiver::new(directory)
            .store(batch(&id, 1, json!([[1, "Refill"]])))
            .await
            .unwrap_err();
        let expected = if failure == "write" {
            "No space left"
        } else {
            "Input/output error"
        };
        assert!(error.to_string().contains(expected), "{error}");
        if failure != "directory_sync" {
            assert_eq!(std::fs::read_to_string(path).unwrap(), before);
        } else {
            let saved: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            assert_eq!(saved["inputs"], json!([[0, "Refill"], [1, "Refill"]]));
        }
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let source = temporary.path().join("fail-write.c");
    let library = temporary.path().join("fail-write.so");
    std::fs::write(
        &source,
        r#"
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
ssize_t write(int fd, const void *buf, size_t count) {
    char link[64], path[4096];
    snprintf(link, sizeof link, "/proc/self/fd/%d", fd);
    ssize_t length = readlink(link, path, sizeof path - 1);
    if (length >= 0) {
        path[length] = 0;
        if (!strcmp(getenv("ZIRAL_TEST_IO_FAILURE"), "write") && length >= 8 && !strcmp(path + length - 8, ".pending")) {
            errno = ENOSPC;
            return -1;
        }
    }
    ssize_t (*next)(int, const void *, size_t) = dlsym(RTLD_NEXT, "write");
    return next(fd, buf, count);
}
int fsync(int fd) {
    char link[64], path[4096];
    snprintf(link, sizeof link, "/proc/self/fd/%d", fd);
    ssize_t length = readlink(link, path, sizeof path - 1);
    if (length >= 0) {
        path[length] = 0;
        const char *failure = getenv("ZIRAL_TEST_IO_FAILURE");
        if ((!strcmp(failure, "file_sync") && length >= 8 && !strcmp(path + length - 8, ".pending")) ||
            (!strcmp(failure, "directory_sync") && !strcmp(path, getenv("ZIRAL_TEST_WRITE_FAILURE_DIR")))) {
            errno = EIO;
            return -1;
        }
    }
    int (*next)(int) = dlsym(RTLD_NEXT, "fsync");
    return next(fd);
}
"#,
    )
    .unwrap();
    assert!(
        std::process::Command::new("cc")
            .args(["-shared", "-fPIC"])
            .arg(&source)
            .arg("-o")
            .arg(&library)
            .arg("-ldl")
            .status()
            .unwrap()
            .success()
    );
    for failure in ["write", "file_sync", "directory_sync"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "buffered_write_errors_cannot_replace_or_acknowledge_a_record",
                "--nocapture",
            ])
            .env("LD_PRELOAD", &library)
            .env("ZIRAL_TEST_IO_FAILURE", failure)
            .env("ZIRAL_TEST_WRITE_FAILURE_DIR", temporary.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[tokio::test]
async fn acknowledged_numeric_prefixes_survive_serialization_and_retries() {
    let temporary = tempfile::tempdir().unwrap();
    let receiver = Receiver::new(temporary.path().join("records"));
    let mut state = 47_u64;
    let values: Vec<_> = (0..4096)
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            json!(f64::from_bits((state >> 12) | 0x3ff0000000000000) * 10000.0)
        })
        .collect();
    let bytes =
        json!({"session":"a".repeat(36),"build":"b".repeat(40),"seed":0,"start":0,"inputs":values})
            .to_string();
    assert_eq!(
        receiver
            .store(serde_json::from_str(&bytes).unwrap())
            .await
            .unwrap(),
        4096
    );
    let saved: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            temporary
                .path()
                .join("records")
                .join(format!("{}.json", "a".repeat(36))),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(saved["inputs"], json!(values));
    assert_eq!(
        receiver
            .store(serde_json::from_str(&bytes).unwrap())
            .await
            .unwrap(),
        4096
    );
}

#[tokio::test]
async fn existing_roots_need_no_ancestor_read_permission_and_archives_stay_in_place() {
    use std::os::unix::fs::PermissionsExt;
    struct Restore(std::path::PathBuf);
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o700));
        }
    }
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path().join("records");
    let archive = directory.join("archive");
    std::fs::create_dir_all(&archive).unwrap();
    let id = "a".repeat(36);
    let path = archive.join(format!("{id}.json"));
    let legacy=json!({"token":"c".repeat(192),"session":id,"build":"b".repeat(40),"seed":0,"inputs":[[0,"Refill"]]}).to_string();
    std::fs::write(&path, &legacy).unwrap();
    let orphan = directory.join("orphan.pending");
    std::fs::write(&orphan, "incomplete").unwrap();
    let _restore = Restore(temporary.path().to_owned());
    std::fs::set_permissions(temporary.path(), std::fs::Permissions::from_mode(0o300)).unwrap();
    let receiver = Receiver::new(directory.clone());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), legacy);
    assert_eq!(
        receiver
            .store(batch(&id, 1, json!([[1, "Refill"]])))
            .await
            .unwrap(),
        2
    );
    assert!(!directory.join(format!("{id}.json")).exists());
    assert!(!orphan.exists());
    let updated: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert!(updated.get("token").is_none());
    assert_eq!(updated["inputs"], json!([[0, "Refill"], [1, "Refill"]]));
}
