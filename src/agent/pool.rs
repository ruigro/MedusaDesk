//! Session pool for the long-running MCP/HTTP servers.
//!
//! One protocol connection supports one ConnType, so sessions are keyed by
//! (peer, kind): input/screenshot/clipboard ride DEFAULT_CONN, exec rides
//! TERMINAL, file transfer rides FILE_TRANSFER. Idle sessions are reaped.

use std::{
    collections::HashMap,
    sync::Arc,
    time::Duration,
};

use hbb_common::{
    log,
    rendezvous_proto::ConnType,
    tokio::{self, sync::Mutex},
    ResultType,
};

use super::session::{AgentSession, Auth};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Kind {
    Control,
    Terminal,
    FileTransfer,
}

impl Kind {
    fn conn_type(self) -> ConnType {
        match self {
            Kind::Control => ConnType::DEFAULT_CONN,
            Kind::Terminal => ConnType::TERMINAL,
            Kind::FileTransfer => ConnType::FILE_TRANSFER,
        }
    }
}

pub struct SessionPool {
    auth: Auth,
    idle: Duration,
    map: Mutex<HashMap<(String, Kind), Arc<AgentSession>>>,
}

impl SessionPool {
    pub fn new(auth: Auth, idle: Duration) -> Arc<Self> {
        let pool = Arc::new(Self {
            auth,
            idle,
            map: Default::default(),
        });
        let weak = Arc::downgrade(&pool);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                match weak.upgrade() {
                    Some(pool) => pool.reap().await,
                    None => break,
                }
            }
        });
        pool
    }

    pub async fn get(&self, peer: &str, kind: Kind) -> ResultType<Arc<AgentSession>> {
        let key = (peer.to_owned(), kind);
        let mut map = self.map.lock().await;
        if let Some(sess) = map.get(&key) {
            if sess.is_alive() {
                sess.touch();
                return Ok(sess.clone());
            }
            map.remove(&key);
        }
        log::info!("[agent] connecting to {peer} ({kind:?})");
        let sess = Arc::new(
            AgentSession::connect(peer, self.auth.clone(), kind.conn_type(), CONNECT_TIMEOUT)
                .await?,
        );
        map.insert(key, sess.clone());
        Ok(sess)
    }

    async fn reap(&self) {
        let mut closing = Vec::new();
        {
            let mut map = self.map.lock().await;
            map.retain(|key, sess| {
                let keep = sess.is_alive() && sess.idle_for() < self.idle;
                if !keep {
                    log::info!("[agent] closing idle session {key:?}");
                    closing.push(sess.clone());
                }
                keep
            });
        }
        for sess in closing {
            sess.close().await;
        }
    }

    pub async fn close_all(&self) {
        let sessions: Vec<_> = self.map.lock().await.drain().map(|(_, s)| s).collect();
        for sess in sessions {
            sess.close().await;
        }
    }

    pub async fn status(&self) -> Vec<(String, String, bool)> {
        self.map
            .lock()
            .await
            .iter()
            .map(|((peer, kind), sess)| (peer.clone(), format!("{kind:?}"), sess.is_alive()))
            .collect()
    }
}
