//! `submate` binary — clap CLI.
//!
//! Wires the server + node binaries together behind the user-facing
//! subcommands `transcribe / translate / node / server / probe / config`:
//!
//! * `submate server` runs the coordinator (axum) with an *embedded* processing
//!   node by default, so a single-box deployment needs no separate worker.
//! * `submate node --server <url>` runs a remote processing node that pulls work
//!   from a coordinator over HTTP — the multi-box analogue.
//! * `submate transcribe --sync` spins up a one-shot local coordinator + node in
//!   the same process and drains exactly the enqueued jobs before returning —
//!   the "process immediately, no worker required" path.
//!
//! Pure sub-helpers that decide *which* files to process and *how* the
//! `config show` table is laid out live in their own modules ([`config_show`],
//! [`translate_paths`], [`transcribe_collect`]); this file is the clap wiring
//! and the IO around them.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand, ValueEnum};
use submate_config::Config;
use submate_media::{AudioSelector, AudioTrack};

/// User-selectable subtitle output format for `submate transcribe` (`-F/--format`).
///
/// A clap `ValueEnum` so the `--help` value list and parsing live with the CLI;
/// [`From`] converts it into [`submate_types::OutputFormat`] for rendering, and
/// [`OutputFormat::extension`] drives the output filename.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
enum OutputFormat {
    /// SubRip subtitles (`.srt`).
    #[default]
    Srt,
    /// WebVTT subtitles (`.vtt`).
    Vtt,
    /// Advanced SubStation Alpha subtitles (`.ass`).
    Ass,
    /// JSON dump of the full transcription result (`.json`).
    Json,
    /// Plain-text transcript, no timestamps (`.txt`).
    Txt,
}

impl OutputFormat {
    /// File extension including the leading dot (e.g. `".srt"`), used to name the
    /// output next to the input file.
    fn extension(self) -> &'static str {
        submate_types::OutputFormat::from(self).extension()
    }
}

impl From<OutputFormat> for submate_types::OutputFormat {
    fn from(f: OutputFormat) -> Self {
        match f {
            OutputFormat::Srt => Self::Srt,
            OutputFormat::Vtt => Self::Vtt,
            OutputFormat::Ass => Self::Ass,
            OutputFormat::Json => Self::Json,
            OutputFormat::Txt => Self::Txt,
        }
    }
}

mod config_show;
mod translate_paths;
// Pure-data classifier + extension formatter for `submate transcribe`.
// `cmd_transcribe`/`collect_media_files` carry their own glob-based collection,
// so this module is currently only exercised by its parity tests. `#[allow]`,
// not `#[expect]`: the module is dead only in a non-test build (the parity test
// uses it), so the expectation would be unfulfilled under `--all-targets`.
#[allow(dead_code)]
mod transcribe_collect;

/// AI-powered subtitle generation using Whisper.
///
/// The global `--config-file` is a `.env`/`.toml`/JSON file layered under the
/// `SUBMATE__` environment when resolving [`Config`].
#[derive(Debug, Parser)]
#[command(name = "submate", version, about, long_about = None)]
struct Cli {
    /// Path to a configuration file (.env, .toml, or JSON).
    #[arg(short = 'c', long = "config-file", global = true, value_name = "PATH")]
    config_file: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

/// Logging knobs shared by the long-running and batch subcommands
/// (`--log-level` + `--log-file`).
#[derive(Debug, Args)]
struct LoggingOpts {
    /// Minimum log level to emit.
    #[arg(long, value_name = "LEVEL", default_value = "INFO")]
    log_level: String,

    /// Append logs to this file instead of stderr.
    #[arg(long, value_name = "PATH")]
    log_file: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Transcribe video or audio files to generate subtitles.
    Transcribe(TranscribeArgs),
    /// Translate subtitle files to a target language using an LLM backend.
    Translate(TranslateArgs),
    /// Run the Bazarr ASR provider HTTP server.
    Server(ServerArgs),
    /// List the audio tracks in a media file.
    Probe(ProbeArgs),
    /// Inspect and manage configuration.
    #[command(subcommand)]
    Config(ConfigCommand),
}

#[derive(Debug, Args)]
struct TranscribeArgs {
    /// File or directory to transcribe.
    path: PathBuf,

    /// Path to the Whisper model file (e.g. `ggml-base.en.bin`). Overrides the
    /// configured `whisper.model` and the `SUBMATE__WHISPER__MODEL` env var.
    #[arg(short = 'm', long, value_name = "PATH")]
    model: Option<PathBuf>,

    /// Path to a Silero VAD model (`ggml-silero-*.bin`). Enables speech-only
    /// transcription — skips silence/music, cutting hallucinated lines. Sets
    /// `SUBMATE__WHISPER__VAD_MODEL` for this run.
    #[arg(long, value_name = "PATH")]
    vad_model: Option<PathBuf>,

    /// Select the audio track. Accepts a language code (e.g. `ja`),
    /// `lang:<code>`, `track:<n>` (0-based), `default`, or `auto`.
    #[arg(short = 'a', long, value_name = "SELECTOR")]
    audio: Option<AudioSelector>,

    /// Deprecated alias for `--audio <code>`; selects a track by language code.
    #[arg(long, value_name = "CODE", hide = true)]
    audio_language: Option<String>,

    /// Whisper decode-language hint, independent of `--audio` track selection.
    /// An ISO code (e.g. `en`) forces that decode language; `auto` lets whisper
    /// auto-detect. When omitted, defaults to the selected track's language tag
    /// (or auto-detect if that track is untagged).
    #[arg(short = 'l', long, value_name = "CODE")]
    language: Option<String>,

    /// Translate the generated subtitles to this target language.
    #[arg(short = 't', long)]
    translate_to: Option<String>,

    /// LLM backend for `--translate-to` (overrides `translation.backend`):
    /// `ollama`, `claude`, `openai`, or `gemini`.
    #[arg(long, value_parser = parse_backend, value_name = "BACKEND")]
    backend: Option<submate_types::TranslationBackend>,

    /// Subtitle output format. Defaults to `srt` (no behavior change for
    /// existing usage).
    #[arg(short = 'F', long, value_enum, default_value_t = OutputFormat::Srt)]
    format: OutputFormat,

    // Whisper decoding knobs. Each overrides `SUBMATE__WHISPER__*` and leaves
    // whisper.cpp's own default in place when omitted.
    /// Prompt text biasing the decoder's vocabulary/spelling.
    #[arg(long, value_name = "TEXT")]
    initial_prompt: Option<String>,

    /// Beam-search width; omit for greedy decoding.
    #[arg(long, value_name = "N")]
    beam_size: Option<u32>,

    /// Sampling temperature.
    #[arg(long, value_name = "T")]
    temperature: Option<f32>,

    /// No-speech probability above which a segment is treated as silence.
    #[arg(long, value_name = "T")]
    no_speech_threshold: Option<f32>,

    /// Entropy threshold for the decoder's temperature fallback.
    #[arg(long, value_name = "T")]
    entropy_threshold: Option<f32>,

    /// Average-log-probability threshold below which a decode is rejected.
    #[arg(long, value_name = "T")]
    logprob_threshold: Option<f32>,

    /// Maximum characters per segment (caps subtitle line length).
    #[arg(long, value_name = "N")]
    max_len: Option<u32>,

    /// Overwrite existing subtitle files.
    #[arg(short = 'f', long)]
    force: bool,

    /// Process subdirectories recursively.
    #[arg(short = 'r', long)]
    recursive: bool,

    /// Stop immediately on the first error.
    #[arg(long)]
    fail_fast: bool,

    /// Never prompt for an ambiguous audio-track choice; take the deterministic
    /// rule pick (first match / track 0) instead. Implied automatically off a
    /// TTY (pipe, batch, server), so this only matters interactively.
    #[arg(long, visible_alias = "yes")]
    non_interactive: bool,

    #[command(flatten)]
    logging: LoggingOpts,
}

#[derive(Debug, Args)]
struct TranslateArgs {
    /// Subtitle file or directory to translate.
    path: PathBuf,

    /// Source language; `auto` detects it from the filename.
    #[arg(short = 's', long, default_value = "auto")]
    source_lang: String,

    /// Target language code (e.g. `es`, `fr`, `de`).
    #[arg(short = 't', long)]
    target_lang: String,

    /// Output file path (defaults to `input.{target}.srt`).
    #[arg(short = 'o', long)]
    output: Option<PathBuf>,

    /// LLM backend to translate with (overrides `translation.backend`):
    /// `ollama`, `claude`, `openai`, or `gemini`.
    #[arg(long, value_parser = parse_backend, value_name = "BACKEND")]
    backend: Option<submate_types::TranslationBackend>,

    /// Process directories recursively.
    #[arg(short = 'r', long)]
    recursive: bool,

    /// Overwrite existing output files.
    #[arg(short = 'f', long)]
    force: bool,

    /// Minimum log level to emit.
    #[arg(long, value_name = "LEVEL", default_value = "INFO")]
    log_level: String,
}

#[derive(Debug, Args)]
struct ServerArgs {
    /// Host/address to bind to.
    #[arg(short = 'H', long)]
    host: Option<String>,

    /// Port to listen on (defaults to the configured `server.port`).
    #[arg(short = 'p', long)]
    port: Option<u16>,

    /// Path to a Silero VAD model (sets `SUBMATE__WHISPER__VAD_MODEL` for all
    /// transcription on this server).
    #[arg(long, value_name = "PATH")]
    vad_model: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct ProbeArgs {
    /// Media file to inspect.
    path: PathBuf,
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    /// Print the resolved configuration.
    Show,
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(cli) {
        eprintln!("submate: {err:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Config(ConfigCommand::Show) => cmd_config_show(cli.config_file.as_deref()),
        Command::Translate(args) => {
            init_logging(&args.log_level, None);
            cmd_translate(cli.config_file.as_deref(), args)
        }
        Command::Transcribe(args) => {
            init_logging(&args.logging.log_level, args.logging.log_file.as_deref());
            cmd_transcribe(cli.config_file.as_deref(), args)
        }
        Command::Server(args) => cmd_server(cli.config_file.as_deref(), args),
        Command::Probe(args) => cmd_probe(args),
    }
}

/// Configure `tracing-subscriber` from a `--log-level` string.
///
/// `RUST_LOG` (an `EnvFilter` directive) wins when set, matching the
/// conventional escape hatch; otherwise the level string seeds the filter. The
/// `log_file` argument is accepted for surface parity with the `--log-file`
/// flag; file sinks are not yet wired, so logs go to stderr regardless.
fn init_logging(log_level: &str, _log_file: Option<&Path>) {
    use tracing_subscriber::filter::EnvFilter;

    let level = log_level.to_lowercase();
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // whisper.cpp's internal logs are routed through the
        // `whisper_rs::whisper_sys_tracing` target — verbose model-load/buffer
        // lines that shouldn't clutter normal output. Keep them out unless the
        // user explicitly asks for debug/trace (or sets RUST_LOG).
        let base = EnvFilter::new(&level);
        if level == "debug" || level == "trace" {
            base
        } else {
            base.add_directive("whisper_rs=warn".parse().expect("static directive"))
        }
    });

    // `try_init` so a double-initialization (e.g. in tests) is a no-op rather
    // than a panic.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

/// Resolve the layered [`Config`], turning figment errors into `anyhow`.
fn load_config(config_file: Option<&Path>) -> anyhow::Result<Config> {
    Config::from_env(config_file).map_err(|e| anyhow::anyhow!("failed to load config: {e}"))
}

/// clap value parser for `--backend`. `TranslationBackend` lives in a pure crate
/// with no clap dependency, so the name → variant mapping lives here.
fn parse_backend(s: &str) -> Result<submate_types::TranslationBackend, String> {
    use submate_types::TranslationBackend as B;
    match s.to_lowercase().as_str() {
        "ollama" => Ok(B::Ollama),
        "claude" | "anthropic" => Ok(B::Claude),
        "openai" => Ok(B::Openai),
        "gemini" => Ok(B::Gemini),
        other => Err(format!(
            "unknown backend {other:?} (expected one of: ollama, claude, openai, gemini)"
        )),
    }
}

/// Apply a `--vad-model` flag by setting the env var the whisper crate reads
/// (`whisper_vad_model()`), so the flag takes precedence over any inherited
/// `SUBMATE__WHISPER__VAD_MODEL`. Called at the top of a command, before any
/// worker thread or runtime starts, so the single-threaded `set_var` is sound.
fn apply_vad_model(vad_model: Option<&Path>) {
    if let Some(path) = vad_model {
        // SAFETY: called at the very top of a command handler, before any tokio
        // runtime or worker thread is spawned, so no other thread can be reading
        // the environment concurrently (the soundness condition for `set_var`).
        unsafe { std::env::set_var("SUBMATE__WHISPER__VAD_MODEL", path) };
    }
}

/// `submate config show` — print the resolved configuration as a table of
/// flattened, title-cased rows.
///
/// The row set and ordering come from [`config_show::config_show_rows`]; the
/// `serde_json` `preserve_order` feature keeps the serialized `Config`'s object
/// keys in field-declaration order so the rows match the golden.
fn cmd_config_show(config_file: Option<&Path>) -> anyhow::Result<()> {
    let config = load_config(config_file)?;
    let json = serde_json::to_value(&config)?;
    let rows = config_show::config_show_rows(&json);

    let width = rows.iter().map(|(name, _)| name.len()).max().unwrap_or(0);
    println!("Submate Configuration");
    for (name, value) in rows {
        println!("{name:width$}  {value}");
    }
    Ok(())
}

/// `submate translate` — translate subtitle files to a target language.
///
/// File selection, source-language detection, and default output naming reuse
/// the pure helpers in [`translate_paths`]; the per-file IO and the backend
/// dispatch live here.
fn cmd_translate(config_file: Option<&Path>, args: TranslateArgs) -> anyhow::Result<()> {
    let mut config = load_config(config_file)?;
    if let Some(backend) = args.backend {
        config.translation.backend = backend;
    }

    let files = find_subtitle_files(&args.path, args.recursive);
    if files.is_empty() {
        anyhow::bail!("no subtitle files found in {}", args.path.display());
    }
    if args.output.is_some() && files.len() != 1 {
        anyhow::bail!("--output can only be used with a single input file");
    }

    let backend = build_backend(&config);
    let chunk_size = config.translation.chunk_size as usize;

    // The translation stack is async (the backends `.await` their reqwest
    // client); this standalone path has no ambient runtime, so drive each
    // translate on a local runtime, mirroring `cmd_transcribe`.
    let runtime = tokio::runtime::Runtime::new()?;

    for file in &files {
        let output_path = match (&args.output, files.len()) {
            (Some(out), 1) => out.clone(),
            _ => translate_paths::output_path(file, &args.target_lang),
        };

        if output_path.exists() && !args.force {
            println!(
                "Skipping {} - output exists (use -f to overwrite)",
                file.display()
            );
            continue;
        }

        println!(
            "Translating {} -> {}",
            file.file_name().and_then(|n| n.to_str()).unwrap_or(""),
            args.target_lang
        );

        let content = std::fs::read_to_string(file)?;
        let source = translate_paths::detect_source_language(file, &args.source_lang);
        let suffix = file
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e.to_lowercase()))
            .unwrap_or_default();

        let mut complete =
            async |prompt: String| backend.complete(&prompt).await.map_err(anyhow::Error::from);
        let translated = runtime.block_on(async {
            let result: anyhow::Result<String> = match suffix.as_str() {
                ".ass" | ".ssa" => {
                    // The portable ASS path translates extracted dialogue lines;
                    // with no ASS (de)serializer wired here it operates on the
                    // whole file as a single block, mirroring the SRT path's
                    // content round-trip.
                    let lines = vec![content.clone()];
                    let out = submate_translate::translate_ass_dialogue(
                        &lines,
                        &source,
                        &args.target_lang,
                        chunk_size,
                        &mut complete,
                    )
                    .await?;
                    Ok(out.into_iter().next().unwrap_or(content))
                }
                ".vtt" => Ok(submate_translate::translate_vtt_content(
                    &content,
                    &source,
                    &args.target_lang,
                    chunk_size,
                    &mut complete,
                )
                .await?),
                _ => Ok(submate_translate::translate_srt_content(
                    &content,
                    &source,
                    &args.target_lang,
                    chunk_size,
                    &mut complete,
                )
                .await?),
            };
            result
        })?;

        std::fs::write(&output_path, translated)?;
        println!(
            "Saved {}",
            output_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
        );
    }

    Ok(())
}

/// Build the translation backend selected by `config.translation.backend`.
fn build_backend(config: &Config) -> Box<dyn submate_translate::Backend + Send + Sync> {
    let t = &config.translation;
    submate_translate::make_backend(&submate_translate::BackendSettings {
        backend: t.backend,
        ollama_model: &t.ollama_model,
        ollama_url: &t.ollama_url,
        anthropic_api_key: &t.anthropic_api_key,
        claude_model: &t.claude_model,
        openai_api_key: &t.openai_api_key,
        openai_model: &t.openai_model,
        openai_base_url: &t.openai_base_url,
        gemini_api_key: &t.gemini_api_key,
        gemini_model: &t.gemini_model,
    })
}

/// Map the `[stable_ts]` config into the whisper assembly knobs. A
/// `custom_regroup` of `false`/empty disables regrouping (empty algo string).
fn assemble_options(s: &submate_config::StableTsSettings) -> submate_whisper::AssembleOptions {
    use submate_config::StrOrBool;
    submate_whisper::AssembleOptions {
        regroup_algo: match &s.custom_regroup {
            StrOrBool::Str(pattern) => pattern.clone(),
            StrOrBool::Bool(_) => String::new(),
        },
        suppress_silence: s.suppress_silence,
        min_word_duration: s.min_word_duration,
    }
}

/// Render `assembled` in `format`, then LLM-translate to `target_language` when
/// one is given and differs from the `detected` source. Shared by the CLI
/// transcribe path and the Bazarr server path. A translation error degrades to
/// the untranslated content (`translate_content` absorbs it).
#[cfg(feature = "model")]
async fn render_subtitle(
    assembled: &submate_whisper::Transcription,
    format: submate_types::OutputFormat,
    word_level: bool,
    detected: &str,
    target_language: Option<&str>,
    backend: Option<&std::sync::Arc<Box<dyn submate_translate::Backend + Send + Sync>>>,
    chunk_size: usize,
) -> String {
    let mut content = assembled.render(format, word_level);
    if let (Some(target), Some(backend)) = (target_language.filter(|t| !t.is_empty()), backend)
        && target != detected
    {
        let backend = backend.clone();
        let mut complete = move |prompt: String| {
            let backend = backend.clone();
            async move { backend.complete(&prompt).await }
        };
        content = submate_translate::translate_content(
            &content,
            detected,
            target,
            format,
            chunk_size,
            &mut complete,
        )
        .await;
    }
    content
}

/// Resolve where a `transcribe` result is written next to its input.
///
/// A plain transcribe targets `<stem>.<ext>` (the format's extension replacing
/// the media extension): `movie.mkv` + SRT → `movie.srt`. When translating, the
/// output is language-suffixed so it never collides with a source-language
/// subtitle: `movie.mkv` + SRT + `es` → `movie.es.srt`.
///
/// The extension always follows the chosen output format, not the input's
/// extension (the input is a media file, not a subtitle).
fn transcribe_output_path(file: &Path, format: OutputFormat, target_lang: Option<&str>) -> PathBuf {
    let ext = format.extension().trim_start_matches('.');
    let stem = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let name = match target_lang {
        Some(lang) => format!("{stem}.{lang}.{ext}"),
        None => format!("{stem}.{ext}"),
    };
    match file.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(name),
        _ => PathBuf::from(name),
    }
}

/// Collect subtitle files under `path`.
fn find_subtitle_files(path: &Path, recursive: bool) -> Vec<PathBuf> {
    if path.is_file() {
        return if translate_paths::is_subtitle_file(path) {
            vec![path.to_path_buf()]
        } else {
            Vec::new()
        };
    }
    let mut out = Vec::new();
    collect_files(path, recursive, &mut |p| {
        if translate_paths::is_subtitle_file(p) {
            out.push(p.to_path_buf());
        }
    });
    out.sort();
    out
}

/// Walk `dir` (one level, or recursively), calling `visit` for each regular
/// file. Errors reading a directory are swallowed, silently skipping unreadable
/// entries.
fn collect_files(dir: &Path, recursive: bool, visit: &mut dyn FnMut(&Path)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if recursive {
                collect_files(&path, recursive, visit);
            }
        } else if path.is_file() {
            visit(&path);
        }
    }
}

/// `submate transcribe` — transcribe media files in-process.
///
/// Extracts each file's audio, transcribes it through the shared [`Dispatcher`]
/// (capped at `server.concurrent_transcriptions`), assembles the subtitle, and
/// writes it next to the input — optionally LLM-translating with `--translate-to`.
fn cmd_transcribe(config_file: Option<&Path>, args: TranscribeArgs) -> anyhow::Result<()> {
    apply_vad_model(args.vad_model.as_deref());
    let mut config = load_config(config_file)?;
    if let Some(backend) = args.backend {
        config.translation.backend = backend;
    }

    let files = collect_media_files(&args.path, args.recursive)?;
    if files.is_empty() {
        println!("No supported media files found");
        return Ok(());
    }
    println!("Found {} media file(s) to process", files.len());

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(transcribe_files(&config, &args, &files))
}

/// Collect media (video/audio) files under `path` using the shared extension
/// checks. A single non-media file path is an "unsupported file type" error.
fn collect_media_files(path: &Path, recursive: bool) -> anyhow::Result<Vec<PathBuf>> {
    let is_media = |p: &Path| {
        let s = p.to_string_lossy();
        submate_paths::is_video_file(&s) || submate_paths::is_audio_file(&s)
    };

    if path.is_file() {
        if is_media(path) {
            return Ok(vec![path.to_path_buf()]);
        }
        anyhow::bail!("unsupported file type: {}", path.display());
    }
    if !path.is_dir() {
        anyhow::bail!("path does not exist: {}", path.display());
    }

    let mut out = Vec::new();
    collect_files(path, recursive, &mut |p| {
        if is_media(p) {
            out.push(p.to_path_buf());
        }
    });
    out.sort();
    Ok(out)
}

/// Resolve the Whisper model file path used to transcribe.
///
/// Resolution order, highest priority first:
/// 1. the `--model <PATH>` flag,
/// 2. the configured `whisper.model`, *only when it names an existing path*
///    (the config value is otherwise a free-form size string like `medium`),
/// 3. the `SUBMATE__WHISPER__MODEL` environment variable.
///
/// When nothing resolves to a usable path, this returns a non-panicking
/// `Result::Err` naming the flag, the env var, and a download hint, so the CLI
/// fails with an actionable message instead of an obscure model-load panic.
fn resolve_model(flag: Option<&Path>, config_model: &str) -> anyhow::Result<PathBuf> {
    if let Some(path) = flag {
        return Ok(path.to_path_buf());
    }

    let config_path = Path::new(config_model);
    if config_path.exists() {
        return Ok(config_path.to_path_buf());
    }

    if let Some(env_model) = std::env::var_os("SUBMATE__WHISPER__MODEL")
        && !env_model.is_empty()
    {
        return Ok(PathBuf::from(env_model));
    }

    anyhow::bail!(
        "no Whisper model configured: pass --model <PATH>, set the whisper.model \
         config to a model file path, or export SUBMATE__WHISPER__MODEL. Download \
         one with e.g. ggml-base.en.bin from \
         https://huggingface.co/ggerganov/whisper.cpp"
    )
}

/// Serialize an [`AudioSelector`] back to its canonical wire string so it can
/// flow through the existing `Option<String>` job plumbing and be re-parsed by
/// `prepare_audio_for_transcription` on the processing node.
fn audio_selector_to_string(sel: &AudioSelector) -> String {
    match sel {
        AudioSelector::Lang(code) => format!("lang:{code}"),
        AudioSelector::Index(n) => format!("track:{n}"),
        AudioSelector::Default => "default".to_string(),
        AudioSelector::Auto => "auto".to_string(),
    }
}

/// Async core of `transcribe`: transcribe every file in-process through the
/// shared [`Dispatcher`], writing each subtitle next to its input.
async fn transcribe_files(
    config: &Config,
    args: &TranscribeArgs,
    files: &[PathBuf],
) -> anyhow::Result<()> {
    // Resolve the model path up front (an actionable error rather than a
    // model-load panic when nothing is configured).
    let model_path = resolve_model(args.model.as_deref(), &config.whisper.model)?;

    // The Dispatcher caps how many files transcribe at once (a batch/recursive
    // run shares the runner count); a single file uses one runner.
    let dispatcher =
        submate_whisper::Dispatcher::new(config.server.concurrent_transcriptions.max(1) as usize);

    // Post-decode assembly knobs from `[stable_ts]`.
    let assemble = assemble_options(&config.stable_ts);

    // The LLM backend is built once and shared (Arc) across files; only needed
    // when `--translate-to` is given.
    let backend: Option<std::sync::Arc<Box<dyn submate_translate::Backend + Send + Sync>>> = args
        .translate_to
        .as_ref()
        .map(|_| std::sync::Arc::new(build_backend(config)));

    // `--audio` is the typed selector; `--audio-language` is a hidden deprecated
    // alias that maps to `Lang(..)`. Prefer `--audio` when both are given. The
    // selector flows to `prepare_audio_for_transcription` as a string; the
    // whisper decode-language hint is resolved separately per file below.
    let mut selector: Option<AudioSelector> = match (&args.audio, &args.audio_language) {
        (Some(sel), _) => Some(sel.clone()),
        (None, Some(lang)) => {
            tracing::warn!("--audio-language is deprecated; use --audio <code> (or lang:<code>)");
            Some(AudioSelector::Lang(lang.clone()))
        }
        (None, None) => None,
    };
    let mut selector_str = selector.as_ref().map(audio_selector_to_string);

    // Interactive track picker — single file only. Multi-file / recursive runs
    // always take the deterministic rule (we never block a batch on a prompt),
    // and the prompt is further gated on stderr being a TTY. Resolving here pins
    // the chosen track via a `track:<n>` selector.
    if files.len() == 1 {
        let file = &files[0];
        let tracks = submate_media::get_audio_tracks(file)
            .await
            .unwrap_or_default();
        let is_tty = std::io::stderr().is_terminal();
        match resolve_single_file_track(
            &tracks,
            selector.as_ref(),
            file,
            is_tty,
            args.non_interactive,
        ) {
            Ok(Some(index)) => {
                selector = Some(AudioSelector::Index(index));
                selector_str = selector.as_ref().map(audio_selector_to_string);
            }
            Ok(None) => {}
            Err(e) => return Err(e),
        }
    }

    let mut failed = 0usize;
    for file in files {
        // The decode-language hint is independent of track selection: an
        // explicit `--language` wins; otherwise it defaults to the selected
        // track's language tag (a probe failure degrades to auto-detect).
        let decode_language = if args.language.is_some() {
            submate_media::resolve_decode_language(&[], selector.as_ref(), args.language.as_deref())
        } else {
            let tracks = submate_media::get_audio_tracks(file)
                .await
                .unwrap_or_default();
            submate_media::resolve_decode_language(&tracks, selector.as_ref(), None)
        };

        // whisper always transcribes in the source language; `--translate-to`
        // is an LLM step applied to the rendered subtitle below.
        let options = submate_whisper::TranscribeOptions {
            language: decode_language,
            task: submate_whisper::Task::Transcribe,
            // CLI flags override the `SUBMATE__WHISPER__*` config defaults.
            initial_prompt: args
                .initial_prompt
                .clone()
                .or_else(|| config.whisper.initial_prompt.clone()),
            beam_size: args.beam_size.or(config.whisper.beam_size),
            temperature: args.temperature.or(config.whisper.temperature),
            no_speech_threshold: args
                .no_speech_threshold
                .or(config.whisper.no_speech_threshold),
            entropy_threshold: args.entropy_threshold.or(config.whisper.entropy_threshold),
            logprob_threshold: args.logprob_threshold.or(config.whisper.logprob_threshold),
            max_len: args.max_len.or(config.whisper.max_len),
        };

        let result = transcribe_one(
            &dispatcher,
            &model_path,
            file,
            selector_str.as_deref(),
            options,
            args.format,
            args.translate_to.as_deref(),
            backend.clone(),
            config.translation.chunk_size,
            &assemble,
            config.stable_ts.word_level_highlight,
        )
        .await;

        match result {
            Ok(content) => {
                // A plain transcribe targets `movie.<ext>`; when translating, the
                // output is language-suffixed so it never overwrites the source.
                let out_path =
                    transcribe_output_path(file, args.format, args.translate_to.as_deref());
                let (count, noun) = output_count(&content, args.format);
                std::fs::write(&out_path, &content)
                    .map_err(|e| anyhow::anyhow!("failed to write {}: {e}", out_path.display()))?;
                println!("{}", result_summary(file, &out_path, count, noun));
            }
            Err(e) => {
                failed += 1;
                println!("  Failed: {} ({e})", file.display());
                if args.fail_fast {
                    break;
                }
            }
        }
    }

    if failed > 0 {
        anyhow::bail!("{failed} file(s) failed to process");
    }
    Ok(())
}

/// Transcribe one media file in-process: extract the selected track's PCM,
/// run whisper under the dispatcher's runner cap, assemble the subtitle, and
/// LLM-translate it when `translate_to` differs from the detected language.
#[cfg(feature = "model")]
#[expect(clippy::too_many_arguments)]
async fn transcribe_one(
    dispatcher: &submate_whisper::Dispatcher,
    model_path: &Path,
    file: &Path,
    selector: Option<&str>,
    options: submate_whisper::TranscribeOptions,
    format: OutputFormat,
    translate_to: Option<&str>,
    backend: Option<std::sync::Arc<Box<dyn submate_translate::Backend + Send + Sync>>>,
    chunk_size: u32,
    assemble: &submate_whisper::AssembleOptions,
    word_level: bool,
) -> anyhow::Result<String> {
    use submate_media::{
        PreparedAudio, extract_audio_track_to_memory, prepare_audio_for_transcription,
    };

    // Extract the selected audio track to mono 16 kHz f32 PCM. The samples are
    // shared (Arc) with the assembly stage rather than deep-copied.
    let pcm: std::sync::Arc<[f32]> = match prepare_audio_for_transcription(file, selector).await {
        PreparedAudio::Pcm(bytes) => submate_bazarr::pcm_s16le_to_f32(&bytes),
        PreparedAudio::Path(path) => {
            let bytes = extract_audio_track_to_memory(&path, 0)
                .await
                .map_err(|e| anyhow::anyhow!("audio extraction failed: {e}"))?;
            submate_bazarr::pcm_s16le_to_f32(&bytes)
        }
    }
    .into();

    let raw = dispatcher
        .transcribe_pcm(
            model_path.to_string_lossy().to_string(),
            pcm.clone(),
            options,
        )
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let assembled = submate_whisper::assemble_result(&raw, assemble, &pcm)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    Ok(render_subtitle(
        &assembled,
        format.into(),
        word_level,
        &raw.language,
        translate_to,
        backend.as_ref(),
        chunk_size.max(1) as usize,
    )
    .await)
}

/// Without the `model` feature there is no whisper.cpp to run.
#[cfg(not(feature = "model"))]
#[expect(clippy::too_many_arguments)]
async fn transcribe_one(
    _dispatcher: &submate_whisper::Dispatcher,
    _model_path: &Path,
    _file: &Path,
    _selector: Option<&str>,
    _options: submate_whisper::TranscribeOptions,
    _format: OutputFormat,
    _translate_to: Option<&str>,
    _backend: Option<std::sync::Arc<Box<dyn submate_translate::Backend + Send + Sync>>>,
    _chunk_size: u32,
    _assemble: &submate_whisper::AssembleOptions,
    _word_level: bool,
) -> anyhow::Result<String> {
    anyhow::bail!("model support not built in (rebuild with --features model)")
}

/// Count the entries in a produced output string, with the unit noun for the
/// summary line, per format: cues for the subtitle formats (srt/vtt/ass),
/// segments for JSON, lines for plain text. Keeps the summary honest instead of
/// reporting `0 cues` for every non-SRT format.
fn output_count(output: &str, format: OutputFormat) -> (usize, &'static str) {
    match format {
        OutputFormat::Srt => (submate_subtitle::cue::parse_srt(output).len(), "cue"),
        OutputFormat::Vtt => (output.matches("-->").count(), "cue"),
        OutputFormat::Ass => (
            output
                .lines()
                .filter(|l| l.starts_with("Dialogue:"))
                .count(),
            "cue",
        ),
        OutputFormat::Json => (
            serde_json::from_str::<serde_json::Value>(output)
                .ok()
                .as_ref()
                .and_then(|v| v.get("segments"))
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len),
            "segment",
        ),
        OutputFormat::Txt => (
            output.lines().filter(|l| !l.trim().is_empty()).count(),
            "line",
        ),
    }
}

/// Format the one-line success summary for a transcribed file, e.g.
/// `✓ movie.mkv → movie.srt (42 cues)`. `noun` is the singular unit (`cue`,
/// `segment`, `line`), pluralized with a trailing `s` when `count != 1`.
///
/// Only the file names (not full paths) are shown so the line stays readable
/// when transcribing inside a deeply nested directory. A path with no final
/// component (e.g. `/`) falls back to its `display()` form so the summary is
/// never empty.
fn result_summary(input: &Path, output: &Path, count: usize, noun: &str) -> String {
    let name = |p: &Path| {
        p.file_name()
            .and_then(|n| n.to_str())
            .map_or_else(|| p.display().to_string(), str::to_owned)
    };
    let unit = if count == 1 {
        noun.to_string()
    } else {
        format!("{noun}s")
    };
    format!("✓ {} → {} ({count} {unit})", name(input), name(output))
}

/// Short, human display name for a path (final component, falling back to the
/// full `display()` form for paths with no final component).
fn display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .map_or_else(|| path.display().to_string(), str::to_owned)
}

/// `submate server` — run the Bazarr ASR provider HTTP server.
fn cmd_server(config_file: Option<&Path>, args: ServerArgs) -> anyhow::Result<()> {
    use submate_server::{AppState, app};

    apply_vad_model(args.vad_model.as_deref());
    let config = load_config(config_file)?;
    init_logging(if config.debug { "DEBUG" } else { "INFO" }, None);

    let host = args.host.unwrap_or_else(|| config.server.address.clone());
    let port = args.port.unwrap_or(config.server.port);

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind((host.as_str(), port)).await?;
        let addr = listener.local_addr()?;
        tracing::info!("submate server listening on {addr}");

        // One Dispatcher caps concurrent Bazarr transcriptions to the configured
        // runner count.
        let dispatcher = submate_whisper::Dispatcher::new(
            config.server.concurrent_transcriptions.max(1) as usize,
        );
        let mut state = AppState::default();
        if let Some(bazarr) = build_bazarr_transcriber(dispatcher, &config)? {
            state = state.with_bazarr(bazarr);
        }
        let router = app(state);
        axum::serve(listener, router).await?;
        Ok::<(), anyhow::Error>(())
    })
}

/// Production [`BazarrTranscriber`]: the real whisper + translate pipeline over
/// the shared [`Dispatcher`], so concurrent Bazarr requests share a runner cap.
/// Model-gated; without the feature [`build_bazarr_transcriber`] returns `None`
/// and the routes degrade gracefully.
#[cfg(feature = "model")]
struct WhisperBazarrTranscriber {
    dispatcher: submate_whisper::Dispatcher,
    model_path: String,
    backend: std::sync::Arc<Box<dyn submate_translate::Backend + Send + Sync>>,
    chunk_size: usize,
    /// Decode knobs from `SUBMATE__WHISPER__*`, applied to every Bazarr request.
    /// `language`/`task` are placeholders overridden per call.
    decode: submate_whisper::TranscribeOptions,
    /// Post-decode assembly knobs from `[stable_ts]`.
    assemble: submate_whisper::AssembleOptions,
}

#[cfg(feature = "model")]
#[async_trait::async_trait]
impl submate_server::BazarrTranscriber for WhisperBazarrTranscriber {
    async fn transcribe(
        &self,
        opts: submate_server::BazarrTranscribeOpts,
        pcm: Vec<u8>,
    ) -> Result<submate_server::BazarrOutput, String> {
        let samples: std::sync::Arc<[f32]> = submate_bazarr::pcm_s16le_to_f32(&pcm).into();
        let task = match opts.task {
            submate_types::TranscriptionTask::Translate => submate_whisper::Task::Translate,
            submate_types::TranscriptionTask::Transcribe => submate_whisper::Task::Transcribe,
        };
        // Source language is always auto-detected; Bazarr's `language` param is
        // the translation target, applied below.
        // Decode knobs come from config (`self.decode`); task is per-request.
        let options = submate_whisper::TranscribeOptions {
            task,
            ..self.decode.clone()
        };
        let raw = self
            .dispatcher
            .transcribe_pcm(self.model_path.clone(), samples.clone(), options)
            .await
            .map_err(|e| e.to_string())?;
        let assembled = submate_whisper::assemble_result(&raw, &self.assemble, &samples)
            .map_err(|e| e.to_string())?;
        let content = render_subtitle(
            &assembled,
            opts.output_format,
            opts.word_timestamps,
            &raw.language,
            opts.target_language.as_deref(),
            Some(&self.backend),
            self.chunk_size,
        )
        .await;

        Ok(submate_server::BazarrOutput {
            content,
            detected_language: raw.language,
        })
    }

    async fn detect(&self, pcm: Vec<u8>) -> Result<submate_server::BazarrDetected, String> {
        // Language id only needs the first ~30 s (Whisper's first mel window).
        let mut samples = submate_bazarr::pcm_s16le_to_f32(&pcm);
        samples.truncate(16_000 * 30);
        let options = self.decode.clone();
        let raw = self
            .dispatcher
            .transcribe_pcm(self.model_path.clone(), samples.into(), options)
            .await
            .map_err(|e| e.to_string())?;
        let detected = submate_bazarr::detect_language(Some(&raw.language));
        Ok(submate_server::BazarrDetected {
            detected_language: detected.detected_language.to_string(),
            language_code: detected.language_code,
        })
    }
}

/// Build the production Bazarr transcriber over `dispatcher`. `None` without the
/// `model` feature — the `/bazarr/*` routes then degrade gracefully.
#[cfg(feature = "model")]
fn build_bazarr_transcriber(
    dispatcher: submate_whisper::Dispatcher,
    config: &Config,
) -> anyhow::Result<Option<std::sync::Arc<dyn submate_server::BazarrTranscriber>>> {
    Ok(Some(std::sync::Arc::new(WhisperBazarrTranscriber {
        dispatcher,
        model_path: config.whisper.model.clone(),
        backend: std::sync::Arc::new(build_backend(config)),
        chunk_size: config.translation.chunk_size.max(1) as usize,
        decode: submate_whisper::TranscribeOptions {
            language: None,
            task: submate_whisper::Task::Transcribe,
            initial_prompt: config.whisper.initial_prompt.clone(),
            beam_size: config.whisper.beam_size,
            temperature: config.whisper.temperature,
            no_speech_threshold: config.whisper.no_speech_threshold,
            entropy_threshold: config.whisper.entropy_threshold,
            logprob_threshold: config.whisper.logprob_threshold,
            max_len: config.whisper.max_len,
        },
        assemble: assemble_options(&config.stable_ts),
    })))
}

#[cfg(not(feature = "model"))]
fn build_bazarr_transcriber(
    _dispatcher: submate_whisper::Dispatcher,
    _config: &Config,
) -> anyhow::Result<Option<std::sync::Arc<dyn submate_server::BazarrTranscriber>>> {
    Ok(None)
}

/// `submate probe <file>` — list the file's audio tracks.
///
/// A thin IO wrapper: it runs `ffprobe` via [`submate_media::get_audio_tracks`]
/// and prints whatever [`render_track_table`] formats. All the layout logic
/// lives in that pure renderer so it is unit-testable without invoking ffprobe.
fn cmd_probe(args: ProbeArgs) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let tracks = runtime
        .block_on(submate_media::get_audio_tracks(&args.path))
        .map_err(|e| anyhow::anyhow!("failed to probe {}: {e}", args.path.display()))?;
    println!("{}", render_track_table(&tracks, &args.path));
    Ok(())
}

/// Render the audio-track listing for a probed file as a plain multi-line
/// string.
///
/// Pure function of the probed tracks (and the file name, for the header) so it
/// is unit-testable without running ffprobe, and reusable by the future
/// interactive picker. Each track line shows its 0-based audio-stream index,
/// language tag, codec, and `title` when present; the container's default track
/// is marked with a trailing `(default)`. An empty track list yields a single
/// "no audio tracks" line.
fn render_track_table(tracks: &[AudioTrack], path: &Path) -> String {
    let name = display_name(path);

    if tracks.is_empty() {
        return format!("No audio tracks in {name}");
    }

    let noun = if tracks.len() == 1 { "track" } else { "tracks" };
    let mut out = format!("{} audio {noun} in {name}:", tracks.len());

    // Pad the codec column so titles line up; the language tag is already a
    // fixed-ish width (ISO 639), so a single space there reads cleanly.
    let codec_width = tracks.iter().map(|t| t.codec.len()).max().unwrap_or(0);

    for track in tracks {
        out.push_str(&format!(
            "\n  #{idx}  {lang:<3}  {codec:<codec_width$}",
            idx = track.index,
            lang = track.language,
            codec = track.codec,
        ));
        if let Some(title) = &track.title {
            out.push_str(&format!("  {title}"));
        }
        if track.default {
            out.push_str("  (default)");
        }
    }

    out
}

/// The outcome of classifying an audio-track selection for a single file.
///
/// Produced by the pure [`decide_track`]; the IO layer turns each variant into
/// the right behaviour — `Resolved` runs straight through, `Prompt` renders the
/// candidates and reads a numbered choice, `Error` aborts.
#[derive(Debug, Clone, PartialEq, Eq)]
enum TrackDecision {
    /// The selection is unambiguous (or was forced by the rule): use this
    /// [`AudioTrack::index`].
    Resolved(usize),
    /// The selection is ambiguous and a human can answer: prompt them to pick
    /// among these candidate [`AudioTrack::index`]es.
    Prompt(Vec<usize>),
    /// The selector could not be resolved at all (e.g. no language match, index
    /// out of range, no tracks). Carries a user-facing message.
    Error(String),
}

/// Decide which audio track to transcribe, as a pure function of the inputs.
///
/// The stdin read / table render is thin IO layered around this; keeping the
/// decision pure makes the "three contexts, one mental model" contract directly
/// unit-testable:
/// - An unambiguously resolving selector (a single track, `Auto`/`Default` with
///   a clear pick, an `Index`, or a `Lang` matching exactly one track) →
///   [`TrackDecision::Resolved`], regardless of `is_tty`.
/// - Ambiguous **and** `is_tty` **and** `!non_interactive` →
///   [`TrackDecision::Prompt`] with the candidate indices.
/// - Ambiguous **and** (off a TTY **or** `--non-interactive`) →
///   [`TrackDecision::Resolved`] with the deterministic rule pick (first match /
///   track 0). Callers note the pick and how to override; this never blocks.
///
/// "Ambiguous" means either a `Lang` selector matched more than one track, or no
/// selector was given and there are several tracks with no default disposition.
/// The candidate tracks a human would choose among when the selection is
/// ambiguous, as their [`AudioTrack::index`]es. Empty means *unambiguous*.
///
/// Ambiguity is exactly the spec's two cases: a `Lang` selector that matched
/// more than one track (→ the tracks of that language), or no selector with
/// several tracks and no default disposition (→ every track). Every other
/// selector — `Index`, `Default`, `Auto`, a single-track file, a `Lang` with a
/// unique match — is unambiguous and yields an empty set.
fn ambiguous_candidates(tracks: &[AudioTrack], sel: Option<&AudioSelector>) -> Vec<usize> {
    use submate_media::{AudioSelector, lang_match_is_ambiguous};

    match sel {
        Some(s @ AudioSelector::Lang(code)) if lang_match_is_ambiguous(tracks, s) => {
            let wanted = code.to_lowercase();
            tracks
                .iter()
                .filter(|t| t.language.to_lowercase() == wanted)
                .map(|t| t.index)
                .collect()
        }
        None if tracks.len() > 1 && !tracks.iter().any(|t| t.default) => {
            tracks.iter().map(|t| t.index).collect()
        }
        _ => Vec::new(),
    }
}

fn decide_track(
    tracks: &[AudioTrack],
    sel: Option<&AudioSelector>,
    is_tty: bool,
    non_interactive: bool,
) -> TrackDecision {
    use submate_media::{AudioSelector, resolve_audio_selector};

    if tracks.is_empty() {
        return TrackDecision::Error("no audio tracks available".to_string());
    }

    let candidates = ambiguous_candidates(tracks, sel);

    if candidates.is_empty() {
        // Unambiguous: resolve through the shared selector rules. A `None`
        // selector with no ambiguity behaves like `Auto` (single track, or a
        // clear default).
        let owned;
        let resolved = match sel {
            Some(s) => s,
            None => {
                owned = AudioSelector::Auto;
                &owned
            }
        };
        return match resolve_audio_selector(tracks, resolved) {
            Ok(index) => TrackDecision::Resolved(index),
            Err(e) => TrackDecision::Error(e.to_string()),
        };
    }

    // Ambiguous. Prompt only when a human can actually answer.
    if is_tty && !non_interactive {
        TrackDecision::Prompt(candidates)
    } else {
        // Deterministic rule pick: the first candidate (first language match, or
        // track 0 when there is no selector).
        TrackDecision::Resolved(candidates[0])
    }
}

/// Thin IO around [`decide_track`] for the single-file transcribe path.
///
/// Returns the chosen [`AudioTrack::index`] to pin (`Some`), or `None` when the
/// selector can resolve downstream on its own (no tracks probed, or nothing to
/// override). `Err` only on a hard selector failure (e.g. an out-of-range index
/// or an unmatched language), which should abort the run.
///
/// On `Prompt` it renders the candidate tracks via [`render_track_table`] and
/// reads a numbered choice from stdin; on the rule path it logs a one-line note
/// naming the pick and how to override.
fn resolve_single_file_track(
    tracks: &[AudioTrack],
    sel: Option<&AudioSelector>,
    path: &Path,
    is_tty: bool,
    non_interactive: bool,
) -> anyhow::Result<Option<usize>> {
    // A probe failure (or a track-less file) leaves the downstream resolver to
    // degrade to auto-detect; nothing to pin here.
    if tracks.is_empty() {
        return Ok(None);
    }

    match decide_track(tracks, sel, is_tty, non_interactive) {
        TrackDecision::Resolved(index) => {
            // When the selection was ambiguous but we took the rule path (off a
            // TTY or `--non-interactive`), say which track we picked and how to
            // override, so the choice is never silent.
            if !ambiguous_candidates(tracks, sel).is_empty() {
                let lang = tracks
                    .iter()
                    .find(|t| t.index == index)
                    .map_or("unknown", |t| t.language.as_str());
                eprintln!(
                    "Ambiguous audio selection; using track #{index} ({lang}). \
                     Override with -a track:<n> (or -a <lang>), or run interactively \
                     without --non-interactive."
                );
            }
            Ok(Some(index))
        }
        TrackDecision::Prompt(candidates) => prompt_for_track(tracks, &candidates, path),
        TrackDecision::Error(msg) => Err(anyhow::anyhow!(msg)),
    }
}

/// Render the candidate tracks and read a 0-based track index from stdin.
///
/// Split from [`resolve_single_file_track`] so the decision stays pure; this is
/// the only part that touches stdin/stderr. An EOF or unreadable choice (e.g.
/// stdin closed mid-prompt) falls back to the first candidate rather than
/// aborting, so a half-interactive pipe still makes progress.
fn prompt_for_track(
    tracks: &[AudioTrack],
    candidates: &[usize],
    path: &Path,
) -> anyhow::Result<Option<usize>> {
    use std::io::Write;

    let shown: Vec<AudioTrack> = tracks
        .iter()
        .filter(|t| candidates.contains(&t.index))
        .cloned()
        .collect();

    eprintln!("Multiple audio tracks match; pick one:");
    eprintln!("{}", render_track_table(&shown, path));
    eprint!("Track index [{}]: ", candidates[0]);
    let _ = std::io::stderr().flush();

    let mut line = String::new();
    let read = std::io::stdin().read_line(&mut line)?;
    let choice = line.trim();
    if read == 0 || choice.is_empty() {
        // EOF or empty input → accept the default (first candidate).
        return Ok(Some(candidates[0]));
    }
    match choice.parse::<usize>() {
        Ok(index) if candidates.contains(&index) => Ok(Some(index)),
        _ => anyhow::bail!(
            "'{choice}' is not one of the offered track indices ({})",
            candidates
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

#[cfg(test)]
mod cli {
    use super::*;
    use clap::CommandFactory;

    /// Falsifier: the `--sync` write path language-suffixes the output only when
    /// translating. With `--translate-to es` the SRT result lands at
    /// `movie.es.srt`; a plain transcribe still targets `movie.srt`. The
    /// extension follows the chosen format regardless of the media extension.
    #[test]
    fn translate_output_path() {
        let file = Path::new("/media/movie.mkv");

        assert_eq!(
            transcribe_output_path(file, OutputFormat::Srt, Some("es")),
            PathBuf::from("/media/movie.es.srt"),
        );
        assert_eq!(
            transcribe_output_path(file, OutputFormat::Srt, None),
            PathBuf::from("/media/movie.srt"),
        );
        // The format extension wins over the input extension, suffixed per lang.
        assert_eq!(
            transcribe_output_path(file, OutputFormat::Vtt, Some("ja")),
            PathBuf::from("/media/movie.ja.vtt"),
        );
    }

    /// The clap definition is internally consistent (no overlapping flags, valid
    /// arg specs). `debug_assert` is clap's own structural validator.
    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    /// The subcommand surface is the expected set of user-facing commands; the
    /// distributed `node` command is gone with the queue.
    #[test]
    fn cli_help_subcommands() {
        let cmd = Cli::command();
        let names: Vec<&str> = cmd.get_subcommands().map(clap::Command::get_name).collect();

        for expected in ["transcribe", "translate", "server", "probe", "config"] {
            assert!(names.contains(&expected), "missing subcommand `{expected}`");
        }
        for gone in ["node", "worker"] {
            assert!(!names.contains(&gone), "`{gone}` must not be a subcommand");
        }
    }

    /// The global `--config-file` flag is present and global, and the
    /// subcommands carry their distinguishing flags.
    #[test]
    fn cli_help_flags() {
        let cmd = Cli::command();

        let has_config_file = cmd
            .get_arguments()
            .any(|a| a.get_long() == Some("config-file"));
        assert!(has_config_file, "global --config-file flag missing");

        let sub = |name: &str| {
            cmd.get_subcommands()
                .find(|c| c.get_name() == name)
                .unwrap_or_else(|| panic!("subcommand `{name}` missing"))
                .clone()
        };

        assert!(
            sub("transcribe")
                .get_arguments()
                .any(|a| a.get_long() == Some("initial-prompt")),
            "`transcribe` must expose --initial-prompt"
        );
        assert!(
            sub("translate")
                .get_arguments()
                .any(|a| a.get_long() == Some("target-lang")),
            "`translate` must expose --target-lang"
        );
    }

    /// `transcribe --model <PATH>` parses, and `resolve_model` returns the
    /// documented `Result::Err` (naming the flag, the env var, and the download
    /// hint) instead of panicking when nothing is configured.
    #[test]
    fn transcribe_model_resolution() {
        // The flag parses and reaches `TranscribeArgs.model`.
        let cli = Cli::try_parse_from([
            "submate",
            "transcribe",
            "--model",
            "/models/ggml-base.en.bin",
            "movie.mkv",
        ])
        .expect("--model should parse");
        let Command::Transcribe(args) = cli.command else {
            panic!("expected the transcribe subcommand");
        };
        assert_eq!(
            args.model.as_deref(),
            Some(Path::new("/models/ggml-base.en.bin"))
        );

        // The flag is the highest-priority source and is returned verbatim,
        // without touching the filesystem or the environment.
        let resolved =
            resolve_model(args.model.as_deref(), "medium").expect("--model should resolve");
        assert_eq!(resolved, PathBuf::from("/models/ggml-base.en.bin"));

        // With no flag, a non-path config value, and no env var, the resolver
        // returns an actionable error (not a panic) naming both knobs and the
        // download hint. Clear `SUBMATE__*` (incl. `SUBMATE__WHISPER__MODEL`)
        // under the shared env lock so this never races other env-driven tests
        // (e.g. `config_show`'s resolution); the guard restores on drop.
        let _env = ::fixtures::EnvGuard::set(&[]);
        let err = resolve_model(None, "medium")
            .expect_err("missing model must be an Err, not a panic")
            .to_string();

        assert!(err.contains("--model"), "error must name the flag: {err}");
        assert!(
            err.contains("SUBMATE__WHISPER__MODEL"),
            "error must name the env var: {err}"
        );
        assert!(
            err.contains("ggml-base.en.bin")
                && err.contains("huggingface.co/ggerganov/whisper.cpp"),
            "error must include the download hint: {err}"
        );
    }

    /// Parse `transcribe` and return its `--format` value.
    fn parse_format(args: &[&str]) -> OutputFormat {
        let mut argv = vec!["submate", "transcribe"];
        argv.extend_from_slice(args);
        argv.push("movie.mkv");
        let cli = Cli::try_parse_from(argv)
            .unwrap_or_else(|e| panic!("`transcribe {args:?}` should parse: {e}"));
        let Command::Transcribe(t) = cli.command else {
            panic!("expected the transcribe subcommand");
        };
        t.format
    }

    /// Each `OutputFormat` variant maps to its dotted file extension and to the
    /// matching proto serializer selector (`From` conversion).
    #[test]
    fn output_format_extension_mapping() {
        for (fmt, ext, proto) in [
            (OutputFormat::Srt, ".srt", submate_types::OutputFormat::Srt),
            (OutputFormat::Vtt, ".vtt", submate_types::OutputFormat::Vtt),
            (OutputFormat::Ass, ".ass", submate_types::OutputFormat::Ass),
            (
                OutputFormat::Json,
                ".json",
                submate_types::OutputFormat::Json,
            ),
            (OutputFormat::Txt, ".txt", submate_types::OutputFormat::Txt),
        ] {
            assert_eq!(fmt.extension(), ext, "wrong extension for {fmt:?}");
            assert_eq!(
                submate_types::OutputFormat::from(fmt),
                proto,
                "wrong proto selector for {fmt:?}"
            );
        }
    }

    /// Omitting `--format` yields `srt`/`.srt` — no behavior change for existing
    /// usage, matching the hardcoded SRT path it replaces.
    #[test]
    fn output_format_default_is_srt() {
        let fmt = parse_format(&[]);
        assert_eq!(fmt, OutputFormat::Srt);
        assert_eq!(fmt.extension(), ".srt");
    }

    /// `-F`/`--format` parse to the right variant; an unknown value is rejected.
    #[test]
    fn output_format_parses_and_rejects_invalid() {
        assert_eq!(parse_format(&["-F", "ass"]), OutputFormat::Ass);
        assert_eq!(parse_format(&["--format", "json"]), OutputFormat::Json);
        assert_eq!(parse_format(&["-F", "vtt"]), OutputFormat::Vtt);
        assert_eq!(parse_format(&["--format", "txt"]), OutputFormat::Txt);

        let err = Cli::try_parse_from(["submate", "transcribe", "-F", "bogus", "movie.mkv"]);
        assert!(err.is_err(), "an invalid --format value must be rejected");
    }

    fn parse_audio(arg: &str) -> AudioSelector {
        let cli = Cli::try_parse_from(["submate", "transcribe", "-a", arg, "movie.mkv"])
            .unwrap_or_else(|e| panic!("`-a {arg}` should parse: {e}"));
        let Command::Transcribe(args) = cli.command else {
            panic!("expected the transcribe subcommand");
        };
        args.audio.expect("-a should populate `audio`")
    }

    /// `-a` parses each grammar form into the right [`AudioSelector`] via clap's
    /// `FromStr` plumbing, and a malformed value is rejected.
    #[test]
    fn audio_selector_flag_parses_grammar() {
        assert_eq!(parse_audio("ja"), AudioSelector::Lang("ja".to_string()));
        assert_eq!(
            parse_audio("lang:ja"),
            AudioSelector::Lang("ja".to_string())
        );
        assert_eq!(parse_audio("track:2"), AudioSelector::Index(2));
        assert_eq!(parse_audio("default"), AudioSelector::Default);
        assert_eq!(parse_audio("auto"), AudioSelector::Auto);

        assert!(
            Cli::try_parse_from(["submate", "transcribe", "-a", "track:abc", "movie.mkv"]).is_err(),
            "a malformed selector must be rejected at parse time"
        );
    }

    /// `--audio-language` stays as a hidden deprecated alias mapping to `Lang`,
    /// and the canonical string round-trips back through `FromStr`.
    #[test]
    fn audio_language_alias_and_canonical_string() {
        let cli = Cli::try_parse_from([
            "submate",
            "transcribe",
            "--audio-language",
            "fr",
            "movie.mkv",
        ])
        .expect("--audio-language alias should still parse");
        let Command::Transcribe(args) = cli.command else {
            panic!("expected the transcribe subcommand");
        };
        assert_eq!(args.audio, None);
        assert_eq!(args.audio_language.as_deref(), Some("fr"));

        // The hidden alias is not advertised in help output.
        let hidden = Cli::command()
            .get_subcommands()
            .find(|c| c.get_name() == "transcribe")
            .expect("transcribe subcommand")
            .get_arguments()
            .find(|a| a.get_long() == Some("audio-language"))
            .expect("audio-language arg exists")
            .is_hide_set();
        assert!(hidden, "--audio-language must be hidden");

        for sel in [
            AudioSelector::Lang("ja".to_string()),
            AudioSelector::Index(3),
            AudioSelector::Default,
            AudioSelector::Auto,
        ] {
            let s = audio_selector_to_string(&sel);
            assert_eq!(s.parse::<AudioSelector>().unwrap(), sel);
        }
    }

    /// Parse a `transcribe` invocation and return its [`TranscribeArgs`].
    fn transcribe_args(tokens: &[&str]) -> TranscribeArgs {
        let mut argv = vec!["submate", "transcribe"];
        argv.extend_from_slice(tokens);
        argv.push("movie.mkv");
        let cli = Cli::try_parse_from(argv).unwrap_or_else(|e| panic!("should parse: {e}"));
        let Command::Transcribe(args) = cli.command else {
            panic!("expected the transcribe subcommand");
        };
        args
    }

    /// A small fixed track list standing in for a probed file: index 0 tagged
    /// `eng`, index 1 tagged `jpn`, index 2 untagged (`und`).
    fn sample_tracks() -> Vec<AudioTrack> {
        vec![
            AudioTrack {
                index: 0,
                language: "eng".to_string(),
                codec: "aac".to_string(),
                default: false,
                title: None,
            },
            AudioTrack {
                index: 1,
                language: "jpn".to_string(),
                codec: "ac3".to_string(),
                default: false,
                title: None,
            },
            AudioTrack {
                index: 2,
                language: "und".to_string(),
                codec: "dts".to_string(),
                default: false,
                title: None,
            },
        ]
    }

    /// Mirror `transcribe_files`' selector resolution: `--audio` wins, else the
    /// deprecated `--audio-language` maps to `Lang`, else `None`.
    fn selector_for(args: &TranscribeArgs) -> Option<AudioSelector> {
        match (&args.audio, &args.audio_language) {
            (Some(sel), _) => Some(sel.clone()),
            (None, Some(lang)) => Some(AudioSelector::Lang(lang.clone())),
            (None, None) => None,
        }
    }

    /// `--audio track:2 --language en` → selector `Index(2)`, decode `Some("en")`:
    /// the explicit flag is the decode hint, the selector is untouched.
    #[test]
    fn decode_language_resolution_explicit_flag() {
        let args = transcribe_args(&["--audio", "track:2", "--language", "en"]);
        let selector = selector_for(&args);
        assert_eq!(selector, Some(AudioSelector::Index(2)));

        let decode = submate_media::resolve_decode_language(
            &sample_tracks(),
            selector.as_ref(),
            args.language.as_deref(),
        );
        assert_eq!(decode, Some("en".to_string()));
    }

    /// `--audio ja` with no `--language` → decode defaults to the selected
    /// track's tag. (`-a jpn` selects the JA track; its tag seeds the hint.)
    #[test]
    fn decode_language_resolution_defaults_to_track_tag() {
        let args = transcribe_args(&["--audio", "jpn"]);
        let selector = selector_for(&args);
        assert_eq!(selector, Some(AudioSelector::Lang("jpn".to_string())));
        assert!(args.language.is_none());

        let decode = submate_media::resolve_decode_language(
            &sample_tracks(),
            selector.as_ref(),
            args.language.as_deref(),
        );
        assert_eq!(decode, Some("jpn".to_string()));
    }

    /// `--audio track:1 --language auto` → decode `None` (whisper auto-detects),
    /// even though track 1 is tagged.
    #[test]
    fn decode_language_resolution_auto_flag() {
        let args = transcribe_args(&["--audio", "track:1", "--language", "auto"]);
        let selector = selector_for(&args);
        assert_eq!(selector, Some(AudioSelector::Index(1)));

        let decode = submate_media::resolve_decode_language(
            &sample_tracks(),
            selector.as_ref(),
            args.language.as_deref(),
        );
        assert_eq!(decode, None);
    }

    /// Selecting an untagged track with no `--language` → decode `None`.
    #[test]
    fn decode_language_resolution_untagged_track() {
        let args = transcribe_args(&["--audio", "track:2"]);
        let selector = selector_for(&args);
        assert_eq!(selector, Some(AudioSelector::Index(2)));

        let decode = submate_media::resolve_decode_language(
            &sample_tracks(),
            selector.as_ref(),
            args.language.as_deref(),
        );
        assert_eq!(decode, None);
    }

    /// The resolved (selector, decode-language) pair varies independently:
    /// holding the selector fixed, the decode hint changes solely with
    /// `--language`.
    #[test]
    fn decode_language_resolution_independent_of_selector() {
        let tracks = sample_tracks();

        let base = transcribe_args(&["--audio", "track:1"]);
        let sel = selector_for(&base);
        // Default: inherits track 1's tag.
        assert_eq!(
            submate_media::resolve_decode_language(&tracks, sel.as_ref(), None),
            Some("jpn".to_string()),
        );

        let forced = transcribe_args(&["--audio", "track:1", "--language", "en"]);
        assert_eq!(selector_for(&forced), sel);
        assert_eq!(
            submate_media::resolve_decode_language(
                &tracks,
                sel.as_ref(),
                forced.language.as_deref()
            ),
            Some("en".to_string()),
        );

        let auto = transcribe_args(&["--audio", "track:1", "--language", "auto"]);
        assert_eq!(selector_for(&auto), sel);
        assert_eq!(
            submate_media::resolve_decode_language(&tracks, sel.as_ref(), auto.language.as_deref()),
            None,
        );
    }

    /// The success summary formatter renders just the file names plus the
    /// count, pluralizing the unit noun, regardless of how deep the input path
    /// is or which format-specific noun is used.
    #[test]
    fn result_summary_format() {
        assert_eq!(
            result_summary(
                Path::new("/media/movies/movie.mkv"),
                Path::new("/media/movies/movie.srt"),
                42,
                "cue",
            ),
            "✓ movie.mkv → movie.srt (42 cues)",
        );
        assert_eq!(
            result_summary(Path::new("clip.mp4"), Path::new("clip.srt"), 1, "cue"),
            "✓ clip.mp4 → clip.srt (1 cue)",
        );
        assert_eq!(
            result_summary(Path::new("clip.mp4"), Path::new("clip.json"), 1, "segment"),
            "✓ clip.mp4 → clip.json (1 segment)",
        );
        assert_eq!(
            result_summary(Path::new("clip.mp4"), Path::new("clip.txt"), 4, "line"),
            "✓ clip.mp4 → clip.txt (4 lines)",
        );
    }

    /// `output_count` reports the right entry count and unit noun per format,
    /// instead of `0 cues` for everything that is not SRT.
    #[test]
    fn output_count_per_format() {
        let srt =
            "1\n00:00:00,000 --> 00:00:01,000\nhi\n\n2\n00:00:01,000 --> 00:00:02,000\nthere\n";
        assert_eq!(output_count(srt, OutputFormat::Srt), (2, "cue"));

        let vtt = "WEBVTT\n\n00:00:00.000 --> 00:00:01.000\nhi\n";
        assert_eq!(output_count(vtt, OutputFormat::Vtt), (1, "cue"));

        let ass = "[Events]\nDialogue: 0,0:00:0.00,0:00:1.00,Default,,0,0,0,,a\nDialogue: 0,0:00:1.00,0:00:2.00,Default,,0,0,0,,b\n";
        assert_eq!(output_count(ass, OutputFormat::Ass), (2, "cue"));

        let json = r#"{"text":"x","segments":[{"start":0.0,"end":1.0,"text":"x"}]}"#;
        assert_eq!(output_count(json, OutputFormat::Json), (1, "segment"));

        let txt = "line one\nline two\n\nline three\n";
        assert_eq!(output_count(txt, OutputFormat::Txt), (3, "line"));
    }

    /// `render_track_table` lists every track's index/language/codec/title and
    /// marks exactly the default track, without invoking ffprobe.
    #[test]
    fn probe_table_renders_tracks() {
        let tracks = vec![
            AudioTrack {
                index: 0,
                language: "jpn".to_string(),
                codec: "ac3".to_string(),
                default: true,
                title: Some("Main".to_string()),
            },
            AudioTrack {
                index: 1,
                language: "eng".to_string(),
                codec: "aac".to_string(),
                default: false,
                title: None,
            },
            AudioTrack {
                index: 2,
                language: "und".to_string(),
                codec: "ac3".to_string(),
                default: false,
                title: Some("Commentary".to_string()),
            },
        ];

        let out = render_track_table(&tracks, Path::new("/media/movie.mkv"));

        // Header reports the count and the bare file name.
        assert!(
            out.contains("3 audio tracks in movie.mkv:"),
            "header: {out}"
        );

        // Each track's index, language and codec is listed.
        for track in &tracks {
            assert!(
                out.contains(&format!("#{}", track.index)),
                "missing index #{}: {out}",
                track.index
            );
            assert!(
                out.contains(&track.language),
                "missing language {}: {out}",
                track.language
            );
            assert!(
                out.contains(&track.codec),
                "missing codec {}: {out}",
                track.codec
            );
        }

        // Titles are shown when present.
        assert!(out.contains("Main"), "missing title `Main`: {out}");
        assert!(
            out.contains("Commentary"),
            "missing title `Commentary`: {out}"
        );

        // Exactly the default track (index 0) is marked.
        assert_eq!(
            out.matches("(default)").count(),
            1,
            "exactly one default marker expected: {out}"
        );
        let default_line = out
            .lines()
            .find(|l| l.contains("(default)"))
            .expect("a line marked default");
        assert!(
            default_line.contains("#0"),
            "the marked default must be track #0: {default_line}"
        );

        // An empty track list degrades to a single no-tracks line.
        let empty = render_track_table(&[], Path::new("clip.mp4"));
        assert_eq!(empty, "No audio tracks in clip.mp4");
    }

    /// `--non-interactive` parses and carries its `--yes` visible alias.
    #[test]
    fn transcribe_non_interactive_flag() {
        let cmd = Cli::command();
        let sub = cmd
            .get_subcommands()
            .find(|c| c.get_name() == "transcribe")
            .expect("transcribe subcommand");
        let arg = sub
            .get_arguments()
            .find(|a| a.get_long() == Some("non-interactive"))
            .expect("--non-interactive flag");
        let aliases = arg.get_visible_aliases().unwrap_or_default();
        assert!(aliases.contains(&"yes"), "missing --yes alias: {aliases:?}");

        for flag in ["--non-interactive", "--yes"] {
            let args = transcribe_args(&[flag]);
            assert!(args.non_interactive, "`{flag}` must set non_interactive");
        }
        // Default is off.
        assert!(!transcribe_args(&[]).non_interactive);
    }

    /// Two `eng` tracks + one `jpn`, none default — a `Lang("eng")` selector is
    /// ambiguous; the no-selector case is ambiguous too (no default flagged).
    fn ambiguous_tracks() -> Vec<AudioTrack> {
        vec![
            AudioTrack {
                index: 0,
                language: "eng".to_string(),
                codec: "aac".to_string(),
                default: false,
                title: Some("Main".to_string()),
            },
            AudioTrack {
                index: 1,
                language: "eng".to_string(),
                codec: "ac3".to_string(),
                default: false,
                title: Some("Commentary".to_string()),
            },
            AudioTrack {
                index: 2,
                language: "jpn".to_string(),
                codec: "dts".to_string(),
                default: false,
                title: None,
            },
        ]
    }

    /// An unambiguous selector resolves to the same index regardless of `is_tty`
    /// or `--non-interactive`: a single-language `Lang`, an `Index`, `Default`,
    /// and a single-track `Auto` all go straight to `Resolved`.
    #[test]
    fn decide_track_unambiguous_resolves_regardless_of_tty() {
        let tracks = sample_tracks(); // eng / jpn / und, all unique langs.

        let cases: &[(Option<AudioSelector>, usize)] = &[
            (Some(AudioSelector::Lang("jpn".into())), 1),
            (Some(AudioSelector::Index(2)), 2),
            (Some(AudioSelector::Auto), 0),
            (Some(AudioSelector::Default), 0),
        ];

        for (sel, expected) in cases {
            for is_tty in [false, true] {
                for non_interactive in [false, true] {
                    assert_eq!(
                        decide_track(&tracks, sel.as_ref(), is_tty, non_interactive),
                        TrackDecision::Resolved(*expected),
                        "sel={sel:?} is_tty={is_tty} non_interactive={non_interactive}",
                    );
                }
            }
        }

        // A lone track is unambiguous even with no selector.
        let one = vec![sample_tracks().remove(0)];
        for is_tty in [false, true] {
            assert_eq!(
                decide_track(&one, None, is_tty, false),
                TrackDecision::Resolved(0),
            );
        }
    }

    /// Ambiguous + TTY + interactive → `Prompt` with the candidate indices, for
    /// both ambiguity flavours (a multi-match `Lang`, and no selector with no
    /// default track).
    #[test]
    fn decide_track_ambiguous_tty_interactive_prompts() {
        let tracks = ambiguous_tracks();

        // `Lang("eng")` matches tracks 0 and 1.
        assert_eq!(
            decide_track(
                &tracks,
                Some(&AudioSelector::Lang("eng".into())),
                true,
                false
            ),
            TrackDecision::Prompt(vec![0, 1]),
        );

        // No selector, several tracks, no default → every track is a candidate.
        assert_eq!(
            decide_track(&tracks, None, true, false),
            TrackDecision::Prompt(vec![0, 1, 2]),
        );
    }

    /// Ambiguous but off a TTY → the deterministic rule pick (first match /
    /// track 0), never a prompt, even when interactive would have asked.
    #[test]
    fn decide_track_ambiguous_off_tty_takes_rule() {
        let tracks = ambiguous_tracks();

        assert_eq!(
            decide_track(
                &tracks,
                Some(&AudioSelector::Lang("eng".into())),
                false,
                false
            ),
            TrackDecision::Resolved(0),
        );
        assert_eq!(
            decide_track(&tracks, None, false, false),
            TrackDecision::Resolved(0),
        );
    }

    /// Ambiguous + TTY + `--non-interactive` → the rule pick, not a prompt: the
    /// flag forces the deterministic path even with a human present.
    #[test]
    fn decide_track_ambiguous_non_interactive_takes_rule() {
        let tracks = ambiguous_tracks();

        assert_eq!(
            decide_track(
                &tracks,
                Some(&AudioSelector::Lang("eng".into())),
                true,
                true
            ),
            TrackDecision::Resolved(0),
        );
        assert_eq!(
            decide_track(&tracks, None, true, true),
            TrackDecision::Resolved(0),
        );
    }

    /// No tracks at all → `Error`, and an unresolvable selector (bad index /
    /// unmatched language) also surfaces as `Error` rather than a silent pick.
    #[test]
    fn decide_track_error_paths() {
        assert!(matches!(
            decide_track(&[], None, true, false),
            TrackDecision::Error(_)
        ));
        assert!(matches!(
            decide_track(
                &sample_tracks(),
                Some(&AudioSelector::Index(9)),
                true,
                false
            ),
            TrackDecision::Error(_)
        ));
        assert!(matches!(
            decide_track(
                &sample_tracks(),
                Some(&AudioSelector::Lang("zzz".into())),
                true,
                false
            ),
            TrackDecision::Error(_)
        ));
    }
}
