//! `submate` binary — clap CLI (ports `submate/cli/`).
//!
//! Wires the server + node binaries together behind the user-facing
//! subcommands. The Python Click group (`submate/cli/main.py`) exposed
//! `transcribe / translate / worker / server / config`; the Rust port keeps the
//! same surface but replaces the standalone `worker` with `node`:
//!
//! * `submate server` runs the coordinator (axum) with an *embedded* processing
//!   node by default, so a single-box deployment needs no separate worker.
//! * `submate node --server <url>` runs a remote processing node that pulls work
//!   from a coordinator over HTTP — the multi-box analogue of the old worker.
//! * `submate transcribe --sync` spins up a one-shot local coordinator + node in
//!   the same process and drains exactly the enqueued jobs before returning,
//!   matching the Python "process immediately, no worker required" path.
//!
//! Pure sub-helpers that decide *which* files to process and *how* the
//! `config show` table is laid out live in their own byte-for-byte-ported
//! modules ([`config_show`], [`translate_paths`], [`transcribe_collect`]); this
//! file is the clap wiring and the IO around them.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::{Args, Parser, Subcommand};
use submate_config::Config;

mod config_show;
mod translate_paths;
// Pure-data classifier + extension formatter for `submate transcribe`, ported
// ahead of the IO wiring. `cmd_transcribe`/`collect_media_files` still carry
// their own glob-based collection; `port-cli-commands` swaps them onto these
// byte-for-byte-ported helpers. Allowed dead until then so the parity tests
// (the item's falsifier) build and run.
#[allow(dead_code)]
mod transcribe_collect;

/// AI-powered subtitle generation using Whisper.
///
/// The global `--config-file` mirrors the Python `-c/--config-file` group
/// option: a `.env`/`.toml`/JSON file layered under the `SUBMATE__` environment
/// when resolving [`Config`].
#[derive(Debug, Parser)]
#[command(name = "submate", version, about, long_about = None)]
struct Cli {
    /// Path to a configuration file (.env, .toml, or JSON).
    #[arg(short = 'c', long = "config-file", global = true, value_name = "PATH")]
    config_file: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

/// Logging knobs shared by the long-running and batch subcommands (ports the
/// Click `logging_options` decorator: `--log-level` + `--log-file`).
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
    /// Run the coordinator server (with an embedded processing node by default).
    Server(ServerArgs),
    /// Run a processing node that pulls work from a coordinator.
    Node(NodeArgs),
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

    /// Select the audio track by language code (e.g. `ja` for a Japanese dub).
    #[arg(short = 'a', long)]
    audio_language: Option<String>,

    /// Translate the generated subtitles to this target language.
    #[arg(short = 't', long)]
    translate_to: Option<String>,

    /// Overwrite existing subtitle files.
    #[arg(short = 'f', long)]
    force: bool,

    /// Process subdirectories recursively.
    #[arg(short = 'r', long)]
    recursive: bool,

    /// Stop immediately on the first error.
    #[arg(long)]
    fail_fast: bool,

    /// Process files immediately in-process instead of queueing them.
    #[arg(long)]
    sync: bool,

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
}

#[derive(Debug, Args)]
struct NodeArgs {
    /// Coordinator base URL to pull work from (e.g. `http://submate:9000`).
    #[arg(short = 's', long, value_name = "URL")]
    server: String,

    /// Maximum concurrent transcriptions this node will run.
    #[arg(short = 'w', long, default_value_t = 2)]
    runners: u32,

    /// Advertise a usable GPU to the coordinator.
    #[arg(long)]
    gpu: bool,

    #[command(flatten)]
    logging: LoggingOpts,
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
        Command::Node(args) => {
            init_logging(&args.logging.log_level, args.logging.log_file.as_deref());
            cmd_node(cli.config_file.as_deref(), args)
        }
    }
}

/// Configure `tracing-subscriber` from a `--log-level` string.
///
/// `RUST_LOG` (an `EnvFilter` directive) wins when set, matching the
/// conventional escape hatch; otherwise the level string seeds the filter. The
/// `log_file` argument is accepted for surface parity with the Python
/// `--log-file` flag; file sinks are out of scope for this wiring, so logs go to
/// stderr regardless.
fn init_logging(log_level: &str, _log_file: Option<&Path>) {
    use tracing_subscriber::filter::EnvFilter;

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level.to_lowercase()));

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

/// `submate config show` — print the resolved configuration as a table of
/// flattened, title-cased rows.
///
/// The row set and ordering come from [`config_show::config_show_rows`], the
/// byte-for-byte port of the Python `_flatten_settings`/`_format_value` layer;
/// the `serde_json` `preserve_order` feature keeps the serialized `Config`'s
/// object keys in Pydantic field-declaration order so the rows match the golden.
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
/// the ported pure helpers in [`translate_paths`]; the per-file IO and the
/// backend dispatch live here.
fn cmd_translate(config_file: Option<&Path>, args: TranslateArgs) -> anyhow::Result<()> {
    let config = load_config(config_file)?;

    let files = find_subtitle_files(&args.path, args.recursive);
    if files.is_empty() {
        anyhow::bail!("no subtitle files found in {}", args.path.display());
    }
    if args.output.is_some() && files.len() != 1 {
        anyhow::bail!("--output can only be used with a single input file");
    }

    let backend = build_backend(&config);
    let chunk_size = config.translation.chunk_size as usize;

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
            |prompt: &str| backend.complete(prompt).map_err(anyhow::Error::from);
        let translated = match suffix.as_str() {
            ".ass" | ".ssa" => {
                // The portable ASS path translates extracted dialogue lines; with
                // no ASS (de)serializer wired here it operates on the whole file
                // as a single block, mirroring the SRT path's content round-trip.
                let lines = vec![content.clone()];
                let out = submate_translate::translate_ass_dialogue(
                    &lines,
                    &source,
                    &args.target_lang,
                    chunk_size,
                    &mut complete,
                )?;
                out.into_iter().next().unwrap_or(content)
            }
            ".vtt" => submate_translate::translate_vtt_content(
                &content,
                &source,
                &args.target_lang,
                chunk_size,
                &mut complete,
            )?,
            _ => submate_translate::translate_srt_content(
                &content,
                &source,
                &args.target_lang,
                chunk_size,
                &mut complete,
            )?,
        };

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
fn build_backend(config: &Config) -> Box<dyn submate_translate::Backend> {
    use submate_types::TranslationBackend;

    let t = &config.translation;
    match t.backend {
        TranslationBackend::Ollama => Box::new(submate_translate::OllamaBackend::new(
            t.ollama_model.clone(),
            t.ollama_url.clone(),
        )),
        TranslationBackend::Claude => Box::new(submate_translate::ClaudeBackend::new(
            t.anthropic_api_key.clone(),
            t.claude_model.clone(),
        )),
        TranslationBackend::Openai => Box::new(submate_translate::OpenAIBackend::new(
            t.openai_api_key.clone(),
            t.openai_model.clone(),
        )),
        TranslationBackend::Gemini => Box::new(submate_translate::GeminiBackend::new(
            t.gemini_api_key.clone(),
            t.gemini_model.clone(),
        )),
    }
}

/// Collect subtitle files under `path` (ports `find_subtitle_files`).
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
/// file. Errors reading a directory are swallowed, matching the glob-based
/// Python scan that silently skips unreadable entries.
fn collect_files(dir: &Path, recursive: bool, visit: &mut dyn FnMut(&Path)) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
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

/// `submate transcribe` — enqueue media files for transcription.
///
/// Without `--sync` this enqueues jobs into the durable store for a separately
/// running node to drain. With `--sync` it spins up an in-process coordinator +
/// embedded node, enqueues the files, and waits for each to finish before
/// returning — the one-shot local path that needs no standalone node.
fn cmd_transcribe(config_file: Option<&Path>, args: TranscribeArgs) -> anyhow::Result<()> {
    let config = load_config(config_file)?;

    let files = collect_media_files(&args.path, args.recursive)?;
    if files.is_empty() {
        println!("No supported media files found");
        return Ok(());
    }
    println!("Found {} media file(s) to process", files.len());

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(transcribe_files(&config, &args, &files))
}

/// Collect media (video/audio) files under `path` using the ported extension
/// checks. A single non-media file path is an error, matching Python's
/// "unsupported file type" abort.
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

    if let Some(env_model) = std::env::var_os("SUBMATE__WHISPER__MODEL") {
        if !env_model.is_empty() {
            return Ok(PathBuf::from(env_model));
        }
    }

    anyhow::bail!(
        "no Whisper model configured: pass --model <PATH>, set the whisper.model \
         config to a model file path, or export SUBMATE__WHISPER__MODEL. Download \
         one with e.g. ggml-base.en.bin from \
         https://huggingface.co/ggerganov/whisper.cpp"
    )
}

/// Async core of `transcribe`: enqueue every file, and in `--sync` mode run a
/// local coordinator + embedded node and wait for each job to complete.
async fn transcribe_files(
    config: &Config,
    args: &TranscribeArgs,
    files: &[PathBuf],
) -> anyhow::Result<()> {
    use submate_node::Dispatcher;
    use submate_proto::JobOpts;
    use submate_queue::JobStore;
    use submate_server::{
        spawn_embedded_node, AudioSource, EmbeddedNodeSettings, NodeCoordinator,
    };
    use submate_types::{Device, TranscriptionTask, WhisperModel};

    let store = if args.sync {
        // One-shot local run: an in-memory store is enough; nothing outlives it.
        JobStore::open_in_memory()
    } else {
        JobStore::open(&config.queue.db_path)
    }
    .map_err(|e| anyhow::anyhow!("failed to open job store: {e}"))?;

    let coord = Arc::new(NodeCoordinator::new(store));

    // `config.whisper.model` is a free-form size string ("medium", ...) while a
    // `JobOpts` carries the typed `WhisperModel`; an unrecognized size falls back
    // to the same default the config uses. `device` is already the typed enum.
    let model = config
        .whisper
        .model
        .parse::<WhisperModel>()
        .unwrap_or(WhisperModel::Medium);
    let device: Device = config.whisper.device;

    let task = if args.translate_to.is_some() {
        TranscriptionTask::Translate
    } else {
        TranscriptionTask::Transcribe
    };

    // In sync mode, bring up an embedded node draining only the jobs we enqueue.
    let node = if args.sync {
        let settings = EmbeddedNodeSettings {
            enabled: true,
            node_id: "local".into(),
            gpu: matches!(device, Device::Cuda),
            runners: config.server.concurrent_transcriptions.max(1),
            tasks: vec![TranscriptionTask::Transcribe, TranscriptionTask::Translate],
        };
        // A coordinator served over loopback is what the embedded node pulls
        // from; reuse the server crate's spawn helper with a real Whisper
        // processor sized to the same dispatcher. The model path is resolved
        // from `--model` > config > env, surfacing an actionable error rather
        // than a model-load panic when nothing is configured.
        let model_path = resolve_model(args.model.as_deref(), &config.whisper.model)?;
        let dispatcher = Dispatcher::new(settings.runners.max(1) as usize);
        let processor = make_processor(dispatcher, &model_path.to_string_lossy());
        let addr = serve_loopback(coord.clone()).await?;
        spawn_embedded_node(format!("http://{addr}"), &settings, processor)
    } else {
        None
    };

    let mut failed = 0usize;
    for file in files {
        let opts = JobOpts {
            model,
            device,
            source_language: args.audio_language.clone(),
            target_language: args.translate_to.clone(),
            translation_backend: None,
        };
        let source = AudioSource::File {
            path: file.clone(),
            language: args.audio_language.clone(),
        };
        let job_id = coord
            .enqueue_with_audio(task, &opts, source)
            .map_err(|e| anyhow::anyhow!("failed to enqueue {}: {e}", file.display()))?;

        if args.sync {
            match coord.wait_for_result(job_id).await {
                Some(submate_proto::JobOutcome::Ok { output }) => {
                    // Persist the produced subtitle next to the input. (The full
                    // skip-condition + language-suffixed naming lives in
                    // port-queue-transcription-service; this writes the result the
                    // sync coordinator already returns so `transcribe --sync`
                    // produces a file today.)
                    let out_path = file.with_extension("srt");
                    let cue_count = submate_subtitle::cue::parse_srt(&output).len();
                    std::fs::write(&out_path, output)
                        .map_err(|e| anyhow::anyhow!("failed to write {}: {e}", out_path.display()))?;
                    println!("{}", result_summary(file, &out_path, cue_count));
                }
                other => {
                    failed += 1;
                    println!("  Failed: {} ({other:?})", file.display());
                    if args.fail_fast {
                        break;
                    }
                }
            }
        } else {
            println!("  Queued: {}", file.display());
        }
    }

    if let Some(node) = node {
        node.abort();
    }

    if failed > 0 {
        anyhow::bail!("{failed} file(s) failed to process");
    }
    Ok(())
}

/// Format the one-line success summary for a transcribed file, e.g.
/// `✓ movie.mkv → movie.srt (42 cues)`.
///
/// Only the file names (not full paths) are shown so the line stays readable
/// when transcribing inside a deeply nested directory; the cue count is derived
/// by the caller from the written SRT. A path with no final component (e.g. `/`)
/// falls back to its `display()` form so the summary is never empty.
fn result_summary(input: &Path, output: &Path, cue_count: usize) -> String {
    let name = |p: &Path| {
        p.file_name()
            .and_then(|n| n.to_str())
            .map(str::to_owned)
            .unwrap_or_else(|| p.display().to_string())
    };
    let noun = if cue_count == 1 { "cue" } else { "cues" };
    format!(
        "✓ {} → {} ({cue_count} {noun})",
        name(input),
        name(output)
    )
}

/// Bind an ephemeral loopback port and serve the coordinator's router on it,
/// returning the bound address. Used by `--sync` so the embedded node has a
/// real coordinator URL to pull from.
async fn serve_loopback(
    coord: Arc<submate_server::NodeCoordinator>,
) -> anyhow::Result<std::net::SocketAddr> {
    use submate_server::{app, AppState};

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
    let addr = listener.local_addr()?;
    let router = app(AppState::with_coordinator(coord));
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Ok(addr)
}

/// `submate server` — run the coordinator with an embedded node by default.
fn cmd_server(config_file: Option<&Path>, args: ServerArgs) -> anyhow::Result<()> {
    use submate_queue::JobStore;
    use submate_server::{
        app, spawn_embedded_node, AppState, EmbeddedNodeSettings, NodeCoordinator,
    };

    let config = load_config(config_file)?;
    init_logging(if config.debug { "DEBUG" } else { "INFO" }, None);

    let host = args.host.unwrap_or_else(|| config.server.address.clone());
    let port = args.port.unwrap_or(config.server.port);

    let store = JobStore::open(&config.queue.db_path)
        .map_err(|e| anyhow::anyhow!("failed to open job store: {e}"))?;
    let coord = Arc::new(NodeCoordinator::new(store));

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        // Reclaim any leases left dangling by a previous crash before serving.
        if let Err(e) = coord.reclaim_stale_leases() {
            tracing::warn!("failed to reclaim stale leases: {e}");
        }

        let listener = tokio::net::TcpListener::bind((host.as_str(), port)).await?;
        let addr = listener.local_addr()?;
        tracing::info!("submate server listening on {addr}");

        // Embedded node: a single-box deployment processes its own queue.
        let node_settings = EmbeddedNodeSettings::from_server(&config.server);
        let _node = if node_settings.enabled {
            let base_url = format!("http://{addr}");
            let dispatcher =
                submate_node::Dispatcher::new(node_settings.runners.max(1) as usize);
            let processor = make_processor(dispatcher, &config.whisper.model);
            spawn_embedded_node(base_url, &node_settings, processor)
        } else {
            None
        };

        let router = app(AppState::with_coordinator(coord)
            .with_server_settings(config.server.clone()));
        axum::serve(listener, router).await?;
        Ok::<(), anyhow::Error>(())
    })
}

/// `submate node --server <url>` — run a processing node that pulls work from a
/// remote coordinator.
fn cmd_node(config_file: Option<&Path>, args: NodeArgs) -> anyhow::Result<()> {
    use submate_node::{Agent, Dispatcher};
    use submate_proto::NodeRegister;
    use submate_types::TranscriptionTask;

    let config = load_config(config_file)?;

    let register = NodeRegister {
        id: hostname_node_id(),
        gpu: args.gpu,
        runners: args.runners,
        tasks: vec![TranscriptionTask::Transcribe, TranscriptionTask::Translate],
    };

    let runners = args.runners.max(1) as usize;
    let dispatcher = Dispatcher::new(runners);
    let processor = make_processor(Dispatcher::new(runners), &config.whisper.model);
    let agent = Agent::new(args.server.clone(), register, dispatcher, processor);

    tracing::info!("submate node pulling work from {}", args.server);

    let runtime = tokio::runtime::Runtime::new()?;
    runtime
        .block_on(agent.run())
        .map_err(|e| anyhow::anyhow!("node agent stopped: {e}"))
}

/// Build the node's [`JobProcessor`].
///
/// With the `model` feature it forwards to the real whisper.cpp pipeline via
/// [`submate_node::whisper_processor`]. Without it — the default, model-less
/// build — every job resolves to a failure string, so the pull loop, queue
/// lifecycle, and HTTP transport are fully exercisable (and the CLI builds and
/// tests) without loading a model.
#[cfg(feature = "model")]
fn make_processor(
    dispatcher: submate_node::Dispatcher,
    model: &str,
) -> impl submate_node::JobProcessor {
    submate_node::whisper_processor(dispatcher, model.to_string())
}

#[cfg(not(feature = "model"))]
fn make_processor(
    _dispatcher: submate_node::Dispatcher,
    _model: &str,
) -> impl submate_node::JobProcessor {
    |_opts: &submate_proto::JobOpts, _pcm: Vec<u8>| async {
        Err::<String, String>(
            "model support not built in (rebuild with --features model)".to_string(),
        )
    }
}

/// A stable-ish node id for registration: the machine hostname, falling back to
/// `"node"`. The coordinator only requires uniqueness per server.
fn hostname_node_id() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "node".to_string())
}

#[cfg(test)]
mod cli {
    use super::*;
    use clap::CommandFactory;

    /// The clap definition is internally consistent (no overlapping flags, valid
    /// arg specs). `debug_assert` is clap's own structural validator.
    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    /// The subcommand surface matches the port contract: the five user-facing
    /// commands, with `node` present and the old `worker` gone.
    #[test]
    fn cli_help_subcommands() {
        let cmd = Cli::command();
        let names: Vec<&str> = cmd.get_subcommands().map(|c| c.get_name()).collect();

        for expected in ["transcribe", "translate", "server", "node", "config"] {
            assert!(names.contains(&expected), "missing subcommand `{expected}`");
        }
        assert!(
            !names.contains(&"worker"),
            "`worker` must be replaced by `node`"
        );
    }

    /// The global `--config-file` flag is present and global, and the
    /// subcommands carry their distinguishing flags (`node --server`,
    /// `transcribe --sync`, `translate --target-lang`).
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
            sub("node")
                .get_arguments()
                .any(|a| a.get_long() == Some("server")),
            "`node` must expose --server"
        );
        assert!(
            sub("transcribe")
                .get_arguments()
                .any(|a| a.get_long() == Some("sync")),
            "`transcribe` must expose --sync"
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
        let resolved = resolve_model(args.model.as_deref(), "medium")
            .expect("--model should resolve");
        assert_eq!(resolved, PathBuf::from("/models/ggml-base.en.bin"));

        // With no flag, a non-path config value, and no env var, the resolver
        // returns an actionable error (not a panic) naming both knobs and the
        // download hint.
        let prev = std::env::var_os("SUBMATE__WHISPER__MODEL");
        std::env::remove_var("SUBMATE__WHISPER__MODEL");
        let err = resolve_model(None, "medium")
            .expect_err("missing model must be an Err, not a panic")
            .to_string();
        if let Some(prev) = prev {
            std::env::set_var("SUBMATE__WHISPER__MODEL", prev);
        }

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

    /// The success summary formatter renders just the file names plus the cue
    /// count, pluralizing `cue`/`cues`, regardless of how deep the input path is.
    #[test]
    fn result_summary_format() {
        assert_eq!(
            result_summary(
                Path::new("/media/movies/movie.mkv"),
                Path::new("/media/movies/movie.srt"),
                42,
            ),
            "✓ movie.mkv → movie.srt (42 cues)",
        );
        assert_eq!(
            result_summary(Path::new("clip.mp4"), Path::new("clip.srt"), 1),
            "✓ clip.mp4 → clip.srt (1 cue)",
        );
        assert_eq!(
            result_summary(Path::new("clip.mp4"), Path::new("clip.srt"), 0),
            "✓ clip.mp4 → clip.srt (0 cues)",
        );
    }
}
