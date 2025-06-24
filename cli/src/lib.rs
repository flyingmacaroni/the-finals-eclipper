mod cache_clips;
mod clip_writer;
mod process;
pub mod process_frame;
mod video_decoder;

pub use crossbeam_channel as channel;
pub use ffmpeg_next as ffmpeg;
pub use process::write_clips;
pub use process::Args;
pub use process::PreviewFrame;
pub use process::ProcessResult;
pub use process::SearchParam;
pub use process::VideoProcessor;
pub use process::SEARCH_PARAMS;
pub use tesseract;
pub use video_decoder::VideoDecoder;

pub fn thread_count() -> usize {
    std::thread::available_parallelism()
        .map_or(4, |c| c.get())
        .max(1)
}
