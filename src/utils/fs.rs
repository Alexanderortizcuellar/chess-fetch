use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

pub fn sanitize_filename(name: &str) -> String {
    let invalid_chars = ['<', '>', ':', '"', '/', '\\', '|', '?', '*', '\0'];
    name.chars()
        .map(|c| if invalid_chars.contains(&c) { '_' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn save_text_to_file<P: AsRef<Path>>(path: P, content: &str) -> Result<()> {
    let p = path.as_ref();
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
    }
    let mut file =
        File::create(p).with_context(|| format!("Failed to create file: {}", p.display()))?;
    file.write_all(content.as_bytes())
        .with_context(|| format!("Failed to write to file: {}", p.display()))?;
    Ok(())
}

#[allow(dead_code)]
pub fn append_text_to_file<P: AsRef<Path>>(path: P, content: &str) -> Result<()> {
    let p = path.as_ref();
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(p)
        .with_context(|| format!("Failed to open file for appending: {}", p.display()))?;
    file.write_all(content.as_bytes())
        .with_context(|| format!("Failed to append to file: {}", p.display()))?;
    Ok(())
}

pub fn save_bytes_to_file<P: AsRef<Path>>(path: P, bytes: &[u8]) -> Result<()> {
    let p = path.as_ref();
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
    }
    let mut file =
        File::create(p).with_context(|| format!("Failed to create file: {}", p.display()))?;
    file.write_all(bytes)
        .with_context(|| format!("Failed to write bytes to: {}", p.display()))?;
    Ok(())
}

#[allow(dead_code)]
pub fn extract_zip_bytes(zip_data: &[u8], target_dir: &Path) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(target_dir).with_context(|| {
        format!(
            "Failed to create output directory: {}",
            target_dir.display()
        )
    })?;

    let cursor = Cursor::new(zip_data);
    let mut archive = zip::ZipArchive::new(cursor).context("Failed to parse zip archive")?;

    let mut extracted_files = Vec::new();

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .with_context(|| format!("Failed to read entry {} in zip", i))?;

        let outpath = match file.enclosed_name() {
            Some(path) => target_dir.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)?;
                }
            }
            let mut outfile = File::create(&outpath).with_context(|| {
                format!("Failed to create extracted file: {}", outpath.display())
            })?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            outfile.write_all(&buffer)?;
            extracted_files.push(outpath);
        }
    }

    Ok(extracted_files)
}

pub fn extract_single_pgn_from_zip(zip_data: &[u8]) -> Result<String> {
    let cursor = Cursor::new(zip_data);
    let mut archive = zip::ZipArchive::new(cursor).context("Failed to parse zip archive")?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_lowercase();
        if name.ends_with(".pgn") {
            let mut content = String::new();
            file.read_to_string(&mut content)
                .context("Failed to read PGN content from zip entry")?;
            return Ok(content);
        }
    }

    // If no .pgn extension was found, read the first file as text
    if !archive.is_empty() {
        let mut file = archive.by_index(0)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        return Ok(content);
    }

    anyhow::bail!("Zip archive is empty or contains no PGN file");
}
