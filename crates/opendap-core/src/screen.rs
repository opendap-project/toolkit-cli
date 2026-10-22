//! Display captures.

use opendap_model::error as codes;
use opendap_model::{Error, ScreenCapture, Transport};

use crate::Outcome;
use crate::ports::ScreenSource;

/// Width and height from a PNG header.
pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[..8] != SIG || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

/// Save the display of `target` as a PNG at `path`. The caller resolved
/// the target and chose the path.
pub fn capture<S: ScreenSource>(
    source: &S,
    target: Transport,
    path: &str,
    dry_run: bool,
) -> Result<Outcome<ScreenCapture>, Error> {
    if dry_run {
        return Ok(Outcome::Plan(vec![source.describe(&target, path)]));
    }
    let bytes = source.capture_png(path)?;
    let (width, height) = png_size(&bytes)
        .ok_or_else(|| Error::new(&codes::TRANSPORT_QMP_FAILED, format!("{path} is not a PNG")))?;
    Ok(Outcome::Done(ScreenCapture {
        target,
        path: path.to_string(),
        format: "png".into(),
        width,
        height,
        bytes: bytes.len() as u64,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeScreen;

    fn vm() -> Transport {
        Transport::Vm {
            name: "default".into(),
        }
    }

    #[test]
    fn reads_png_size() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend_from_slice(&320u32.to_be_bytes());
        png.extend_from_slice(&480u32.to_be_bytes());
        assert_eq!(png_size(&png), Some((320, 480)));
        assert_eq!(png_size(b"not a png at all, really no"), None);
    }

    #[test]
    fn capture_reports_the_image() {
        let Outcome::Done(c) = capture(&FakeScreen::default(), vm(), "/tmp/x.png", false).unwrap()
        else {
            panic!()
        };
        assert_eq!((c.width, c.height, c.bytes), (320, 480, 24));
        assert_eq!((c.path.as_str(), c.format.as_str()), ("/tmp/x.png", "png"));
    }

    #[test]
    fn garbage_is_a_transport_error_and_dry_run_plans() {
        let s = FakeScreen {
            garbage: true,
            ..Default::default()
        };
        let err = capture(&s, vm(), "/tmp/x.png", false).unwrap_err();
        assert_eq!(err.code.code, "transport.qmp_failed");
        assert_eq!(err.message, "/tmp/x.png is not a PNG");
        assert_eq!(
            capture(&s, vm(), "/tmp/x.png", true).unwrap(),
            Outcome::Plan(vec!["fake capture of vm:default to /tmp/x.png".into()])
        );
    }
}
