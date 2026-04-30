use rodio::{OutputStream, Sink, Source};
use std::time::Duration;

/// Joue deux tonalités courtes (880 Hz puis 1100 Hz) via rodio
pub(crate) fn play_notification_sound() {
    let Ok((_stream, stream_handle)) = OutputStream::try_default() else { return; };
    let Ok(sink) = Sink::try_new(&stream_handle) else { return; };

    let tone1 = rodio::source::SineWave::new(880.0)
        .take_duration(Duration::from_millis(80))
        .amplify(0.15);
    let tone2 = rodio::source::SineWave::new(1100.0)
        .take_duration(Duration::from_millis(80))
        .amplify(0.15);

    sink.append(tone1);
    sink.append(tone2);
    sink.sleep_until_end();
}
