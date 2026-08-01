use std::backtrace::Backtrace;

/// panic を tracing の ERROR イベントとして記録する panic hook を登録します。
///
/// `panic = true` フィールドは seichi_infra 側の LogQL ルール
/// (`| json | panic="true"`) とアプリ間の契約であり、
/// 変更する場合は LogQL ルール側も直すこと。
///
/// 既存の hook (stderr への出力) はチェーンして維持する。
pub fn install() {
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let payload = panic_info.payload();
        let message = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("<non-string panic payload>");
        let location = panic_info
            .location()
            .map(ToString::to_string)
            .unwrap_or_else(|| "<unknown location>".to_owned());
        let backtrace = Backtrace::force_capture();

        tracing::error!(
            panic = true,
            panic.location = %location,
            backtrace = %backtrace,
            "panicked: {message}"
        );

        previous_hook(panic_info);
    }));
}

#[cfg(test)]
mod tests {
    use std::{
        io,
        panic::catch_unwind,
        sync::{Arc, Mutex},
    };

    use tracing_subscriber::{fmt::MakeWriter, layer::SubscriberExt};

    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<u8>>>);

    impl io::Write for Capture {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for Capture {
        type Writer = Capture;

        fn make_writer(&'a self) -> Capture {
            self.clone()
        }
    }

    #[test]
    fn panic_is_logged_with_panic_field_and_backtrace() {
        let capture = Capture::default();
        let subscriber = tracing_subscriber::registry()
            .with(crate::logging::json_log_layer().with_writer(capture.clone()));

        super::install();
        tracing::subscriber::with_default(subscriber, || {
            let _ = catch_unwind(|| panic!("boom for test"));
        });

        let bytes = capture.0.lock().unwrap();
        let text = std::str::from_utf8(&bytes).expect("log output must be valid UTF-8");
        let json: serde_json::Value =
            serde_json::from_str(text.lines().next().expect("a log line must be written"))
                .expect("log line must be valid JSON");

        assert_eq!(json["panic"], true, "LogQL ルールとの契約フィールド");
        assert_eq!(json["level"], "ERROR");
        assert!(
            json["message"]
                .as_str()
                .is_some_and(|message| message.contains("boom for test"))
        );
        assert!(
            json["backtrace"]
                .as_str()
                .is_some_and(|backtrace| !backtrace.is_empty())
        );
    }
}
