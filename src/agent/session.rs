//! `AgentSession`: a headless remote-desktop session with an async API.
//!
//! Wraps the existing `Session<T>` engine with `AgentUi` as the headless UI
//! handler and drives it through the same `io_loop` the Flutter client uses,
//! so login/2FA/reconnect/file-job logic is all reused.

use std::{
    sync::{
        atomic::{AtomicI32, Ordering},
        Arc, Mutex, RwLock,
    },
    time::{Duration, Instant},
};

use bytes::Bytes;
use hbb_common::{
    bail,
    message_proto::*,
    rendezvous_proto::ConnType,
    tokio::{self, sync::mpsc, sync::oneshot, time::timeout},
    ResultType,
};

use crate::client::{Data, FileManager, Interface};
use crate::input::{MOUSE_TYPE_DOWN, MOUSE_TYPE_MOVE, MOUSE_TYPE_UP, MOUSE_TYPE_WHEEL};
use crate::ui_session_interface::{io_loop, Session};

use super::ui::{AgentUi, AgentState, ConnState, TermEvent};

pub const MOUSE_BUTTON_LEFT: i32 = crate::input::MOUSE_BUTTON_LEFT;
pub const MOUSE_BUTTON_RIGHT: i32 = crate::input::MOUSE_BUTTON_RIGHT;
pub const MOUSE_BUTTON_MIDDLE: i32 = crate::input::MOUSE_BUTTON_WHEEL;

const SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(20);

/// Compute the current TOTP code from a base32 secret (as shown during 2FA setup).
pub fn totp_code(secret: &str) -> Option<String> {
    use totp_rs::{Algorithm, Secret, TOTP};
    let secret = Secret::Encoded(secret.trim().replace(' ', "").to_uppercase())
        .to_bytes()
        .ok()?;
    TOTP::new_unchecked(Algorithm::SHA1, 6, 1, 30, secret, None, String::new())
        .generate_current()
        .ok()
}

#[derive(Clone, Default)]
pub struct Auth {
    /// Plaintext password; empty falls back to the saved peer password,
    /// then to click-approval on the remote side.
    pub password: String,
    /// Base32 TOTP secret for peers with 2FA enabled.
    pub totp_secret: Option<String>,
}

pub struct AgentSession {
    pub peer: String,
    pub conn_type: ConnType,
    pub(crate) inner: Session<AgentUi>,
    pub(crate) st: Arc<AgentState>,
    pub last_used: Mutex<Instant>,
    term_id_gen: AtomicI32,
}

impl AgentSession {
    pub async fn connect(
        peer: &str,
        auth: Auth,
        conn_type: ConnType,
        ready_timeout: Duration,
    ) -> ResultType<Self> {
        let session: Session<AgentUi> = Session {
            password: auth.password.clone(),
            server_keyboard_enabled: Arc::new(RwLock::new(true)),
            server_file_transfer_enabled: Arc::new(RwLock::new(true)),
            server_clipboard_enabled: Arc::new(RwLock::new(true)),
            ..Default::default()
        };
        session.lc.write().unwrap().initialize(
            peer.to_owned(),
            conn_type,
            None,
            false,
            None,
            None,
            None,
        );

        let st = session.ui_handler.st.clone();
        *st.password.write().unwrap() = auth.password;
        *st.totp_secret.write().unwrap() = auth.totp_secret;
        *st.sender.write().unwrap() = Some(session.sender.clone());

        let round = session.connection_round_state.lock().unwrap().new_round();
        let cloned = session.clone();
        let thread = std::thread::spawn(move || io_loop(cloned, round));
        *session.thread.lock().unwrap() = Some(thread);

        let mut rx = st.state_rx.clone();
        let wait_ready = async {
            loop {
                let state = rx.borrow_and_update().clone();
                match state {
                    ConnState::Ready => return Ok(()),
                    ConnState::Error(e) => bail!(e),
                    ConnState::Connecting => {}
                }
                if rx.changed().await.is_err() {
                    bail!("connection terminated");
                }
            }
        };
        match timeout(ready_timeout, wait_ready).await {
            Ok(res) => res?,
            Err(_) => bail!(
                "Timed out connecting to {} (is the peer online? does it need approval?)",
                peer
            ),
        }

        Ok(Self {
            peer: peer.to_owned(),
            conn_type,
            inner: session,
            st,
            last_used: Mutex::new(Instant::now()),
            term_id_gen: AtomicI32::new(1),
        })
    }

    pub fn touch(&self) {
        *self.last_used.lock().unwrap() = Instant::now();
    }

    pub fn idle_for(&self) -> Duration {
        self.last_used.lock().unwrap().elapsed()
    }

    /// The io_loop thread exits when the connection ends.
    pub fn is_alive(&self) -> bool {
        if let ConnState::Error(_) = &*self.st.state_rx.borrow() {
            return false;
        }
        self.inner
            .thread
            .lock()
            .unwrap()
            .as_ref()
            .map(|h| !h.is_finished())
            .unwrap_or(false)
    }

    pub fn platform(&self) -> String {
        self.inner.peer_platform()
    }

    pub fn peer_version(&self) -> i64 {
        self.inner.lc.read().unwrap().version
    }

    pub fn displays(&self) -> usize {
        self.st
            .peer_info
            .read()
            .unwrap()
            .as_ref()
            .map(|pi| pi.displays.len())
            .unwrap_or(0)
    }

    pub async fn screenshot(&self, display: i32) -> ResultType<Bytes> {
        self.touch();
        if !crate::common::is_support_screenshot_num(self.peer_version()) {
            bail!("The remote peer is too old to support screenshots (needs RustDesk/MedusaDesk >= 1.4.0)");
        }
        let mut last_err = String::new();
        for attempt in 0..2u32 {
            let (tx, rx) = oneshot::channel();
            *self.st.screenshot_tx.lock().unwrap() = Some(tx);
            self.inner
                .send(Data::TakeScreenshot((display, format!("agent-{attempt}"))));
            match timeout(SCREENSHOT_TIMEOUT, rx).await {
                Ok(Ok(Ok(data))) if !data.is_empty() => return Ok(data),
                Ok(Ok(Ok(_))) => last_err = "empty screenshot".to_owned(),
                Ok(Ok(Err(e))) => last_err = e,
                Ok(Err(_)) => bail!("Screenshot aborted (connection lost?)"),
                Err(_) => {
                    last_err = "screenshot timed out".to_owned();
                }
            }
            if attempt == 0 {
                // The remote only fills the screenshot inside its capture
                // loop; make sure the display is being captured, then retry.
                self.inner.capture_displays(vec![], vec![], vec![display]);
                tokio::time::sleep(Duration::from_millis(800)).await;
            }
        }
        bail!("Screenshot failed: {last_err}");
    }

    fn send_mouse(&self, mask: i32, x: i32, y: i32) {
        self.touch();
        self.inner.send_mouse(mask, x, y, false, false, false, false);
    }

    pub fn move_mouse(&self, x: i32, y: i32) {
        self.send_mouse(MOUSE_TYPE_MOVE, x, y);
    }

    pub async fn click(&self, x: i32, y: i32, button: i32, double: bool) {
        let clicks = if double { 2 } else { 1 };
        self.send_mouse(MOUSE_TYPE_MOVE, x, y);
        tokio::time::sleep(Duration::from_millis(60)).await;
        for _ in 0..clicks {
            self.send_mouse(MOUSE_TYPE_DOWN | (button << 3), x, y);
            tokio::time::sleep(Duration::from_millis(60)).await;
            self.send_mouse(MOUSE_TYPE_UP | (button << 3), x, y);
            tokio::time::sleep(Duration::from_millis(60)).await;
        }
    }

    pub fn scroll(&self, dx: i32, dy: i32) {
        self.send_mouse(MOUSE_TYPE_WHEEL, dx, dy);
    }

    pub fn type_text(&self, text: &str) {
        self.touch();
        self.inner.input_string(text);
    }

    /// `name` is a single character or a VK_* name from `crate::client::KEY_MAP`
    /// (the CLI/MCP layers map friendly names like "enter" first).
    pub fn key(&self, name: &str, alt: bool, ctrl: bool, shift: bool, command: bool) {
        self.touch();
        self.inner.input_key(name, false, true, alt, ctrl, shift, command);
    }

    pub async fn exec(&self, cmd: &str, run_timeout: Duration) -> ResultType<super::ExecResult> {
        self.touch();
        super::exec::exec(self, cmd, run_timeout).await
    }

    /// Upload (`upload == true`, local -> remote) or download a file/directory.
    pub async fn transfer(
        &self,
        local: &str,
        remote: &str,
        upload: bool,
        job_timeout: Duration,
    ) -> ResultType<()> {
        self.touch();
        let id = hbb_common::fs::get_next_job_id();
        let (tx, rx) = oneshot::channel();
        self.st.job_txs.lock().unwrap().insert(id, tx);
        let (path, to) = if upload {
            (local, remote)
        } else {
            (remote, local)
        };
        self.inner.send_files(
            id,
            hbb_common::fs::JobType::Generic as i32,
            path.to_owned(),
            to.to_owned(),
            0,
            true,
            !upload,
        );
        let res = match timeout(job_timeout, rx).await {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(e))) => Err(hbb_common::anyhow::anyhow!("Transfer failed: {e}")),
            Ok(Err(_)) => Err(hbb_common::anyhow::anyhow!(
                "Transfer aborted (connection lost?)"
            )),
            Err(_) => Err(hbb_common::anyhow::anyhow!("Transfer timed out")),
        };
        self.st.job_txs.lock().unwrap().remove(&id);
        res
    }

    pub fn clipboard_set(&self, text: &str) {
        self.touch();
        let mut msg = Message::new();
        msg.set_clipboard(Clipboard {
            compress: false,
            content: Bytes::from(text.as_bytes().to_vec()),
            format: ClipboardFormat::Text.into(),
            ..Default::default()
        });
        self.inner.send(Data::Message(msg));
    }

    /// Reads the local clipboard. With clipboard sync enabled, content copied
    /// on the remote side lands here; this is best-effort by protocol design.
    pub fn clipboard_get(&self) -> ResultType<String> {
        self.touch();
        let mut cb = arboard::Clipboard::new()?;
        Ok(cb.get_text()?)
    }

    pub(crate) fn register_terminal(&self) -> (i32, mpsc::UnboundedReceiver<TermEvent>) {
        let id = self.term_id_gen.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::unbounded_channel();
        self.st.term_txs.lock().unwrap().insert(id, tx);
        (id, rx)
    }

    pub(crate) fn unregister_terminal(&self, id: i32) {
        self.st.term_txs.lock().unwrap().remove(&id);
    }

    pub(crate) fn open_terminal(&self, terminal_id: i32, rows: u32, cols: u32) {
        self.inner.open_terminal(terminal_id, rows, cols);
    }

    pub(crate) fn send_terminal_input(&self, terminal_id: i32, data: String) {
        self.inner.send_terminal_input(terminal_id, data);
    }

    pub(crate) fn close_terminal(&self, terminal_id: i32) {
        self.inner.close_terminal(terminal_id);
    }

    /// Politely close the connection and give the io_loop a moment to flush.
    pub async fn close(&self) {
        self.inner.send(Data::Close);
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}
