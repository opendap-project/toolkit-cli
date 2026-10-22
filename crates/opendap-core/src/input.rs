//! Input events.

use opendap_model::{Error, InputEvent, InputSent, Transport};

use crate::Outcome;
use crate::ports::InputSink;

/// Send one event to `target`. The caller resolved the target.
pub fn send<S: InputSink>(
    sink: &S,
    target: Transport,
    event: InputEvent,
    dry_run: bool,
) -> Result<Outcome<InputSent>, Error> {
    if dry_run {
        return Ok(Outcome::Plan(vec![sink.describe(&event)]));
    }
    sink.send(&event)?;
    Ok(Outcome::Done(InputSent {
        target,
        event,
        changed: true,
    }))
}

#[cfg(test)]
mod tests {
    use opendap_model::Button;

    use super::*;
    use crate::fake::FakeInput;

    #[test]
    fn sends_or_plans() {
        let sink = FakeInput::default();
        let t = Transport::Vm {
            name: "default".into(),
        };
        let key = InputEvent::Key {
            button: Button::VolUp,
            hold_ms: 100,
        };
        assert!(matches!(
            send(&sink, t.clone(), key, true).unwrap(),
            Outcome::Plan(_)
        ));
        assert!(sink.sent.borrow().is_empty());
        let Outcome::Done(sent) = send(&sink, t, key, false).unwrap() else {
            panic!()
        };
        assert!(sent.changed);
        assert_eq!(*sink.sent.borrow(), [key]);
        assert_eq!(
            serde_json::to_value(sent.event).unwrap(),
            serde_json::json!({"kind": "key", "button": "vol-up", "hold_ms": 100})
        );
    }
}
