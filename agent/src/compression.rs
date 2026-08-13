use anyhow::Result;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::Write;

/// Compress HTTP payloads before uploading batches of logs.
///
/// Sender can attach the returned bytes with:
/// Content-Encoding: gzip
pub fn gzip_compress(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

pub fn should_compress(size: usize) -> bool {
    size >= 1024
}
