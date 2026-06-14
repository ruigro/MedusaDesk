//! Headless [`InvokeUiSession`] implementation.
//!
//! `Remote<T>` (the existing session engine) talks to its UI through the
//! ~50-method `InvokeUiSession` trait. `AgentUi` implements it with no-ops for
//! everything visual and routes the handful of results agents care about
//! (connection state, screenshots, terminal output, file-job completion) into
//! async channels consumed by [`super::session::AgentSession`].

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
};

use hbb_common::{
    log,
    message_proto::*,
    rendezvous_proto::ConnType,
    tokio::sync::{mpsc, oneshot, watch},
};

use crate::client::{Data, QualityStatus};
use crate::ui_session_interface::InvokeUiSession;

#[derive(Clone, Debug)]
pub enum ConnState {
    Connecting,
    Ready,
    Error(String),
}

#[derive(Debug)]
pub enum TermEvent {
    Opened {
        terminal_id: i32,
        success: bool,
        message: String,
    },
    Data(Vec<u8>),
    Closed {
        terminal_id: i32,
        exit_code: i32,
    },
    Error {
        terminal_id: i32,
        message: String,
    },
}

pub struct AgentState {
    state_tx: watch::Sender<ConnState>,
    pub state_rx: watch::Receiver<ConnState>,
    // The same Arc the io_loop populates with the session's Data sender.
    pub sender: RwLock<Option<Arc<RwLock<Option<mpsc::UnboundedSender<Data>>>>>>,
    pub password: RwLock<String>,
    pub totp_secret: RwLock<Option<String>>,
    pub peer_info: RwLock<Option<PeerInfo>>,
    pub screenshot_tx: Mutex<Option<oneshot::Sender<Result<bytes::Bytes, String>>>>,
    pub term_txs: Mutex<HashMap<i32, mpsc::UnboundedSender<TermEvent>>>,
    pub job_txs: Mutex<HashMap<i32, oneshot::Sender<Result<(), String>>>>,
}

impl Default for AgentState {
    fn default() -> Self {
        let (state_tx, state_rx) = watch::channel(ConnState::Connecting);
        Self {
            state_tx,
            state_rx,
            sender: Default::default(),
            password: Default::default(),
            totp_secret: Default::default(),
            peer_info: Default::default(),
            screenshot_tx: Default::default(),
            term_txs: Default::default(),
            job_txs: Default::default(),
        }
    }
}

impl AgentState {
    pub fn send_data(&self, data: Data) {
        if let Some(holder) = self.sender.read().unwrap().as_ref() {
            if let Some(sender) = holder.read().unwrap().as_ref() {
                sender.send(data).ok();
            }
        }
    }

    fn set_ready(&self) {
        // Only move forward from Connecting; never mask an error.
        if matches!(*self.state_tx.borrow(), ConnState::Connecting) {
            self.state_tx.send(ConnState::Ready).ok();
        }
    }

    /// Record a fatal error and fail every pending waiter.
    pub fn fail(&self, msg: String) {
        if let Some(tx) = self.screenshot_tx.lock().unwrap().take() {
            tx.send(Err(msg.clone())).ok();
        }
        for (id, tx) in self.term_txs.lock().unwrap().drain() {
            tx.send(TermEvent::Error {
                terminal_id: id,
                message: msg.clone(),
            })
            .ok();
        }
        for (_, tx) in self.job_txs.lock().unwrap().drain() {
            tx.send(Err(msg.clone())).ok();
        }
        self.state_tx.send(ConnState::Error(msg)).ok();
    }
}

#[derive(Clone, Default)]
pub struct AgentUi {
    pub st: Arc<AgentState>,
}

impl InvokeUiSession for AgentUi {
    fn set_cursor_data(&self, _cd: CursorData) {}
    fn set_cursor_id(&self, _id: String) {}
    fn set_cursor_position(&self, _cp: CursorPosition) {}
    fn set_display(&self, _x: i32, _y: i32, _w: i32, _h: i32, _cursor_embedded: bool, _scale: f64) {
    }
    fn switch_display(&self, _display: &SwitchDisplay) {}

    fn set_peer_info(&self, peer_info: &PeerInfo) {
        *self.st.peer_info.write().unwrap() = Some(peer_info.clone());
        self.st.set_ready();
    }

    fn set_displays(&self, _displays: &Vec<DisplayInfo>) {}
    fn set_platform_additions(&self, _data: &str) {}
    fn on_connected(&self, _conn_type: ConnType) {}
    fn update_privacy_mode(&self) {}
    fn set_permission(&self, _name: &str, _value: bool) {}
    fn close_success(&self) {}
    fn update_quality_status(&self, _qs: QualityStatus) {}
    fn set_connection_type(&self, _is_secured: bool, _direct: bool, _stream_type: &str) {}
    fn set_fingerprint(&self, _fingerprint: String) {}

    fn job_error(&self, id: i32, err: String, _file_num: i32) {
        if let Some(tx) = self.st.job_txs.lock().unwrap().remove(&id) {
            tx.send(Err(err)).ok();
        }
    }

    fn job_done(&self, id: i32, _file_num: i32) {
        if let Some(tx) = self.st.job_txs.lock().unwrap().remove(&id) {
            tx.send(Ok(())).ok();
        }
    }

    fn clear_all_jobs(&self) {}
    fn new_message(&self, _msg: String) {}
    fn update_transfer_list(&self) {}
    fn load_last_job(&self, _cnt: i32, _job_json: &str, _auto_start: bool) {}
    fn update_folder_files(
        &self,
        _id: i32,
        _entries: &Vec<FileEntry>,
        _path: String,
        _is_local: bool,
        _only_count: bool,
    ) {
    }
    fn confirm_delete_files(&self, _id: i32, _i: i32, _name: String) {}

    fn override_file_confirm(
        &self,
        id: i32,
        file_num: i32,
        _to: String,
        is_upload: bool,
        _is_identical: bool,
    ) {
        // Headless: always overwrite, remember for the rest of the job.
        self.st.send_data(Data::SetConfirmOverrideFile((
            id, file_num, true, true, is_upload,
        )));
    }

    fn update_block_input_state(&self, _on: bool) {}
    fn job_progress(&self, _id: i32, _file_num: i32, _speed: f64, _finished_size: f64) {}
    fn adapt_size(&self) {}
    fn on_rgba(&self, _display: usize, _rgba: &mut scrap::ImageRgb) {}

    fn msgbox(&self, msgtype: &str, title: &str, text: &str, _link: &str, _retry: bool) {
        match msgtype {
            "input-password" => {
                // Empty preset/saved password: send a blank login so the remote
                // side can fall back to click-approval.
                let pw = self.st.password.read().unwrap().clone();
                self.st
                    .send_data(Data::Login(("".to_owned(), "".to_owned(), pw, false)));
            }
            "re-input-password" => {
                self.st.fail(format!("Wrong password ({}: {})", title, text));
                self.st.send_data(Data::Close);
            }
            "input-2fa" => {
                let secret = self.st.totp_secret.read().unwrap().clone();
                match secret.as_deref().and_then(super::session::totp_code) {
                    Some(code) => {
                        let mut msg = Message::new();
                        msg.set_auth_2fa(Auth2FA {
                            code,
                            ..Default::default()
                        });
                        self.st.send_data(Data::Message(msg));
                    }
                    None => {
                        self.st.fail(
                            "2FA required: pass a TOTP secret via --2fa-secret".to_owned(),
                        );
                        self.st.send_data(Data::Close);
                    }
                }
            }
            _ => {
                if msgtype.contains("error") {
                    self.st.fail(format!("{}: {}", title, text));
                } else {
                    log::info!("[agent] {}: {}: {}", msgtype, title, text);
                }
            }
        }
    }

    fn cancel_msgbox(&self, _tag: &str) {}
    fn switch_back(&self, _id: &str) {}
    fn portable_service_running(&self, _running: bool) {}
    fn on_voice_call_started(&self) {}
    fn on_voice_call_closed(&self, _reason: &str) {}
    fn on_voice_call_waiting(&self) {}
    fn on_voice_call_incoming(&self) {}

    fn get_rgba(&self, _display: usize) -> *const u8 {
        std::ptr::null()
    }

    fn next_rgba(&self, _display: usize) {}

    #[cfg(all(feature = "vram", feature = "flutter"))]
    fn on_texture(&self, _display: usize, _texture: *mut std::ffi::c_void) {}

    fn set_multiple_windows_session(&self, _sessions: Vec<WindowsSession>) {}
    fn set_current_display(&self, _disp_idx: i32) {}

    #[cfg(feature = "flutter")]
    fn is_multi_ui_session(&self) -> bool {
        false
    }

    fn update_record_status(&self, _start: bool) {}
    fn printer_request(&self, _id: i32, _path: String) {}

    fn handle_screenshot_resp(&self, _sid: String, msg: String) {
        if let Some(tx) = self.st.screenshot_tx.lock().unwrap().take() {
            if msg.is_empty() {
                let data = crate::client::screenshot::take_screenshot_data().unwrap_or_default();
                tx.send(Ok(data)).ok();
            } else {
                tx.send(Err(msg)).ok();
            }
        }
    }

    fn handle_terminal_response(&self, response: TerminalResponse) {
        use hbb_common::message_proto::terminal_response::Union;
        let (terminal_id, event) = match response.union {
            Some(Union::Opened(o)) => (
                o.terminal_id,
                TermEvent::Opened {
                    terminal_id: o.terminal_id,
                    success: o.success,
                    message: o.message.clone(),
                },
            ),
            Some(Union::Data(d)) => {
                let bytes = if d.compressed {
                    hbb_common::compress::decompress(&d.data)
                } else {
                    d.data.to_vec()
                };
                (d.terminal_id, TermEvent::Data(bytes))
            }
            Some(Union::Closed(c)) => (
                c.terminal_id,
                TermEvent::Closed {
                    terminal_id: c.terminal_id,
                    exit_code: c.exit_code,
                },
            ),
            Some(Union::Error(e)) => (
                e.terminal_id,
                TermEvent::Error {
                    terminal_id: e.terminal_id,
                    message: e.message.clone(),
                },
            ),
            _ => return,
        };
        if let Some(tx) = self.st.term_txs.lock().unwrap().get(&terminal_id) {
            tx.send(event).ok();
        }
    }
}
