//! Demo: native Rust transcription. Build/run with --features model:
//!   SUBMATE_WHISPER_MODEL=<ggml.bin> cargo run -p submate-whisper \
//!     --features model --example demo -- fixtures/clips/clipA.wav
#[cfg(feature = "model")]
#[tokio::main]
async fn main() {
    let model = std::env::var("SUBMATE_WHISPER_MODEL").expect("set SUBMATE_WHISPER_MODEL");
    let clip = std::env::args().nth(1).expect("pass a .wav path");
    let t = submate_whisper::transcribe(
        model,
        std::path::Path::new(&clip),
        submate_whisper::DEFAULT_REGROUP,
        submate_whisper::TranscribeOptions::default(),
    )
    .await
    .expect("transcribe");
    println!(
        "--- text ---\n{}\n--- srt ---\n{}",
        t.text(),
        t.to_srt_vtt(false)
    );
}
#[cfg(not(feature = "model"))]
fn main() {
    eprintln!("build with --features model");
}
