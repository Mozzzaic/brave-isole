use crate::ipc::{read_authenticated, read_until};
use crate::{
    framing::{read_native, write_native},
    ipc::{BridgeConfig, validate_origin, validate_secret, verify_peer},
    protocol::{BridgeError, DEADLINE_SECS, Envelope, Payload, Reply, Role},
    session::{SessionRegistry, token},
};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    os::unix::{fs::MetadataExt, net::UnixStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

type Pending = HashMap<String, (Role, SyncSender<Result<Reply, BridgeError>>)>;
struct Broker {
    registry: Mutex<SessionRegistry>,
    pending: Mutex<Pending>,
    writer: Mutex<Box<dyn Write + Send>>,
    alive: AtomicBool,
}
impl Broker {
    fn send(&self, frame: &Envelope) -> Result<(), BridgeError> {
        write_native(
            &mut *self.writer.lock().map_err(|_| BridgeError::Unavailable)?,
            frame,
        )
    }
    fn stop(&self) {
        if let Ok(mut registry) = self.registry.lock() {
            registry.revoke_all();
        }
        self.alive.store(false, Ordering::SeqCst);
        if let Ok(mut pending) = self.pending.lock() {
            for (_, (_, sender)) in pending.drain() {
                let _ = sender.try_send(Err(BridgeError::StaleSession));
            }
        }
    }
}
fn serve_client(
    mut stream: UnixStream,
    config: BridgeConfig,
    broker: Arc<Broker>,
) -> Result<(), BridgeError> {
    verify_peer(&stream)?;
    stream
        .set_nonblocking(false)
        .map_err(|_| BridgeError::Unavailable)?;
    stream
        .set_read_timeout(Some(Duration::from_secs(DEADLINE_SECS + 1)))
        .map_err(|_| BridgeError::Unavailable)?;
    stream
        .set_write_timeout(Some(Duration::from_secs(DEADLINE_SECS)))
        .map_err(|_| BridgeError::Unavailable)?;
    let first = read_until(
        &mut stream,
        Instant::now() + Duration::from_secs(DEADLINE_SECS),
    )?
    .ok_or(BridgeError::Disconnected)?;
    let role = match first.payload {
        Payload::Authenticate { role, secret } => {
            validate_secret(secret.expose(), &config.secret(role)?)?;
            role
        }
        _ => return Err(BridgeError::Unauthorized),
    };
    let session = broker
        .registry
        .lock()
        .map_err(|_| BridgeError::Unavailable)?
        .open(role)?;
    let result = (|| {
        write_native(
            &mut stream,
            &Envelope {
                version: 1,
                expires_at_ms: None,
                epoch: session.epoch.clone(),
                id: "auth".into(),
                payload: Payload::Hello {},
            },
        )?;
        while broker.alive.load(Ordering::SeqCst) {
            let Some(frame) = read_authenticated(&mut stream)? else {
                break;
            };
            let id = frame.id.clone();
            let answer = (|| {
                broker
                    .registry
                    .lock()
                    .map_err(|_| BridgeError::Unavailable)?
                    .authorize(&session, &frame)?;
                let (sender, receiver) = mpsc::sync_channel(1);
                {
                    let mut pending = broker
                        .pending
                        .lock()
                        .map_err(|_| BridgeError::Unavailable)?;
                    if pending.values().filter(|(owner, _)| *owner == role).count() >= 32
                        || pending.contains_key(&id)
                    {
                        return Err(BridgeError::Unavailable);
                    }
                    pending.insert(id.clone(), (role, sender));
                }
                if let Err(error) = broker.send(&frame) {
                    broker
                        .pending
                        .lock()
                        .map_err(|_| BridgeError::Unavailable)?
                        .remove(&id);
                    return Err(error);
                }
                let result = receiver
                    .recv_timeout(Duration::from_secs(DEADLINE_SECS))
                    .map_err(|_| BridgeError::Timeout);
                broker
                    .pending
                    .lock()
                    .map_err(|_| BridgeError::Unavailable)?
                    .remove(&id);
                result?
            })();
            let reply = answer.unwrap_or_else(Reply::failure);
            write_native(
                &mut stream,
                &Envelope {
                    version: 1,
                    expires_at_ms: None,
                    epoch: session.epoch.clone(),
                    id,
                    payload: Payload::Reply { reply },
                },
            )?;
        }
        Ok(())
    })();
    if let Ok(mut registry) = broker.registry.lock() {
        registry.close(&session);
    }
    result
}

pub fn run_native<R: Read + Send + 'static, W: Write + Send + 'static>(
    mut input: R,
    output: W,
    config: BridgeConfig,
    origin: &str,
) -> Result<(), BridgeError> {
    validate_origin(origin, &config.extension_id)?;
    let listener = config.bind()?;
    listener
        .set_nonblocking(true)
        .map_err(|_| BridgeError::Unavailable)?;
    let socket_inode = fs::symlink_metadata(config.socket())
        .map_err(|_| BridgeError::Unavailable)?
        .ino();
    let registry = SessionRegistry::new()?;
    let epoch = registry.epoch().to_owned();
    let broker = Arc::new(Broker {
        registry: Mutex::new(registry),
        pending: Mutex::new(HashMap::new()),
        writer: Mutex::new(Box::new(output)),
        alive: AtomicBool::new(true),
    });
    broker.send(&Envelope {
        version: 1,
        expires_at_ms: None,
        epoch: epoch.clone(),
        id: token()?,
        payload: Payload::Hello {},
    })?;
    let reader_broker = Arc::clone(&broker);
    thread::spawn(move || {
        while let Ok(Some(frame)) = read_native(&mut input) {
            if frame.epoch != epoch {
                continue;
            }
            match frame.payload {
                Payload::Reply { reply } => {
                    if let Ok(mut pending) = reader_broker.pending.lock() {
                        if let Some((_, sender)) = pending.remove(&frame.id) {
                            let _ = sender.try_send(Ok(reply));
                        }
                    }
                }
                Payload::Stop {} => break,
                _ => break,
            }
        }
        reader_broker.stop();
    });
    while broker.alive.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                let next_config = config.clone();
                let next_broker = Arc::clone(&broker);
                thread::spawn(move || {
                    let _ = serve_client(stream, next_config, next_broker);
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10))
            }
            Err(_) => {
                broker.stop();
                break;
            }
        }
    }
    drop(listener);
    if fs::symlink_metadata(config.socket()).is_ok_and(|meta| meta.ino() == socket_inode) {
        let _ = fs::remove_file(config.socket());
    }
    Ok(())
}
