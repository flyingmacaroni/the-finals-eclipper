// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::convert::Into;
use std::path::PathBuf;
use std::sync::{Mutex, RwLock};

use tauri::{AppHandle, Emitter};
use tracing::info;
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::util::SubscriberInitExt;

use common::channel::unbounded;
use common::{ProcessResult, VideoProcessor};
use image_experimenter::process_image;

use crate::file_server::{get_file_server_address, serve, CLIP_CACHE};

mod file_server;
mod image_experimenter;
mod pixels_to_base64_image;

static VIDEO_FILE: Mutex<Option<PathBuf>> = Mutex::new(None);
static KEYFRAMES: RwLock<Option<Box<[f64]>>> = RwLock::new(None);

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) {
    info!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command(async)]
fn process(
    input: String,
    threads: usize,
    include_assists: bool,
    include_spectating: bool,
    elim_clip_duration: f64,
    hw_accel: bool,
    app_handle: AppHandle,
) -> ProcessResult {
    let path: PathBuf = input.parse().unwrap();
    {
        let mut p = VIDEO_FILE.lock().unwrap();
        *p = Some(path.clone());
    }
    let (progress_tx, progress_rx) = unbounded();
    let (frame_tx, frame_rx) = unbounded();
    let args = common::Args {
        input: path,
        threads,
        include_assists,
        elim_clip_duration,
        include_spectating,
    };
    let video_processor = VideoProcessor::open(args, progress_tx, frame_tx);
    {
        let mut lock = KEYFRAMES.write().unwrap();
        *lock = Some(video_processor.keyframes.clone());
    }
    let join_handle = std::thread::spawn(move || video_processor.process(hw_accel));

    while let Ok(progress) = progress_rx.recv() {
        app_handle.emit("progress", progress).ok();
        while let Ok(frame) = frame_rx.try_recv() {
            let pts = frame.pts;
            CLIP_CACHE.insert_frame(frame);
            app_handle.emit("preview_frame", pts).ok();
        }
    }
    while let Ok(frame) = frame_rx.recv() {
        let pts = frame.pts;
        CLIP_CACHE.insert_frame(frame);
        app_handle.emit("preview_frame", pts).ok();
    }
    join_handle.join().unwrap()
}

#[tauri::command(async)]
fn write_clips(clips: Vec<[f64; 2]>, keyframes: Vec<f64>, path: String) {
    let input: PathBuf = {
        let lock = VIDEO_FILE.lock().unwrap();
        Option::clone(&lock).unwrap_or_default()
    };
    let output: PathBuf = path.parse().unwrap();
    common::write_clips(&input, &output, &clips, &keyframes);
}

#[tauri::command]
fn max_thread_count() -> usize {
    common::thread_count()
}

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let handle = rt.spawn(serve());

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            process,
            max_thread_count,
            process_image,
            get_file_server_address,
            write_clips,
        ])
        .setup(|app| {
            let handle = app.handle();
            let subscriber = fmt()
                .with_writer(LogWriter {
                    app_handle: handle.clone(),
                })
                .event_format(
                    fmt::format()
                        .with_file(false)
                        .with_source_location(false)
                        .with_target(false)
                        .without_time(),
                )
                .finish();
            subscriber.init();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    handle.abort();
}

#[derive(Clone)]
struct LogWriter {
    app_handle: AppHandle,
}

impl<'a> MakeWriter<'a> for LogWriter {
    type Writer = LogWriter;

    fn make_writer(&self) -> Self::Writer {
        self.clone()
    }
}

impl std::io::Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let string = String::from_utf8_lossy(buf).to_string();
        self.app_handle.emit("log", string).ok();
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
