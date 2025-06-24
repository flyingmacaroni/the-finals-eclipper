use std::collections::Bound;
use std::num::NonZeroUsize;
use std::sync::{Mutex, OnceLock};

use axum::extract::Query;
use axum::http::{header, Response};
use axum::response::IntoResponse;
use axum_extra::TypedHeader;
use bytes::Bytes;
use headers::Header;
use lru::LruCache;
use serde::Deserialize;

use common::{PreviewFrame, VideoDecoder};

use crate::{KEYFRAMES, VIDEO_FILE};
pub static CLIP_CACHE: Cache = Cache::new();

// use actix_files as fs;
// use actix_web::{get, App, Error, HttpRequest, HttpServer};

pub static PORT: OnceLock<u16> = OnceLock::new();

// #[get("/{filename:.*}")]
// async fn index(req: HttpRequest) -> Result<fs::NamedFile, Error> {
//     let path: std::path::PathBuf = req.match_info().query("filename").parse().unwrap();
//     let file = fs::NamedFile::open_async(path).await?;
//     Ok(file)
// }
//
#[tauri::command]
pub fn get_file_server_address() -> String {
    format!(
        "http://127.0.0.1:{}",
        PORT.get().copied().unwrap_or_default()
    )
}

// pub async fn serve() -> std::io::Result<()> {
//     let srv = HttpServer::new(|| App::new().service(index)).bind(("127.0.0.1", 0))?;
//
//     PORT.get_or_init(|| srv.addrs()[0].port());
//
//     println!("listening on: {}", get_file_server_address());
//
//     srv.run().await
// }

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Params {
    start: f64,
    end: f64,
}

async fn index(
    Query(params): Query<Params>,
    range: Option<TypedHeader<headers::Range>>,
) -> impl IntoResponse {
    let result = tokio::task::block_in_place(|| CLIP_CACHE.get_clip((params.start, params.end)));

    let data_len = result.as_ref().map(|data| data.len()).unwrap_or_default();

    let mut ranges = Vec::new();
    if let Some(TypedHeader(range)) = range {
        for (start, end) in range.satisfiable_ranges(data_len as u64) {
            let start = match start {
                Bound::Included(start) => start,
                Bound::Excluded(start) => start + 1,
                Bound::Unbounded => 0,
            };
            let end = match end {
                Bound::Included(end) => end + 1,
                Bound::Excluded(end) => end,
                Bound::Unbounded => data_len as u64,
            };
            ranges.push(start as usize..end as usize);
        }
    }

    let data = if !ranges.is_empty() {
        let range = ranges[0].clone();
        result.map(|data| {
            (
                headers::ContentRange::bytes(range.start as u64..range.end as u64, data_len as u64),
                data.slice(range),
            )
        })
    } else {
        result.map(|data| {
            (
                headers::ContentRange::bytes(0..data_len as u64, data_len as u64),
                data,
            )
        })
    };

    data.map_err(|err| format!("{err}"))
        .map(|(content_range, data)| {
            let data_len = data.len();
            let body = axum::body::Body::from(data);
            let mut response = Response::new(body);
            let headers = response.headers_mut();
            let mut values = Vec::new();
            headers::AcceptRanges::bytes().encode(&mut values);
            assert_eq!(values.len(), 1);
            headers.insert(header::ACCEPT_RANGES, values.remove(0));
            let mut status = axum::http::StatusCode::OK;

            match content_range {
                Ok(content_range) => {
                    if !content_range
                        .bytes_len()
                        .is_some_and(|len| len == data_len as u64)
                    {
                        let mut values = Vec::new();
                        content_range.encode(&mut values);
                        assert_eq!(values.len(), 1);
                        headers.insert(header::CONTENT_RANGE, values.remove(0));
                        status = axum::http::StatusCode::PARTIAL_CONTENT;
                    }
                }
                Err(err) => {
                    eprintln!("content_range: {err}");
                }
            }
            headers.insert(
                header::CONTENT_LENGTH,
                data_len.to_string().parse().unwrap(),
            );
            *response.status_mut() = status;

            response
        })
}

pub async fn get_frame(axum::extract::Path(pts): axum::extract::Path<i64>) -> Response<Body> {
    if let Some(frame) = CLIP_CACHE.get_frame(pts) {
        let body = axum::body::Body::from(frame);
        let mut response = Response::new(body);
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, "image/bmp".parse().unwrap());
        return response;
    }
    let body = axum::body::Body::from(Bytes::new());
    Response::new(body)
}

pub async fn serve() {
    let app = axum::Router::new()
        .route("/frame/:pts", axum::routing::get(get_frame))
        .route("/*O", axum::routing::get(index));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    PORT.get_or_init(|| listener.local_addr().unwrap().port());
    println!("listening on: {}", get_file_server_address());
    axum::serve(listener, app).await.unwrap();
}

pub struct Cache {
    #[allow(clippy::type_complexity)]
    clip_cache: Mutex<Option<LruCache<Clip, Bytes>>>,
    frame_cache: Mutex<Option<LruCache<i64, Bytes>>>,
}

impl Cache {
    const fn new() -> Cache {
        Cache {
            clip_cache: Mutex::new(None),
            frame_cache: Mutex::new(None),
        }
    }

    fn get_clip(&self, clip: (f64, f64)) -> Result<Bytes, common::ffmpeg::Error> {
        let mut lock = self.clip_cache.lock().unwrap();
        let lru = lock.get_or_insert_with(|| LruCache::new(NonZeroUsize::new(20).unwrap()));
        if let Some(data) = lru.get(&clip.into()).map(Bytes::clone) {
            Self::limit_size(lru);
            Ok(data)
        } else {
            drop(lock);
            let path: std::path::PathBuf = {
                let lock = VIDEO_FILE.lock().unwrap();
                Option::clone(&lock).unwrap_or_default()
            };
            let mut decoder = VideoDecoder::open(&path, false)?;

            let key_frames = { KEYFRAMES.read().unwrap().clone().unwrap_or_default() };

            let result = decoder.transcode_range(clip.0, clip.1, &key_frames)?;
            let mut lock = self.clip_cache.lock().unwrap();
            let lru = lock.as_mut().unwrap();
            lru.push(clip.into(), Bytes::from(result));
            Self::limit_size(lru);
            Ok(Bytes::clone(lru.get(&clip.into()).unwrap()))
        }
    }

    fn get_frame(&self, pts: i64) -> Option<Bytes> {
        let mut lock = self.frame_cache.lock().unwrap();
        let Some(lru) = lock.as_mut() else {
            return None;
        };
        lru.get(&pts).cloned()
    }

    pub fn insert_frame(&self, mut preview_frame: PreviewFrame) -> Option<(i64, Bytes)> {
        let image = pixels_to_bmp(
            &mut preview_frame.pixels,
            preview_frame.width,
            preview_frame.height,
        );
        let mut lock = self.frame_cache.lock().unwrap();
        let lru = lock.get_or_insert_with(|| LruCache::new(NonZeroUsize::new(50).unwrap()));
        let old = lru.push(preview_frame.pts, image);
        drop(lock);
        old
    }

    fn limit_size(lru: &mut LruCache<Clip, Bytes>) {
        // compute size
        const HALF_GB: usize = 1_000_000 * 500;
        // keep lru less than half gb
        while lru.iter().map(|(_, d)| d.len()).sum::<usize>() > HALF_GB && lru.len() >= 2 {
            println!("remove clip from cache due to total size of cache");
            lru.pop_lru();
        }
    }
}

use crate::pixels_to_base64_image::pixels_to_bmp;
use axum::body::Body;
use std::cmp::Eq;
use std::hash::{Hash, Hasher};

#[derive(Debug)]
struct Timestamp(f64);

impl Timestamp {
    fn canonicalize(&self) -> i64 {
        (self.0 * 1024.0 * 1024.0).round() as i64
    }
}

impl PartialEq for Timestamp {
    fn eq(&self, other: &Timestamp) -> bool {
        self.canonicalize() == other.canonicalize()
    }
}

impl Eq for Timestamp {}

impl Hash for Timestamp {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.canonicalize().hash(state);
    }
}

#[derive(Eq, PartialEq, Hash)]
struct Clip(Timestamp, Timestamp);

impl From<(f64, f64)> for Clip {
    fn from(value: (f64, f64)) -> Self {
        Clip(Timestamp(value.0), Timestamp(value.1))
    }
}
