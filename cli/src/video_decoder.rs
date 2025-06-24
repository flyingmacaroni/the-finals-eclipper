use std::ffi::{c_uchar, c_void, CString};
use std::io::{Cursor, Seek, SeekFrom, Write};
use std::os::raw::c_int;
use std::path::PathBuf;
use std::ptr;

use ffmpeg::format::Pixel;
use ffmpeg::frame::Video;
use ffmpeg::software::scaling;
use ffmpeg::{codec, format, media, Rational};
use ffmpeg_next as ffmpeg;
use ffmpeg_next::sys::AVHWDeviceType::{
    AV_HWDEVICE_TYPE_CUDA, AV_HWDEVICE_TYPE_DXVA2, AV_HWDEVICE_TYPE_NONE,
};
use ffmpeg_next::sys::{
    av_buffer_ref, av_guess_format, av_hwdevice_ctx_create, av_hwdevice_iterate_types, av_malloc,
    av_seek_frame, avcodec_get_hw_config, avformat_alloc_output_context2, avformat_flush,
    avio_alloc_context, AVCodecContext, AVHWDeviceType, AVPixelFormat,
    AV_CODEC_HW_CONFIG_METHOD_HW_DEVICE_CTX, AV_NOPTS_VALUE, AV_TIME_BASE,
};
use ffmpeg_next::{encoder, Error};
use tracing::info;

pub struct VideoDecoder {
    #[allow(dead_code)]
    path: PathBuf,
    input_ctx: format::context::Input,
    decoder: codec::decoder::Video,
    video_stream_index: usize,
    initial_format: Pixel,
}

impl VideoDecoder {
    pub fn open<P: AsRef<std::path::Path>>(
        path: &P,
        hw_accel: bool,
    ) -> Result<VideoDecoder, Error> {
        let input_ctx = format::input(path)?;

        let video_stream = input_ctx
            .streams()
            .best(media::Type::Video)
            .ok_or(Error::StreamNotFound)?;

        let video_stream_index = video_stream.index();

        let mut decoder_ctx = codec::context::Context::from_parameters(video_stream.parameters())?;

        if hw_accel {
            let mut hw_device_ctx = ptr::null_mut();
            unsafe {
                (*decoder_ctx.as_mut_ptr()).get_format = Some(get_hw_format);
                let device_type = get_device_type();
                let err = av_hwdevice_ctx_create(
                    &mut hw_device_ctx,
                    device_type,
                    ptr::null(),
                    ptr::null_mut(),
                    0,
                );
                if err != 0 {
                    eprintln!("failed to run hwdevice_ctx_create");
                    return Err(Error::DecoderNotFound);
                }
                (*decoder_ctx.as_mut_ptr()).hw_device_ctx = av_buffer_ref(hw_device_ctx);
            }
        }

        let decoder = decoder_ctx.decoder().video()?;
        let initial_format = decoder.format();

        Ok(VideoDecoder {
            path: path.as_ref().to_path_buf(),
            input_ctx,
            decoder,
            video_stream_index,
            initial_format,
        })
    }

    pub fn initial_format(&self) -> Pixel {
        self.initial_format
    }

    pub fn decode_frame(&mut self) -> Result<Video, Error> {
        let mut decoded = Video::empty();
        for (stream, packet) in self.input_ctx.packets() {
            if stream.index() == self.video_stream_index {
                if self.decoder.receive_frame(&mut decoded).is_ok() {
                    return Ok(decoded);
                }

                self.decoder.send_packet(&packet)?;

                if self.decoder.receive_frame(&mut decoded).is_ok() {
                    return Ok(decoded);
                }
            }
        }

        self.decoder.send_eof()?;

        if self.decoder.receive_frame(&mut decoded).is_ok() {
            return Ok(decoded);
        }

        Err(Error::Eof)
    }

    pub fn rgb_scaler(&mut self) -> Result<scaling::Context, Error> {
        scaling::Context::get(
            self.decoder.format(),
            self.decoder.width(),
            self.decoder.height(),
            Pixel::RGB24,
            self.decoder.width(),
            self.decoder.height(),
            scaling::Flags::AREA,
        )
    }

    pub fn decode_iter(&mut self) -> VideoDecoderIter {
        VideoDecoderIter { decoder: self }
    }

    pub fn width(&self) -> u32 {
        self.decoder.width()
    }

    pub fn height(&self) -> u32 {
        self.decoder.height()
    }

    pub fn frame_rate(&self) -> f64 {
        let rate = self
            .input_ctx
            .stream(self.video_stream_index)
            .unwrap()
            .rate();
        rate.numerator() as f64 / rate.denominator() as f64
    }

    pub fn time_base(&self) -> Rational {
        self.input_ctx
            .stream(self.video_stream_index)
            .unwrap()
            .time_base()
    }

    pub fn seek(&mut self, secs: f64) {
        let time_base = self.time_base();
        let timestamp = secs / (time_base.numerator() as f64 / time_base.denominator() as f64);

        info!("secs: {secs}, timestamp: {timestamp}");
        unsafe {
            av_seek_frame(
                self.input_ctx.as_mut_ptr(),
                self.video_stream_index as c_int,
                timestamp as i64,
                0,
            )
        };
    }

    // video duration
    pub fn video_duration(&self) -> f64 {
        let duration = self.input_ctx.duration();

        if duration == AV_NOPTS_VALUE {
            return 0.;
        }

        duration as f64 / AV_TIME_BASE as f64
    }

    pub fn key_frames(mut self) -> Box<[f64]> {
        let mut key_frames = vec![];
        let video = self
            .input_ctx
            .streams()
            .best(media::Type::Video)
            .unwrap()
            .index();
        for (stream, packet) in self.input_ctx.packets() {
            if stream.index() == video && packet.is_key() {
                key_frames.push(t_to_secs(packet.pts().unwrap(), stream.time_base()));
            }
        }

        key_frames.into_boxed_slice()
    }

    pub fn transcode_range(
        &mut self,
        start: f64,
        end: f64,
        keyframes: &[f64],
    ) -> Result<Box<[u8]>, Error> {
        let mut ofmt_ctx = ptr::null_mut();
        let out_buffer;
        let mut output = Vec::new();
        let mut cursor = Cursor::new(&mut output);

        let mut octx = unsafe {
            out_buffer = av_malloc(32768) as *mut c_uchar;
            let filename = CString::new(self.path.file_name().unwrap().to_str().unwrap()).unwrap();
            let format = av_guess_format(ptr::null(), filename.as_ptr(), ptr::null());
            match avformat_alloc_output_context2(&mut ofmt_ctx, format, ptr::null(), ptr::null()) {
                0 => {
                    let out_ctx = avio_alloc_context(
                        out_buffer,
                        32768,
                        1,
                        (&mut cursor as *mut Cursor<&mut Vec<u8>>) as *mut c_void,
                        None,
                        Some(write_buffer),
                        Some(seek_buffer),
                    );
                    (*ofmt_ctx).pb = out_ctx;
                    Ok(format::context::Output::wrap(ofmt_ctx))
                }

                e => Err(Error::from(e)),
            }
        }?;

        let mut stream_mapping = vec![0_isize; self.input_ctx.nb_streams() as _];
        let mut ost_index = 0;
        for (ist_index, ist) in self.input_ctx.streams().enumerate() {
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
            // We need to set codec_tag to 0 lest we run into incompatible codec tag
            // issues when muxing into a different container format. Unfortunately
            // there's no high level API to do this (yet).
            unsafe {
                (*ost.parameters().as_mut_ptr()).codec_tag = 0;
            }
        }

        octx.set_metadata(self.input_ctx.metadata().to_owned());
        octx.write_header()?;

        if let Some(keyframe) = keyframes.iter().rev().copied().find(|k| *k < start) {
            self.seek(keyframe);
        }

        for (stream, mut packet) in self.input_ctx.packets() {
            let ost_index = stream_mapping[packet.stream()];
            if ost_index < 0 {
                continue;
            }
            let ist_time_base = stream.time_base();
            let ts = t_to_secs(packet.pts().unwrap(), ist_time_base);
            if ts > end + 5. {
                break;
            }
            if !(ts >= start && ts <= end) {
                continue;
            }
            let ost = octx.stream(ost_index as _).unwrap();
            let skipped_duration = start;
            let skipped_pts = (skipped_duration
                / (ist_time_base.numerator() as f64 / ist_time_base.denominator() as f64))
                as i64;
            packet.set_pts(packet.pts().map(|pts| pts - skipped_pts));
            packet.set_dts(packet.dts().map(|dts| dts - skipped_pts));
            packet.rescale_ts(ist_time_base, ost.time_base());
            packet.set_position(-1);
            packet.set_stream(ost_index as _);
            packet.write_interleaved(&mut octx)?;
        }

        octx.write_trailer()?;
        unsafe {
            avformat_flush(octx.as_mut_ptr());
        }

        unsafe {
            (*(*octx.as_mut_ptr()).pb).opaque = ptr::null_mut();
        };

        Ok(output.into())
    }
}

const SEEK_SET: c_int = 0; /* Seek from beginning of file.  */
const SEEK_CUR: c_int = 1; /* Seek from current position.  */
const SEEK_END: c_int = 2; /* Seek from end of file.  */

unsafe extern "C" fn seek_buffer(opaque: *mut c_void, offset: i64, whence: c_int) -> i64 {
    let output: &mut Cursor<&mut Vec<u8>> = &mut *(opaque as *mut Cursor<&mut Vec<u8>>);
    let seek_from = match whence {
        SEEK_SET => SeekFrom::Start(offset as u64),
        SEEK_CUR => SeekFrom::Current(offset),
        SEEK_END => SeekFrom::End(offset),
        _ => {
            eprintln!("invalid whence: {whence}. seeking from start");
            SeekFrom::Start(offset as u64)
        }
    };
    output.seek(seek_from).unwrap_or_default() as i64
}

unsafe extern "C" fn write_buffer(opaque: *mut c_void, buf: *const u8, buf_size: c_int) -> c_int {
    let output: &mut Cursor<&mut Vec<u8>> = &mut *(opaque as *mut Cursor<&mut Vec<u8>>);

    let data = std::slice::from_raw_parts(buf, buf_size as usize);

    output.write(data).unwrap_or_default() as c_int
}

pub struct VideoDecoderIter<'a> {
    decoder: &'a mut VideoDecoder,
}

impl<'a> Iterator for VideoDecoderIter<'a> {
    type Item = Video;

    fn next(&mut self) -> Option<Self::Item> {
        self.decoder.decode_frame().ok()
    }
}

pub fn t_to_secs(t: i64, time_base: Rational) -> f64 {
    let ratio = time_base.numerator() as f64 / time_base.denominator() as f64;
    t as f64 * ratio
}

fn get_device_type() -> AVHWDeviceType {
    unsafe {
        let mut device_type = av_hwdevice_iterate_types(AV_HWDEVICE_TYPE_NONE);
        let first_device_type = device_type;
        while device_type != AV_HWDEVICE_TYPE_NONE {
            println!("device_type: {device_type:?}");
            if matches!(device_type, AV_HWDEVICE_TYPE_CUDA | AV_HWDEVICE_TYPE_DXVA2) {
                break;
            }
            device_type = av_hwdevice_iterate_types(device_type);
        }
        if matches!(device_type, AV_HWDEVICE_TYPE_CUDA | AV_HWDEVICE_TYPE_DXVA2) {
            device_type
        } else {
            first_device_type
        }
    }
}

unsafe extern "C" fn get_hw_format(
    ctx: *mut AVCodecContext,
    pix_fmts: *const AVPixelFormat,
) -> AVPixelFormat {
    let device_type = get_device_type();
    println!("device_type: {device_type:#?}");

    let hw_pix_fmt = unsafe {
        let mut hw_pix_fmt = None;
        for i in 0.. {
            let hw_config = avcodec_get_hw_config((*ctx).codec, i);
            if hw_config.is_null() {
                break;
            }
            println!("{:?}", *hw_config);
            if ((*hw_config).methods & AV_CODEC_HW_CONFIG_METHOD_HW_DEVICE_CTX as i32 != 0)
                && (*hw_config).device_type == device_type
            {
                hw_pix_fmt = Some((*hw_config).pix_fmt);
                break;
            }
        }
        hw_pix_fmt
    };

    if let Some(hw_pix_fmt) = hw_pix_fmt {
        let mut p = *pix_fmts;
        while p as i32 != -1 {
            let next_n = p as i32;
            // manual hack for safely wrapping
            if next_n > AVPixelFormat::AV_PIX_FMT_NB as i32 {
                p = AVPixelFormat::AV_PIX_FMT_NONE
            } else {
                p = std::mem::transmute(next_n);
            }
            if p == hw_pix_fmt {
                return p;
            }
        }
        println!("Failed to get HW surface format.");
        hw_pix_fmt
    } else {
        println!("No surface format.");
        AVPixelFormat::AV_PIX_FMT_NONE
    }
}
