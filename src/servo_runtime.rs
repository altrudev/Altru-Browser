#[cfg(any(feature = "servo-engine", test))]
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServoRuntimeEvidence {
    pub servo_version: &'static str,
    pub width: u32,
    pub height: u32,
    pub load_complete: bool,
    pub frame_ready: bool,
    pub image_bytes: usize,
    pub non_uniform: bool,
    pub frame_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServoRuntimeError {
    FeatureDisabled,
    Context(String),
    Url(String),
    Timeout {
        load_complete: bool,
        frame_ready: bool,
    },
    EmptyFrame,
    UniformFrame,
}

impl std::fmt::Display for ServoRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FeatureDisabled => write!(f, "Servo runtime feature is disabled"),
            Self::Context(error) => write!(f, "Servo rendering context error: {error}"),
            Self::Url(error) => write!(f, "Servo local document URL error: {error}"),
            Self::Timeout {
                load_complete,
                frame_ready,
            } => write!(
                f,
                "Servo runtime timed out: load_complete={load_complete}, frame_ready={frame_ready}"
            ),
            Self::EmptyFrame => write!(f, "Servo rendered no readable frame"),
            Self::UniformFrame => write!(
                f,
                "Servo frame readback is uniform and does not prove document paint"
            ),
        }
    }
}

#[cfg(feature = "servo-engine")]
mod enabled {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    use dpi::PhysicalSize;
    use servo::{
        DeviceIntPoint, DeviceIntRect, DeviceIntSize, EventLoopWaker, LoadStatus, Preferences,
        RenderingContext, ServoBuilder, SoftwareRenderingContext, WebView, WebViewBuilder,
        WebViewDelegate,
    };
    use url::Url;

    use super::{ServoRuntimeError, ServoRuntimeEvidence, sha256_hex};

    #[derive(Clone)]
    struct RuntimeWaker(Arc<AtomicBool>);

    impl EventLoopWaker for RuntimeWaker {
        fn clone_box(&self) -> Box<dyn EventLoopWaker> {
            Box::new(self.clone())
        }

        fn wake(&self) {
            self.0.store(true, Ordering::Release);
        }
    }

    #[derive(Default)]
    struct RuntimeDelegate {
        frame_ready: Cell<bool>,
        load_complete: Cell<bool>,
        crashed: Cell<bool>,
    }

    impl WebViewDelegate for RuntimeDelegate {
        fn notify_new_frame_ready(&self, webview: WebView) {
            webview.paint();
            self.frame_ready.set(true);
        }

        fn notify_load_status_changed(&self, _webview: WebView, status: LoadStatus) {
            if status == LoadStatus::Complete {
                self.load_complete.set(true);
            }
        }

        fn notify_crashed(&self, _webview: WebView, _reason: String, _backtrace: Option<String>) {
            self.crashed.set(true);
        }
    }

    pub fn render_local_document(
        html: &str,
        width: u32,
        height: u32,
        timeout: Duration,
    ) -> Result<ServoRuntimeEvidence, ServoRuntimeError> {
        let size = PhysicalSize {
            width: width.max(1),
            height: height.max(1),
        };
        let context: Rc<dyn RenderingContext> = Rc::new(
            SoftwareRenderingContext::new(size)
                .map_err(|error| ServoRuntimeError::Context(format!("{error:?}")))?,
        );
        context
            .make_current()
            .map_err(|error| ServoRuntimeError::Context(format!("{error:?}")))?;

        let wake_flag = Arc::new(AtomicBool::new(false));
        let mut preferences = Preferences::default();
        preferences.network_http_proxy_uri = String::new();
        preferences.network_https_proxy_uri = String::new();

        let servo = ServoBuilder::default()
            .preferences(preferences)
            .event_loop_waker(Box::new(RuntimeWaker(wake_flag.clone())))
            .build();

        let delegate = Rc::new(RuntimeDelegate::default());
        let data_url = Url::parse(&format!(
            "data:text/html;charset=utf-8,{}",
            percent_encode_html(html)
        ))
        .map_err(|error| ServoRuntimeError::Url(error.to_string()))?;

        let webview = WebViewBuilder::new(&servo, context.clone())
            .delegate(delegate.clone())
            .url(data_url)
            .build();

        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            servo.spin_event_loop();

            if delegate.crashed.get() {
                return Err(ServoRuntimeError::Context(
                    "Servo WebView reported a crash".into(),
                ));
            }

            if delegate.load_complete.get() && delegate.frame_ready.get() {
                break;
            }

            if !wake_flag.swap(false, Ordering::AcqRel) {
                thread::sleep(Duration::from_millis(1));
            }
        }

        if !(delegate.load_complete.get() && delegate.frame_ready.get()) {
            return Err(ServoRuntimeError::Timeout {
                load_complete: delegate.load_complete.get(),
                frame_ready: delegate.frame_ready.get(),
            });
        }

        // Paint once more immediately before readback so the back buffer contains
        // the frame we bind into the evidence receipt.
        webview.paint();

        let rect = DeviceIntRect::from_origin_and_size(
            DeviceIntPoint::new(0, 0),
            DeviceIntSize::new(size.width as i32, size.height as i32),
        );
        let image = context
            .read_to_image(rect)
            .ok_or(ServoRuntimeError::EmptyFrame)?;
        let raw = image.as_raw();
        if raw.is_empty() {
            return Err(ServoRuntimeError::EmptyFrame);
        }

        let first_pixel = raw.get(0..4).ok_or(ServoRuntimeError::EmptyFrame)?;
        let non_uniform = raw.chunks_exact(4).any(|pixel| pixel != first_pixel);
        if !non_uniform {
            return Err(ServoRuntimeError::UniformFrame);
        }

        let frame_sha256 = sha256_hex(raw);
        context.present();

        Ok(ServoRuntimeEvidence {
            servo_version: "0.6.0",
            width: size.width,
            height: size.height,
            load_complete: delegate.load_complete.get(),
            frame_ready: delegate.frame_ready.get(),
            image_bytes: raw.len(),
            non_uniform,
            frame_sha256,
        })
    }

    fn percent_encode_html(input: &str) -> String {
        let mut encoded = String::with_capacity(input.len());
        for byte in input.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    encoded.push(byte as char)
                }
                _ => encoded.push_str(&format!("%{byte:02X}")),
            }
        }
        encoded
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn data_url_encoding_is_deterministic() {
            assert_eq!(
                percent_encode_html("<p>A & B</p>"),
                "%3Cp%3EA%20%26%20B%3C%2Fp%3E"
            );
        }
    }
}

#[cfg(any(feature = "servo-engine", test))]
fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(feature = "servo-engine")]
pub use enabled::render_local_document;

#[cfg(not(feature = "servo-engine"))]
pub fn render_local_document(
    _html: &str,
    _width: u32,
    _height: u32,
    _timeout: std::time::Duration,
) -> Result<ServoRuntimeEvidence, ServoRuntimeError> {
    Err(ServoRuntimeError::FeatureDisabled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_hash_is_sha256_width() {
        assert_eq!(sha256_hex(b"a").len(), 64);
    }

    #[cfg(not(feature = "servo-engine"))]
    #[test]
    fn runtime_fails_closed_without_feature() {
        let result = render_local_document(
            "<html><body>test</body></html>",
            64,
            64,
            std::time::Duration::from_millis(1),
        );
        assert_eq!(result, Err(ServoRuntimeError::FeatureDisabled));
    }
}
