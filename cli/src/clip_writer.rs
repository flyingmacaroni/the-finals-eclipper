use crate::ffmpeg;
use crate::video_decoder::t_to_secs;
use ffmpeg_next::ffi::{av_seek_frame, AVSEEK_FLAG_ANY, AV_TIME_BASE_Q};
use ffmpeg_next::format::context;
use ffmpeg_next::{codec, encoder, format, log, media, Rational};
use std::ffi::c_int;
use std::path::PathBuf;

pub struct ClipWriter {
    end_ts: i64,
    ictx: context::Input,
    octx: context::Output,
    /// index of video stream in ictx
    video_stream_index: usize,
    video_stream_timebase: Rational,
    /// maps input stream index to output stream index
    /// output index will be -1 if no octx stream exists for index
    stream_mapping: Vec<isize>,
    /// keyframe timestamps in video_stream_timebase
    keyframes: Option<Box<[i64]>>,
}

impl ClipWriter {
    pub fn new(input_file: &PathBuf, output_file: &PathBuf) -> ClipWriter {
        ffmpeg::init().unwrap();
        log::set_level(log::Level::Warning);

        let ictx = format::input(input_file).unwrap();
        let mut octx = format::output(output_file).unwrap();

        let video_stream = ictx.streams().best(media::Type::Video).unwrap();
        let video_stream_index = video_stream.index();
        let video_stream_timebase = video_stream.time_base();

        let mut stream_mapping = vec![0_isize; ictx.nb_streams() as _];
        let mut ost_index = 0;
        for (ist_index, ist) in ictx.streams().enumerate() {
            let ist_medium = ist.parameters().medium();
            if ist_medium != media::Type::Audio
                && ist_medium != media::Type::Video
                && ist_medium != media::Type::Subtitle
            {
                stream_mapping[ist_index] = -1;
                continue;
            }
            stream_mapping[ist_index] = ost_index;
            ost_index += 1;
            let mut ost = octx.add_stream(encoder::find(codec::Id::None)).unwrap();
            ost.set_parameters(ist.parameters());
            ost.set_time_base(ist.time_base());
            ist.start_time();
            // We need to set codec_tag to 0 lest we run into incompatible codec tag
            // issues when muxing into a different container format. Unfortunately
            // there's no high level API to do this (yet).
            unsafe {
                (*ost.parameters().as_mut_ptr()).codec_tag = 0;
            }
        }

        octx.set_metadata(ictx.metadata().to_owned());
        octx.write_header().unwrap();

        ClipWriter {
            end_ts: 0,
            ictx,
            octx,
            video_stream_index,
            video_stream_timebase,
            stream_mapping,
            keyframes: None,
        }
    }

    pub fn seek(&mut self, timestamp: i64) {
        unsafe {
            av_seek_frame(
                self.ictx.as_mut_ptr(),
                self.video_stream_index as c_int,
                timestamp,
                AVSEEK_FLAG_ANY,
            );
        };
    }

    pub fn keyframes(&mut self) -> &[i64] {
        if self.keyframes.is_none() {
            self.keyframes = Some(self.compute_keyframes());
        }
        self.keyframes.as_ref().unwrap()
    }

    pub fn compute_keyframes(&mut self) -> Box<[i64]> {
        let mut key_frames = vec![];
        for (stream, packet) in self.ictx.packets() {
            if stream.index() == self.video_stream_index && packet.is_key() {
                key_frames.push(packet.pts().unwrap());
            }
        }
        self.ictx.seek(0, 0..1).unwrap();

        key_frames.into_boxed_slice()
    }

    pub fn write_clip(&mut self, from_secs: f64, to_secs: f64) {
        // convert from_secs and to_secs to timebase timestamp
        let from_ts = secs_to_ts(from_secs, self.video_stream_timebase);

        let keyframe_index = self.keyframes().iter().position(|ts| ts >= &from_ts);
        let Some(keyframe_index) = keyframe_index else {
            eprintln!("Warning: from_secs out of range");
            return;
        };
        // for some reason I have to seek to the previous keyframe otherwise the first dts is not set
        let keyframe = self.keyframes()[keyframe_index.saturating_sub(2)];
        println!(
            "seeking to: {}",
            t_to_secs(keyframe, self.video_stream_timebase)
        );
        self.seek(keyframe);

        let mut first_pts: Option<i64> = None;
        let mut last_pts = 0;
        let mut last_duration = 0;

        for (stream, mut packet) in self.ictx.packets() {
            let ost_index = self.stream_mapping[stream.index()];
            if ost_index == -1 {
                continue;
            }

            let to_ts = secs_to_ts(to_secs, stream.time_base());
            let from_ts = secs_to_ts(from_secs, stream.time_base());

            if packet.pts().unwrap_or(0) >= from_ts && packet.pts().unwrap_or(i64::MAX) < to_ts {
                if let Some(pts) = packet.pts() {
                    let pts = if stream.index() == self.video_stream_index {
                        pts
                    } else {
                        convert_timebase(pts, stream.time_base(), self.video_stream_timebase)
                    };
                    let old_first = first_pts.get_or_insert(pts);
                    if *old_first > pts {
                        eprintln!("found and older pts! this should not happen I think. I don't know. Print this anyway just in case.");
                        *old_first = pts;
                    }
                    if pts > last_pts && stream.index() == self.video_stream_index {
                        last_pts = pts;
                        last_duration = packet.duration();
                    }
                }
                let Some(first_pts) = first_pts else { continue };
                let first_pts = if stream.index() == self.video_stream_index {
                    first_pts
                } else {
                    convert_timebase(first_pts, self.video_stream_timebase, stream.time_base())
                };
                let end_ts = if stream.index() == self.video_stream_index {
                    self.end_ts
                } else {
                    convert_timebase(self.end_ts, self.video_stream_timebase, stream.time_base())
                };
                let difference = -first_pts + end_ts;
                packet.set_pts(packet.pts().map(|pts| pts + difference));
                packet.set_dts(packet.dts().map(|dts| dts + difference));
                // let ost = self.octx.stream(ost_index as _).unwrap();
                // packet.rescale_ts(stream.time_base(), ost.time_base());
                packet.set_position(-1);
                packet.set_stream(ost_index as _);
                packet.write_interleaved(&mut self.octx).unwrap();
            }

            if t_to_secs(packet.pts().unwrap_or(0), stream.time_base()) > to_secs + 10. {
                break;
            }
        }

        let duration = last_pts - first_pts.unwrap() + last_duration;
        self.end_ts += duration;
    }

    #[allow(dead_code)]
    pub fn keyframes_secs(&mut self) -> Box<[f64]> {
        let keyframes = self.keyframes().to_vec();
        keyframes
            .into_iter()
            .map(|k| t_to_secs(k, self.video_stream_timebase))
            .collect::<Box<[_]>>()
    }

    pub fn set_keyframes(&mut self, keyframes: &[f64]) {
        self.keyframes = Some(
            keyframes
                .iter()
                .map(|k| secs_to_ts(*k, self.video_stream_timebase))
                .collect(),
        );
    }

    pub fn write_trailer(&mut self) {
        let duration = convert_timebase(
            self.end_ts,
            self.video_stream_timebase,
            AV_TIME_BASE_Q.into(),
        );
        unsafe {
            (*self.octx.as_mut_ptr()).duration = duration;
        }
        self.octx.write_trailer().unwrap();
    }
}

pub fn secs_to_ts(secs: f64, time_base: Rational) -> i64 {
    (secs / (time_base.numerator() as f64 / time_base.denominator() as f64)) as i64
}

pub fn convert_timebase(ts: i64, from_base: Rational, to_base: Rational) -> i64 {
    let secs = t_to_secs(ts, from_base);
    secs_to_ts(secs, to_base)
}
