//! The controls of a virtual device in real time: a QMP session of its own (QEMU's documented
//! monitor protocol), held open, so a key goes down when a finger does and up when it lifts,
//! without starting a process per event. For surfaces that follow a hand, such as the simulator
//! window; one-off events go through `odap-vdev` ([`crate::VirtualDevice`]).
//!
//! The socket is one `odap-vdev` opened for this surface (a QMP socket serves one client at a
//! time), and the roles' QEMU key codes come from `odap-vdev frame` (its `controls`).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use opendap_core::InputSink;
use opendap_model::error as codes;
use opendap_model::{Error, InputEvent, Role};
use serde_json::{Value, json};

type Session = (UnixStream, BufReader<UnixStream>);

/// Keys over a held QMP session; it reconnects when the session dropped (the device restarted).
pub struct LiveKeys {
    socket: PathBuf,
    /// Each role and its QEMU key code.
    keys: Vec<(Role, String)>,
    session: Mutex<Option<Session>>,
}

impl LiveKeys {
    pub fn new(socket: &Path, keys: Vec<(Role, String)>) -> Self {
        LiveKeys {
            socket: socket.to_path_buf(),
            keys,
            session: Mutex::new(None),
        }
    }

    fn open(&self) -> std::io::Result<Session> {
        let s = UnixStream::connect(&self.socket)?;
        s.set_read_timeout(Some(Duration::from_secs(5)))?;
        let mut r = BufReader::new(s.try_clone()?);
        let mut line = String::new();
        r.read_line(&mut line)?; // the greeting
        let mut session = (s, r);
        command(&mut session, &json!({"execute": "qmp_capabilities"}))?;
        Ok(session)
    }

    fn qcode(&self, role: &Role) -> Result<&str, Error> {
        self.keys
            .iter()
            .find(|(r, _)| r == role)
            .map(|(_, q)| q.as_str())
            .ok_or_else(|| {
                Error::new(
                    &codes::INPUT_INVALID,
                    format!("the device has no control with role {}", role.as_str()),
                )
            })
    }

    fn key(&self, qcode: &str, down: bool) -> Result<(), Error> {
        let cmd = json!({"execute": "input-send-event", "arguments": {"events": [
            {"type": "key", "data": {"down": down, "key": {"type": "qcode", "data": qcode}}}
        ]}});
        let mut guard = self.session.lock().unwrap_or_else(|p| p.into_inner());
        // Once on the held session; if it dropped, once more on a new one.
        for _ in 0..2 {
            if guard.is_none() {
                *guard = Some(self.open().map_err(|e| qmp_error(&self.socket, e))?);
            }
            match command(guard.as_mut().unwrap(), &cmd) {
                Ok(()) => return Ok(()),
                Err(_) => *guard = None,
            }
        }
        Err(Error::new(
            &codes::TRANSPORT_QMP_FAILED,
            format!("QMP at {} does not take the key", self.socket.display()),
        ))
    }
}

fn qmp_error(socket: &Path, e: std::io::Error) -> Error {
    Error::new(
        &codes::TRANSPORT_QMP_FAILED,
        format!("cannot reach QMP at {}: {e}", socket.display()),
    )
}

/// Send one command and read up to its reply; events in between are skipped.
fn command((w, r): &mut Session, cmd: &Value) -> std::io::Result<()> {
    writeln!(w, "{cmd}")?;
    let mut line = String::new();
    loop {
        line.clear();
        if r.read_line(&mut line)? == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        let v: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
        if v.get("return").is_some() {
            return Ok(());
        }
        if let Some(e) = v.get("error") {
            return Err(std::io::Error::other(e.to_string()));
        }
    }
}

impl InputSink for LiveKeys {
    fn describe(&self, event: &InputEvent) -> String {
        let what = match event {
            InputEvent::Key { role, hold_ms } => {
                format!("press {} for {hold_ms} ms", role.as_str())
            }
            InputEvent::KeyDown { role } => format!("put {} down", role.as_str()),
            InputEvent::KeyUp { role } => format!("release {}", role.as_str()),
            InputEvent::Tap { x, y } => format!("tap {x},{y} (not a control: refused)"),
        };
        format!("QMP {}: {what}", self.socket.display())
    }

    fn send(&self, event: &InputEvent) -> Result<(), Error> {
        match event {
            InputEvent::KeyDown { role } => self.key(self.qcode(role)?, true),
            InputEvent::KeyUp { role } => self.key(self.qcode(role)?, false),
            InputEvent::Key { role, hold_ms } => {
                let q = self.qcode(role)?;
                self.key(q, true)?;
                thread::sleep(Duration::from_millis(*hold_ms));
                self.key(q, false)
            }
            InputEvent::Tap { .. } => Err(Error::new(
                &codes::INPUT_INVALID,
                "live keys carry the controls; the display takes touches through its own link",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixListener;

    use super::*;

    /// A QMP server that answers every command and puts an event before some replies.
    fn server(path: &Path) -> thread::JoinHandle<Vec<Value>> {
        let listener = UnixListener::bind(path).unwrap();
        thread::spawn(move || {
            let (s, _) = listener.accept().unwrap();
            let mut w = s.try_clone().unwrap();
            writeln!(w, r#"{{"QMP": {{"version": {{}}, "capabilities": []}}}}"#).unwrap();
            let mut got = Vec::new();
            for line in BufReader::new(s).lines() {
                let v: Value = serde_json::from_str(&line.unwrap()).unwrap();
                writeln!(w, r#"{{"event": "SOMETHING", "data": {{}}}}"#).unwrap();
                writeln!(w, r#"{{"return": {{}}}}"#).unwrap();
                got.push(v);
            }
            got
        })
    }

    #[test]
    fn keys_go_down_and_up_on_one_session() {
        let dir = std::env::temp_dir().join(format!("odap-live-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("qmp.sock");
        let _ = std::fs::remove_file(&sock);
        let srv = server(&sock);
        let keys = LiveKeys::new(&sock, vec![(Role::from("main"), "p".into())]);
        let main = Role::from("main");
        keys.send(&InputEvent::KeyDown { role: main.clone() })
            .unwrap();
        keys.send(&InputEvent::KeyUp { role: main }).unwrap();
        let err = keys
            .send(&InputEvent::KeyDown {
                role: Role::from("dial"),
            })
            .unwrap_err();
        assert_eq!(err.code.code, "input.invalid");
        drop(keys);
        let got = srv.join().unwrap();
        assert_eq!(got[0]["execute"], "qmp_capabilities");
        let downs: Vec<bool> = got[1..]
            .iter()
            .map(|c| {
                c["arguments"]["events"][0]["data"]["down"]
                    .as_bool()
                    .unwrap()
            })
            .collect();
        assert_eq!(downs, [true, false]);
        assert_eq!(got[1]["arguments"]["events"][0]["data"]["key"]["data"], "p");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn passes_the_input_sink_contract() {
        let keys = LiveKeys::new(Path::new("/nonexistent"), vec![]);
        opendap_core::contract::input_sink(&keys);
    }
}
