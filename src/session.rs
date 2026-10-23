use crate::protocol::{BridgeError, Envelope, Payload, Role};
use std::collections::HashMap;

pub fn token() -> Result<String, BridgeError> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| BridgeError::Unavailable)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
#[derive(Clone)]
pub struct Session {
    pub role: Role,
    pub epoch: String,
    id: String,
}
impl Session {
    pub fn id(&self) -> &str {
        &self.id
    }
}
pub struct SessionRegistry {
    epoch: String,
    active: bool,
    sessions: HashMap<String, Role>,
}
impl SessionRegistry {
    pub fn new() -> Result<Self, BridgeError> {
        Ok(Self {
            epoch: token()?,
            active: true,
            sessions: HashMap::new(),
        })
    }
    pub fn epoch(&self) -> &str {
        &self.epoch
    }
    pub fn open(&mut self, role: Role) -> Result<Session, BridgeError> {
        if !self.active {
            return Err(BridgeError::StaleSession);
        }
        if self
            .sessions
            .values()
            .filter(|value| **value == role)
            .count()
            >= 32
        {
            return Err(BridgeError::Unavailable);
        }
        let session = Session {
            role,
            epoch: self.epoch.clone(),
            id: token()?,
        };
        self.sessions.insert(session.id.clone(), role);
        Ok(session)
    }
    pub fn close(&mut self, session: &Session) {
        self.sessions.remove(&session.id);
    }
    pub fn revoke_all(&mut self) {
        self.active = false;
        self.sessions.clear();
    }
    pub fn authorize(&self, session: &Session, frame: &Envelope) -> Result<(), BridgeError> {
        frame.validate()?;
        if !self.active
            || frame.epoch != self.epoch
            || session.epoch != self.epoch
            || !self.sessions.contains_key(&session.id)
        {
            return Err(BridgeError::StaleSession);
        }
        match &frame.payload {
            Payload::Command { role, .. }
                if *role == session.role && self.sessions.get(&session.id) == Some(role) =>
            {
                Ok(())
            }
            _ => Err(BridgeError::Unauthorized),
        }
    }
    pub(crate) fn client_session(role: Role, epoch: String) -> Result<Session, BridgeError> {
        Ok(Session {
            role,
            epoch,
            id: token()?,
        })
    }
}
