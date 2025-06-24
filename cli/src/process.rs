#[allow(unused_imports)]
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use clap::ValueHint::FilePath;
use crossbeam_channel::TryRecvError;
use fast_image_resize as fr;
use ffmpeg::frame::Video;
use ffmpeg::software::scaling;
use ffmpeg_next as ffmpeg;
use ffmpeg_next::ffi::{av_hwframe_transfer_data, av_image_copy_to_buffer, AVPixelFormat};
use ffmpeg_next::format::Pixel;
use serde::{Deserialize, Serialize};
use tesseract::Tesseract;
use tracing::{error, info};

use crate::cache_clips::{cache_clips, clips_from_cache, CacheKey};
use crate::clip_writer::ClipWriter;
use crate::process_frame::{frame_binarisation, frame_brightness_contrast, scale_frame};
use crate::video_decoder::{t_to_secs, VideoDecoder};

/// Commandline args
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Path to input video
    #[arg(short, long, value_hint = FilePath)]
    pub input: PathBuf,
    // /// Path to output video
    // #[arg(short, long, value_hint = FilePath)]
    // output: PathBuf,
    // #[arg(long, default_value_t = 4)]
    // clip_length: i32,
    #[arg(long, default_value_t = false)]
    pub include_assists: bool,
    #[arg(long, default_value_t = false)]
    pub include_spectating: bool,
    #[arg(long, default_value_t = 4.)]
    pub elim_clip_duration: f64,
    #[arg(long, short = 'j', default_value_t = super::thread_count())]
    pub threads: usize,
}

pub struct Resize {
    height: u32,
}

pub struct BinarisationParams {
    min_rgb: [u8; 3],
    max_rgb: [u8; 3],
}

pub struct BrightnessContrastParams {
    brightness: f64,
    contrast: f64,
    invert: bool,
}

pub enum SearchParam<'a> {
    Text {
        patterns: &'a [&'a str],
        timeout: f64,
        clip_length_before: f64,
        clip_length_after: f64,
        search_area: SearchArea,
        binarisation_params: BinarisationParams,
        brightness_contrast_params: Option<BrightnessContrastParams>,
        resize: Option<Resize>,
    },
    #[allow(dead_code)]
    AveragePixelValue {
        value: u8,
        clip_length_before: f64,
        clip_length_after: f64,
    },
}

impl SearchParam<'_> {
    fn timeout(&self) -> f64 {
        match self {
            SearchParam::Text { timeout, .. } => *timeout,
            SearchParam::AveragePixelValue { .. } => 0.,
        }
    }
}

pub struct SearchArea {
    pub top: f64,
    pub left: f64,
    pub width: f64,
    pub height: f64,
}

pub struct PreviewFrame {
    pub width: u32,
    pub height: u32,
    pub pts: i64,
    pub pixels: Box<[u8]>,
}

pub static SEARCH_PARAMS: &[SearchParam] = &[
    SearchParam::Text {
        patterns: &[
            "elim",
            "eliminated",
            "bulhnates",
            "ehiminated",
            "eihiminated",
            "eiiminated",
            "elbinated",
            "eliminaterd",
            "elinated",
            "elininated",
            "elinminated",
            "eliriinated",
            "elirinated",
            "elirmnated",
            "elirnated",
            "elminated",
            "elrirated",
            "eminated",
            "eminater",
            "euehnated",
            "eulhnated",
            "eulhyated",
            "eultnated",
            "euminated",
            "fhninated",
            "fiangted",
            "fiiminaterd",
            "fiminaier",
            "fliinated",
            "flrinated",
            "fuchnater",
            "furater",
            "fushnated",
            "hiinated",
            "himinaied",
            "iminated",
            "suminates",
        ],
        timeout: 0.,
        clip_length_before: 4.0,
        clip_length_after: 0.0,
        search_area: SearchArea {
            top: 0.604,
            left: 0.2,
            width: 0.42,
            height: 0.091,
        },
        resize: Some(Resize { height: 720 }),
        binarisation_params: BinarisationParams {
            min_rgb: [215, 215, 215],
            max_rgb: [255, 254, 253],
        },
        brightness_contrast_params: Some(BrightnessContrastParams {
            brightness: -0.8,
            contrast: 10.,
            invert: true,
        }),
    },
    SearchParam::Text {
        patterns: &["assis", "ssist", "a58i8", "as5i", "amssr", "5i5t"],
        timeout: 0.,
        clip_length_before: 4.0,
        clip_length_after: 0.0,
        search_area: SearchArea {
            top: 0.604,
            left: 0.2,
            width: 0.42,
            height: 0.091,
        },
        resize: Some(Resize { height: 720 }),
        binarisation_params: BinarisationParams {
            min_rgb: [215, 215, 215],
            max_rgb: [255, 254, 253],
        },
        brightness_contrast_params: Some(BrightnessContrastParams {
            brightness: -0.8,
            contrast: 10.,
            invert: true,
        }),
    },
    SearchParam::Text {
        patterns: &["winners", "qualif", "lified", "vinners", "linkers"],
        timeout: 30.,
        clip_length_before: 10.,
        clip_length_after: 10.,
        search_area: SearchArea {
            top: 0.4,
            left: 0.17,
            width: 0.69,
            height: 0.2,
        },
        resize: Some(Resize { height: 360 }),
        binarisation_params: BinarisationParams {
            min_rgb: [248, 248, 248],
            max_rgb: [255, 255, 255],
        },
        brightness_contrast_params: Some(BrightnessContrastParams {
            brightness: -0.8,
            contrast: 10.,
            invert: true,
        }),
    },
    // SearchParam::Text {
    //     patterns: &["summary"],
    //     timeout: 30.,
    //     clip_length_after: 4.,
    //     clip_length_before: 0.0,
    //     search_area: SearchArea {
    //         top: 0.038,
    //         left: 0.0,
    //         width: 0.159,
    //         height: 0.0764,
    //     },
    // },
    // SearchParam::Text {
    //     patterns: &["the arena's calling!", "let's get into it!", "the arena has loaded", "buckle up"],
    // },
];

pub struct VideoProcessor {
    pub args: Args,
    pub progress_tx: crate::channel::Sender<ProgressMessage>,
    pub frame_tx: crate::channel::Sender<PreviewFrame>,
    pub keyframes: Box<[f64]>,
    pub clips: Option<Box<[[f64; 2]]>>,
    pub video_duration: f64,
}

impl VideoProcessor {
    pub fn open(
        args: Args,
        progress_tx: crate::channel::Sender<ProgressMessage>,
        frame_tx: crate::channel::Sender<PreviewFrame>,
    ) -> VideoProcessor {
        let decoder = VideoDecoder::open(&args.input, false).unwrap();
        let video_duration = decoder.video_duration();

        info!("video duration {}", format_seconds(video_duration as i32));

        let keyframes;
        let clips;
        if let Some(cache) = clips_from_cache(&args.input) {
            keyframes = cache.keyframes;
            clips = cache
                .clips
                .get(&CacheKey {
                    include_assists: args.include_assists,
                    include_spectating: args.include_spectating,
                })
                .cloned();
        } else {
            keyframes = decoder.key_frames();
            let is_sorted = keyframes.windows(2).all(|w| w[0] <= w[1]);
            assert!(is_sorted);
            clips = None;
        }

        VideoProcessor {
            args,
            progress_tx,
            frame_tx,
            keyframes,
            clips,
            video_duration,
        }
    }

    pub fn process(self, hw_accel: bool) -> ProcessResult {
        let video_duration = self.video_duration;

        let keyframes = self.keyframes;
        if let Some(clips) = self.clips {
            return ProcessResult {
                clips,
                keyframes,
                input_duration: video_duration,
            };
        }

        let thread_count = self.args.threads;

        let max_threads = video_duration.ceil() as i64 / 30 + 1;
        let thread_count = thread_count.min(max_threads as usize);

        let segment_len = video_duration as usize / thread_count;

        let mut join_handles = Vec::with_capacity(thread_count);
        let mut progress_channels = Vec::with_capacity(thread_count);

        for i in 0..thread_count {
            let start = (i * segment_len) as f64;
            let start = keyframes
                .binary_search_by(|ts| ts.partial_cmp(&(start)).expect("Couldn't compare values"))
                .unwrap_or_else(|err| err);
            let start = keyframes[start];
            let end = if i == thread_count {
                video_duration.ceil()
            } else {
                start + segment_len as f64
            };
            let end = keyframes
                .binary_search_by(|ts| ts.partial_cmp(&(end)).expect("Couldn't compare values"))
                .unwrap_or_else(|err| err);
            let end = if i == thread_count - 1 {
                video_duration.ceil()
            } else {
                keyframes[end]
            };

            let seek_ts =
                if let Some(keyframe) = keyframes.iter().rev().copied().find(|k| *k < start) {
                    keyframe
                } else {
                    0.
                };

            info!(
                "spawning thread for start: {}, end: {}",
                format_seconds(start as i32),
                format_seconds(end as i32)
            );

            let (progress_tx, progress_rx) = crate::channel::bounded::<f32>(thread_count);
            progress_channels.push(progress_rx);

            let args = self.args.clone();
            let frame_tx = self.frame_tx.clone();
            let handle = std::thread::spawn(move || {
                get_clip_times(args, seek_ts, start, end, progress_tx, frame_tx, hw_accel)
            });

            join_handles.push(handle);
        }

        let mut progress = vec![0.; progress_channels.len()];
        let mut sel = crate::channel::Select::new();
        for rx in progress_channels.iter() {
            sel.recv(rx);
        }
        let mut remove_count = 0;

        let mut previous_progress = 0.;
        let mut instant = Instant::now();
        let mut speed = 0.;
        loop {
            // Wait until a recv operation becomes ready and try executing it.
            let index = sel.ready();
            let res = progress_channels[index].try_recv();

            // If the operation turns out not to be ready, retry.
            if let Err(e) = res {
                if e.is_empty() {
                    continue;
                }
            }

            // Success!
            match res {
                Ok(p) => {
                    progress[index] = p;
                }
                Err(err) => match err {
                    TryRecvError::Empty => {}
                    TryRecvError::Disconnected => {
                        sel.remove(index);
                        remove_count += 1;
                        progress[index] = 100.;
                    }
                },
            }
            let p = progress.iter().sum::<f32>() / progress.len() as f32;
            if instant.elapsed().as_secs_f32() >= 1.0 {
                let progress_delta = p - previous_progress;
                let progress_delta_duration = progress_delta as f64 / 100. * video_duration;
                let elapsed = instant.elapsed().as_secs_f64();
                speed = progress_delta_duration / elapsed;
                previous_progress = p;
                instant = Instant::now();
            }
            self.progress_tx
                .send(ProgressMessage {
                    progress: p,
                    speed: speed as f32,
                })
                .ok();

            if remove_count == progress_channels.len() {
                self.progress_tx
                    .send(ProgressMessage {
                        progress: p,
                        speed: speed as f32,
                    })
                    .ok();
                break;
            }
        }

        let mut clips = Vec::<[f64; 2]>::new();
        // combine clip times
        for handle in join_handles {
            let mut new_clips = handle.join().unwrap().unwrap();

            // combine overlapping clip range
            if let Some((last_clip_range, next_clip_range)) =
                clips.last().zip(new_clips.first_mut())
            {
                if last_clip_range[1] >= next_clip_range[0] {
                    next_clip_range[0] = last_clip_range[0];
                    clips.pop();
                }
            }

            clips.extend_from_slice(&new_clips);
        }

        // make clip start and end times be on i-frames (key frames)
        for clip in clips.iter_mut() {
            let start_index = keyframes
                .binary_search_by(|ts| ts.partial_cmp(&clip[0]).expect("Couldn't compare values"))
                .unwrap_or_else(|err| err)
                .min(keyframes.len() - 1)
                .max(0);
            let mut start_time = keyframes[start_index];
            if start_time - clip[0] > 0.5 {
                start_time = keyframes[start_index.saturating_sub(1)];
            }
            let end_index = keyframes
                .binary_search_by(|ts| ts.partial_cmp(&clip[1]).expect("Couldn't compare values"))
                .unwrap_or_else(|err| err)
                .min(keyframes.len() - 1);
            let end_time = keyframes[end_index];
            clip[0] = start_time;
            clip[1] = end_time;
        }

        // combine overlapping clips
        for index in (0..clips.len().saturating_sub(1)).rev() {
            let next_clip = clips[index + 1];
            let clip = &mut clips[index];
            // overlap
            if next_clip[0] <= clip[1] {
                info!("found overlapping clip");
                clip[1] = next_clip[1];
                info!(
                    "new clip duration: {}",
                    format_seconds((clip[1] - clip[0]) as i32)
                );
                clips.remove(index + 1);
            }
        }

        for clip in clips.iter() {
            info!(
                "clip at: {:.2}, duration: {:.2}",
                clip[0],
                clip[1] - clip[0],
            );
        }

        let clips = clips.into_boxed_slice();

        cache_clips(clips.clone(), keyframes.clone(), &self.args);

        ProcessResult {
            clips,
            keyframes,
            input_duration: video_duration,
        }
    }
}

#[derive(serde::Serialize)]
pub struct ProcessResult {
    clips: Box<[[f64; 2]]>,
    keyframes: Box<[f64]>,
    input_duration: f64,
}

//noinspection DuplicatedCode
fn get_clip_times(
    args: Args,
    seek_ts: f64,
    start_ts: f64,
    end_ts: f64,
    progress_tx: crate::channel::Sender<f32>,
    frame_tx: crate::channel::Sender<PreviewFrame>,
    hw_accel: bool,
) -> Result<Vec<[f64; 2]>, ffmpeg::Error> {
    let clip_length = 4.0;
    let mut clips = Vec::new();

    let push_clip = |clips: &mut Vec<[f64; 2]>, clip_range: [f64; 2]| {
        // overlap
        if let Some(last_clip_range) = clips.last_mut() {
            if last_clip_range[1] >= clip_range[0] {
                last_clip_range[1] = clip_range[1];
                return;
            }
        }

        clips.push([clip_range[0].max(0.), clip_range[1]]);
    };

    let mut decoder = VideoDecoder::open(&args.input, hw_accel)?;
    let initial_format = decoder.initial_format();
    if seek_ts > 0. {
        decoder.seek(start_ts);
    }

    let width = decoder.width() as i32;
    let height = decoder.height() as i32;
    let frame_rate = decoder.frame_rate();
    let time_base = decoder.time_base();
    let mut rgb_scaler = decoder.rgb_scaler()?;
    // 10 times per second
    let step_size = frame_rate as usize / 10;
    info!("step_size: {step_size}");

    info!("Resolution {width}x{height}");

    let mut tess = Tesseract::new(None, Some("eng")).unwrap();

    let mut last_times = vec![-50000.; SEARCH_PARAMS.len()];

    'frame: for mut frame in decoder.decode_iter().step_by(step_size) {
        let time = t_to_secs(frame.pts().unwrap(), time_base);

        if time < start_ts {
            continue;
        }

        let progress = (time - start_ts) / (end_ts - start_ts) * 100.;
        progress_tx.send(progress.min(100.) as f32).ok();

        // if any timeouts are active continue
        if SEARCH_PARAMS
            .iter()
            .enumerate()
            .any(|(index, s)| last_times[index] + s.timeout() > time)
        {
            continue;
        }

        let frame_data = convert_frame_to_rgb24(&mut frame, &mut rgb_scaler, initial_format)
            .map_err(|err| error!("failed to convert frame to rgb: {err:#?}"))
            .unwrap();

        for (index, search) in SEARCH_PARAMS.iter().enumerate() {
            match search {
                SearchParam::Text {
                    search_area,
                    patterns,
                    clip_length_after,
                    clip_length_before,
                    timeout: _timeout,
                    resize,
                    binarisation_params,
                    brightness_contrast_params,
                } => {
                    // // remove duplicates
                    // let mut patts = patterns.to_vec();
                    // for i in (0..patts.len()).rev() {
                    //     let curr = patts[i];
                    //     if patts
                    //         .iter()
                    //         .enumerate()
                    //         .any(|(index, pat)| index != i && curr.contains(*pat))
                    //     {
                    //         patts.remove(i);
                    //     }
                    // }
                    // println!("{patts:?}");
                    let clip_length_before =
                        if patterns.contains(&"iminated") || patterns.contains(&"ass") {
                            args.elim_clip_duration
                        } else {
                            *clip_length_before
                        };

                    if !args.include_assists && patterns.contains(&"ass") {
                        continue;
                    }
                    if !args.include_spectating && !patterns.contains(&"winners") {
                        // check for spectating
                        let last_row = &frame_data[frame_data.len() - (width as usize * 3)..];
                        assert_eq!(last_row.len(), width as usize * 3);
                        let color_sum =
                            last_row
                                .chunks_exact(3)
                                .fold([0_i32, 0_i32, 0_i32], |mut acc, x| {
                                    acc[0] += x[0] as i32;
                                    acc[1] += x[1] as i32;
                                    acc[2] += x[2] as i32;

                                    acc
                                });
                        let avg_color = [
                            color_sum[0] / width,
                            color_sum[1] / width,
                            color_sum[2] / width,
                        ];
                        let r = avg_color[0];
                        let g = avg_color[1];
                        let b = avg_color[2];
                        let is_spectating =
                            r > 173 && r < 205 && g > 4 && g < 45 && b > 50 && b < 76;
                        // rgb_max = [r.max(rgb_max[0]), g.max(rgb_max[1]), b.max(rgb_max[2])];
                        // rgb_min = [r.min(rgb_min[0]), g.min(rgb_min[1]), b.min(rgb_min[2])];
                        if is_spectating {
                            continue;
                        }
                    }
                    let mut pixels;
                    let (pixels, width, height) = if let Some(resize) = resize {
                        let dst_height = resize.height as i32;
                        let scale_factor = height as f64 / resize.height as f64;
                        let dst_width = (width as f64 / scale_factor) as u32;
                        pixels = scale_frame(
                            &frame_data,
                            width,
                            height,
                            resize.height,
                            fr::ResizeAlg::Convolution(fr::FilterType::Bilinear),
                        );

                        (pixels.as_mut_slice(), dst_width as i32, dst_height)
                    } else {
                        pixels = frame_data.clone();
                        (pixels.as_mut_slice(), width, height)
                    };

                    let pixels_clone = brightness_contrast_params
                        .as_ref()
                        .map(|_| Vec::from_iter(pixels.iter().copied()));

                    frame_binarisation(
                        pixels,
                        binarisation_params.min_rgb,
                        binarisation_params.max_rgb,
                    );

                    let left = (search_area.left * width as f64) as i32;
                    let top = (search_area.top * height as f64) as i32;
                    let inner_width = (search_area.width * width as f64) as i32;
                    let inner_height = (search_area.height * height as f64) as i32;

                    // info!("search_area left: {left}, top: {top}, width: {inner_width}, height: {inner_height}");

                    tess = tess
                        .set_frame(pixels, width, height, 3, width * 3)
                        .unwrap()
                        .set_rectangle(left, top, inner_width, inner_height)
                        .recognize()
                        .unwrap();
                    let Ok(result) = tess.get_text() else {
                        continue 'frame;
                    };

                    // if patterns.contains(&"assist") || patterns.contains(&"iminated") {
                    //     println!("{}", result.to_lowercase());
                    // }

                    if patterns
                        .iter()
                        .any(|pat| result.to_lowercase().contains(pat))
                    {
                        info!("found matching text at: {}", format_seconds(time as i32));
                        info!(
                            "thread progress: {:.1}%",
                            (time - start_ts) / (end_ts - start_ts) * 100.
                        );
                        last_times[index] = time;
                        push_clip(
                            &mut clips,
                            [time - clip_length_before, time + clip_length_after],
                        );
                        frame_tx
                            .send(PreviewFrame {
                                pts: frame.pts().unwrap(),
                                width: frame.width(),
                                height: frame.height(),
                                pixels: frame_data.into_boxed_slice(),
                            })
                            .ok();
                        continue 'frame;
                    }

                    if let Some(BrightnessContrastParams {
                        brightness,
                        contrast,
                        invert,
                    }) = *brightness_contrast_params
                    {
                        let mut pixels = pixels_clone.unwrap();
                        frame_brightness_contrast(&mut pixels, brightness, contrast, invert);
                        tess = tess
                            .set_frame(&pixels, width, height, 3, width * 3)
                            .unwrap()
                            .set_rectangle(left, top, inner_width, inner_height)
                            .recognize()
                            .unwrap();

                        let Ok(result) = tess.get_text() else {
                            continue 'frame;
                        };

                        if patterns
                            .iter()
                            .any(|pat| result.to_lowercase().contains(pat))
                        {
                            info!(
                                "found matching text using brightness/contrast at: {}",
                                format_seconds(time as i32)
                            );
                            info!(
                                "thread progress: {:.1}%",
                                (time - start_ts) / (end_ts - start_ts) * 100.
                            );
                            last_times[index] = time;
                            push_clip(
                                &mut clips,
                                [time - clip_length_before, time + clip_length_after],
                            );
                            frame_tx
                                .send(PreviewFrame {
                                    pts: frame.pts().unwrap(),
                                    width: frame.width(),
                                    height: frame.height(),
                                    pixels: frame_data.into_boxed_slice(),
                                })
                                .ok();
                            continue 'frame;
                        }
                    }
                }
                SearchParam::AveragePixelValue {
                    value,
                    clip_length_before,
                    clip_length_after,
                } => {
                    let frame_data = frame.data(0);
                    let average: u64 =
                        frame_data.iter().map(|v| *v as u64).sum::<u64>() / frame_data.len() as u64;
                    if average as u8 >= *value {
                        info!("found average pixel value: {}", { average });
                        push_clip(
                            &mut clips,
                            [time - clip_length_before, time + clip_length_after],
                        );
                        continue 'frame;
                    }
                }
            }
        }

        // if (time as i64 >= end_ts && frame.is_key()) || (time as i64 > end_ts) {
        if time >= end_ts {
            info!("reached end_ts");
            break;
        }
    }

    for clip in clips.iter() {
        info!(
            "clip at: {}, duration: {}",
            format_seconds(clip[0] as i32),
            format_seconds((clip[1] - clip[0]) as i32)
        );
        let duration = clip[1] - clip[0];
        if duration < clip_length {
            info!("duration < clip_length, duration: {duration}, clip_length: {clip_length}");
        }
    }

    Ok(clips)
}

pub fn write_clips(input: &PathBuf, output: &PathBuf, clips: &[[f64; 2]], keyframes: &[f64]) {
    info!("writing clips...");
    let input_file = input;
    let output_file = output;

    let mut clip_writer = ClipWriter::new(input_file, output_file);
    clip_writer.set_keyframes(keyframes);

    for clip in clips.iter() {
        clip_writer.write_clip(clip[0], clip[1]);
    }

    clip_writer.write_trailer();
}

fn to_rgb(
    frame: &Video,
    rgb_scaler: &mut scaling::Context,
    initial_format: Pixel,
) -> Result<Video, ffmpeg::Error> {
    let mut sw_frame;
    // if frame pixel format is not the same as initial format it means we are using hardware decoding, and we need to transfer the frame from hardware to memory
    let frame = if frame.format() != initial_format {
        sw_frame = Video::empty();
        let ret = unsafe { av_hwframe_transfer_data(sw_frame.as_mut_ptr(), frame.as_ptr(), 0) };
        if ret == 0 {
            sw_frame.set_pts(frame.pts());
            unsafe {
                (*sw_frame.as_mut_ptr()).key_frame = (*frame.as_ptr()).key_frame;
            }
            &sw_frame
        } else {
            return Err(ffmpeg::Error::from(ret));
        }
    } else {
        frame
    };
    let mut rgb_frame = Video::empty();
    if rgb_scaler.input().format != frame.format() {
        *rgb_scaler = scaling::Context::get(
            frame.format(),
            frame.width(),
            frame.height(),
            Pixel::RGB24,
            frame.width(),
            frame.height(),
            scaling::Flags::AREA,
        )?;
    }
    rgb_scaler.run(frame, &mut rgb_frame)?;
    rgb_frame.set_pts(frame.pts());
    unsafe {
        (*rgb_frame.as_mut_ptr()).key_frame = (*frame.as_ptr()).key_frame;
    }
    Ok(rgb_frame)
}

fn format_seconds(seconds: i32) -> String {
    let seconds_i32 = seconds;
    let seconds = seconds_i32 % 60;
    let minutes = (seconds_i32 / 60) % 60;
    let hours = (seconds_i32 / 60) / 60;

    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

pub fn convert_frame_to_rgb24(
    frame: &mut Video,
    rgb_scaler: &mut scaling::Context,
    initial_format: Pixel,
) -> Result<Vec<u8>, i32> {
    let mut new_frame;
    let frame = if frame.format() != Pixel::RGB24 {
        new_frame = to_rgb(frame, rgb_scaler, initial_format).unwrap();
        &mut new_frame
    } else {
        frame
    };

    unsafe {
        let frame_ptr = frame.as_mut_ptr();
        let frame_width: i32 = (*frame_ptr).width;
        let frame_height: i32 = (*frame_ptr).height;
        let frame_format =
            std::mem::transmute::<std::ffi::c_int, AVPixelFormat>((*frame_ptr).format);
        assert_eq!(frame_format, AVPixelFormat::AV_PIX_FMT_RGB24);

        let mut frame_array = vec![0; frame_height as usize * frame_width as usize * 3_usize];

        let bytes_copied = av_image_copy_to_buffer(
            frame_array.as_mut_ptr(),
            frame_array.len() as i32,
            (*frame_ptr).data.as_ptr() as *const *const u8,
            (*frame_ptr).linesize.as_ptr(),
            frame_format,
            frame_width,
            frame_height,
            1,
        );

        if bytes_copied == frame_array.len() as i32 {
            Ok(frame_array)
        } else {
            Err(bytes_copied as i32)
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ProgressMessage {
    pub progress: f32,
    pub speed: f32,
}

mod base64 {
    use base64::prelude::BASE64_STANDARD;
    use base64::Engine;
    use serde::{Deserialize, Serialize};
    use serde::{Deserializer, Serializer};

    #[allow(unused)]
    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        let base64 = BASE64_STANDARD.encode(v);
        String::serialize(&base64, s)
    }

    #[allow(unused)]
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Box<[u8]>, D::Error> {
        let base64 = String::deserialize(d)?;
        BASE64_STANDARD
            .decode(base64.as_bytes())
            .map(|v| v.into_boxed_slice())
            .map_err(serde::de::Error::custom)
    }
}
