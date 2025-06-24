use std::collections::HashMap;
use std::path::Path;

use bincode::{Decode, Encode};

use crate::Args;

#[derive(Hash, Eq, PartialEq, Decode, Encode)]
pub struct CacheKey {
    pub include_spectating: bool,
    pub include_assists: bool,
}

#[derive(Decode, Encode)]
pub struct EclipperCache {
    pub file_size: u64,
    pub keyframes: Box<[f64]>,
    pub clips: HashMap<CacheKey, Box<[[f64; 2]]>>,
}

#[allow(unused)]
pub fn cache_clips(clips: Box<[[f64; 2]]>, keyframes: Box<[f64]>, args: &Args) {
    let Ok(metadata) = std::fs::metadata(&args.input) else {
        return;
    };
    let mut cache = if let Some(cache) = clips_from_cache(&args.input) {
        if cache.file_size == metadata.len() {
            cache
        } else {
            EclipperCache {
                file_size: metadata.len(),
                keyframes,
                clips: Default::default(),
            }
        }
    } else {
        EclipperCache {
            file_size: metadata.len(),
            keyframes,
            clips: Default::default(),
        }
    };

    let cache_key = CacheKey {
        include_spectating: args.include_spectating,
        include_assists: args.include_assists,
    };

    cache.clips.insert(cache_key, clips);

    let mut cache_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(args.input.with_extension("eclipper"));

    let Ok(mut cache_file) = cache_file else {
        return;
    };
    bincode::encode_into_std_write(cache, &mut cache_file, bincode::config::standard());
}

pub fn clips_from_cache(input: &Path) -> Option<EclipperCache> {
    let Ok(metadata) = std::fs::metadata(input) else {
        return None;
    };
    let mut cache_file = std::fs::File::open(input.with_extension("eclipper")).ok()?;

    let decoded: EclipperCache =
        bincode::decode_from_std_read(&mut cache_file, bincode::config::standard()).ok()?;

    if decoded.file_size != metadata.len() {
        return None;
    }

    Some(decoded)
}
