use anyhow::{Result, ensure};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
};

const MAGIC: &[u8; 16] = b"SBL_PAYLOAD_V001";
const FOOTER_SIZE: u64 = 56;

// Packaging appends ZIP bytes and a fixed footer to the linked PE executable.
// Read only on installation, so opening the UI does not load the service archive.
pub fn read() -> Result<Vec<u8>> {
    let mut executable = File::open(std::env::current_exe()?)?;
    read_from(&mut executable)
}

fn read_from(input: &mut (impl Read + Seek)) -> Result<Vec<u8>> {
    let size = input.seek(SeekFrom::End(0))?;
    ensure!(
        size >= FOOTER_SIZE,
        "No bundled runtime. Build with scripts/build-installer.ps1."
    );
    input.seek(SeekFrom::End(-(FOOTER_SIZE as i64)))?;
    let mut footer = [0u8; FOOTER_SIZE as usize];
    input.read_exact(&mut footer)?;
    ensure!(
        &footer[..16] == MAGIC,
        "No bundled runtime. Build with scripts/build-installer.ps1."
    );
    let length = u64::from_le_bytes(footer[16..24].try_into()?);
    ensure!(
        length > 0 && length <= size - FOOTER_SIZE,
        "Invalid bundled runtime length"
    );
    input.seek(SeekFrom::Start(size - FOOTER_SIZE - length))?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(usize::try_from(length)?)?;
    input.take(length).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 == length && Sha256::digest(&bytes).as_slice() == &footer[24..],
        "Bundled runtime checksum mismatch"
    );
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    #[test]
    fn reads_bundle_and_rejects_corruption_or_missing_footer() {
        let payload = b"payload archive";
        let mut bytes = b"executable stub".to_vec();
        bytes.extend_from_slice(payload);
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&Sha256::digest(payload));
        assert_eq!(read_from(&mut Cursor::new(&bytes)).unwrap(), payload);
        bytes[15] ^= 1;
        assert!(read_from(&mut Cursor::new(&bytes)).is_err());
        assert!(read_from(&mut Cursor::new(b"unbundled executable")).is_err());
    }
}
