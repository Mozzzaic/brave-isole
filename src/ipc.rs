use crate::{
    framing::{read_native, write_native},
    protocol::{BridgeError, Command, DEADLINE_SECS, Envelope, Payload, Reply, Role},
    session::{Session, SessionRegistry, token},
};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::{
        fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub fn validate_peer_uid(actual: u32, expected: u32) -> Result<(), BridgeError> {
    if actual == expected {
        Ok(())
    } else {
        Err(BridgeError::Unauthorized)
    }
}
pub fn validate_origin(origin: &str, id: &str) -> Result<(), BridgeError> {
    if id.len() == 32
        && id.bytes().all(|c| (b'a'..=b'p').contains(&c))
        && origin == format!("chrome-extension://{id}/")
    {
        Ok(())
    } else {
        Err(BridgeError::Unauthorized)
    }
}
pub fn validate_secret(actual: &str, expected: &str) -> Result<(), BridgeError> {
    if actual.len() != 64 || expected.len() != 64 {
        return Err(BridgeError::Unauthorized);
    }
    let difference = actual
        .bytes()
        .zip(expected.bytes())
        .fold(0_u8, |value, (a, b)| value | (a ^ b));
    if difference == 0 {
        Ok(())
    } else {
        Err(BridgeError::Unauthorized)
    }
}
pub fn validate_private_dir(path: &Path) -> Result<(), BridgeError> {
    let meta = fs::symlink_metadata(path).map_err(|_| BridgeError::Unavailable)?;
    if !meta.is_dir() || meta.uid() != nix::unistd::getuid().as_raw() || meta.mode() & 0o077 != 0 {
        return Err(BridgeError::Unauthorized);
    }
    Ok(())
}
fn private_file(path: &Path) -> Result<fs::File, BridgeError> {
    let meta = fs::symlink_metadata(path).map_err(|_| BridgeError::Unavailable)?;
    if !meta.is_file() || meta.uid() != nix::unistd::getuid().as_raw() || meta.mode() & 0o077 != 0 {
        return Err(BridgeError::Unauthorized);
    }
    OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| BridgeError::Unavailable)
}
#[derive(Clone)]
pub struct BridgeConfig {
    pub root: PathBuf,
    pub extension_id: String,
}
impl BridgeConfig {
    pub fn initialize(root: &Path, extension_id: &str) -> Result<Self, BridgeError> {
        validate_origin(&format!("chrome-extension://{extension_id}/"), extension_id)?;
        if !root.exists() {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(root)
                .map_err(|_| BridgeError::Unavailable)?;
        }
        validate_private_dir(root)?;
        for role in [Role::Codex, Role::Claude] {
            let path = root.join(role.label());
            if !path.exists() {
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&path)
                    .map_err(|_| BridgeError::Unavailable)?;
                file.write_all(token()?.as_bytes())
                    .map_err(|_| BridgeError::Unavailable)?;
            }
            let _ = private_file(&path)?;
        }
        let path = root.join("extension");
        if path.exists() {
            let mut old = String::new();
            private_file(&path)?
                .read_to_string(&mut old)
                .map_err(|_| BridgeError::Unavailable)?;
            if old != extension_id {
                return Err(BridgeError::Unauthorized);
            }
        } else {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
                .and_then(|mut file| file.write_all(extension_id.as_bytes()))
                .map_err(|_| BridgeError::Unavailable)?;
        }
        Ok(Self {
            root: root.to_path_buf(),
            extension_id: extension_id.into(),
        })
    }
    pub fn load(root: &Path) -> Result<Self, BridgeError> {
        validate_private_dir(root)?;
        let mut extension_id = String::new();
        private_file(&root.join("extension"))?
            .take(64)
            .read_to_string(&mut extension_id)
            .map_err(|_| BridgeError::Unavailable)?;
        validate_origin(
            &format!("chrome-extension://{extension_id}/"),
            &extension_id,
        )?;
        Ok(Self {
            root: root.into(),
            extension_id,
        })
    }
    pub fn secret(&self, role: Role) -> Result<String, BridgeError> {
        let mut value = String::new();
        private_file(&self.root.join(role.label()))?
            .take(65)
            .read_to_string(&mut value)
            .map_err(|_| BridgeError::Unavailable)?;
        if value.len() != 64 {
            return Err(BridgeError::Unauthorized);
        }
        Ok(value)
    }
    pub fn socket(&self) -> PathBuf {
        self.root.join("s")
    }
    pub fn bind(&self) -> Result<UnixListener, BridgeError> {
        validate_private_dir(&self.root)?;
        let path = self.socket();
        if let Ok(meta) = fs::symlink_metadata(&path) {
            if !meta.file_type().is_socket() || meta.uid() != nix::unistd::getuid().as_raw() {
                return Err(BridgeError::Unauthorized);
            }
            match UnixStream::connect(&path) {
                Ok(_) => return Err(BridgeError::Unavailable),
                Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
                    fs::remove_file(&path).map_err(|_| BridgeError::Unavailable)?
                }
                Err(_) => return Err(BridgeError::Unavailable),
            }
        }
        let listener = UnixListener::bind(&path).map_err(|_| BridgeError::Unavailable)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|_| BridgeError::Unavailable)?;
        Ok(listener)
    }
}
use std::os::unix::fs::DirBuilderExt;
pub fn verify_peer(stream: &UnixStream) -> Result<(), BridgeError> {
    let (uid, _) = nix::unistd::getpeereid(stream).map_err(|_| BridgeError::Unauthorized)?;
    validate_peer_uid(uid.as_raw(), nix::unistd::getuid().as_raw())
}
pub struct NativeBridge {
    stream: Mutex<UnixStream>,
    session: Session,
}
impl NativeBridge {
    pub fn connect(config: &BridgeConfig, role: Role) -> Result<Self, BridgeError> {
        validate_private_dir(&config.root)?;
        let mut stream =
            UnixStream::connect(config.socket()).map_err(|_| BridgeError::Disconnected)?;
        verify_peer(&stream)?;
        stream
            .set_read_timeout(Some(Duration::from_secs(DEADLINE_SECS)))
            .map_err(|_| BridgeError::Unavailable)?;
        stream
            .set_write_timeout(Some(Duration::from_secs(DEADLINE_SECS)))
            .map_err(|_| BridgeError::Unavailable)?;
        write_native(
            &mut stream,
            &Envelope {
                version: 1,
                expires_at_ms: None,
                epoch: "pending".into(),
                id: "auth".into(),
                payload: Payload::Authenticate {
                    role,
                    secret: config.secret(role)?.into(),
                },
            },
        )?;
        let frame = read_until(
            &mut stream,
            Instant::now() + Duration::from_secs(DEADLINE_SECS),
        )?
        .ok_or(BridgeError::Disconnected)?;
        if !matches!(frame.payload, Payload::Hello {}) || frame.id != "auth" {
            return Err(BridgeError::Unauthorized);
        }
        Ok(Self {
            stream: Mutex::new(stream),
            session: SessionRegistry::client_session(role, frame.epoch)?,
        })
    }
    pub fn session(&self) -> &Session {
        &self.session
    }
    pub fn dispatch(&self, session: &Session, command: Command) -> Result<Reply, BridgeError> {
        self.dispatch_until(
            session,
            command,
            Instant::now() + Duration::from_secs(DEADLINE_SECS),
        )
    }
    pub fn dispatch_until(
        &self,
        session: &Session,
        command: Command,
        deadline: Instant,
    ) -> Result<Reply, BridgeError> {
        if session.id() != self.session.id()
            || session.role != self.session.role
            || session.epoch != self.session.epoch
        {
            return Err(BridgeError::Unauthorized);
        }
        let id = token()?;
        let mut stream = self.stream.lock().map_err(|_| BridgeError::Unavailable)?;
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or(BridgeError::Timeout)?;
        stream
            .set_read_timeout(Some(remaining))
            .map_err(|_| BridgeError::Disconnected)?;
        stream
            .set_write_timeout(Some(remaining))
            .map_err(|_| BridgeError::Disconnected)?;
        let expires = (SystemTime::now() + remaining)
            .duration_since(UNIX_EPOCH)
            .map_err(|_| BridgeError::Unavailable)?
            .as_millis() as u64;
        write_native(
            &mut *stream,
            &Envelope {
                version: 1,
                expires_at_ms: Some(expires),
                epoch: session.epoch.clone(),
                id: id.clone(),
                payload: Payload::Command {
                    role: session.role,
                    command,
                },
            },
        )?;
        let frame = read_until(&mut stream, deadline)?.ok_or(BridgeError::Disconnected)?;
        if frame.id != id || frame.epoch != session.epoch {
            return Err(BridgeError::StaleSession);
        }
        match frame.payload {
            Payload::Reply { reply } => Ok(reply),
            _ => Err(BridgeError::InvalidRequest),
        }
    }
}

struct DeadlineReader<'a> {
    stream: &'a mut UnixStream,
    deadline: Instant,
}
impl Read for DeadlineReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let remaining = self
            .deadline
            .checked_duration_since(Instant::now())
            .filter(|value| !value.is_zero())
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::TimedOut))?;
        self.stream.set_read_timeout(Some(remaining))?;
        self.stream.read(buffer)
    }
}
pub fn read_until(
    stream: &mut UnixStream,
    deadline: Instant,
) -> Result<Option<Envelope>, BridgeError> {
    read_native(&mut DeadlineReader { stream, deadline })
}
pub fn read_authenticated(stream: &mut UnixStream) -> Result<Option<Envelope>, BridgeError> {
    stream
        .set_read_timeout(None)
        .map_err(|_| BridgeError::Unavailable)?;
    let mut first = [0_u8; 1];
    match stream.read(&mut first) {
        Ok(0) => return Ok(None),
        Ok(_) => (),
        Err(_) => return Err(BridgeError::Disconnected),
    }
    let rest = DeadlineReader {
        stream,
        deadline: Instant::now() + Duration::from_secs(DEADLINE_SECS),
    };
    read_native(&mut std::io::Cursor::new(first).chain(rest))
}
