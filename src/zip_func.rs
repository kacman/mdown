use std::{ fs::File, io::{ Read, Seek, Write }, path::Path };
use walkdir::{ DirEntry, WalkDir };
use zip::{ result::ZipError, write::FileOptions, ZipArchive };

use crate::{
    args,
    error,
    log,
    MAXPOINTS,
    metadata,
    string,
    utils::{ self, progress_bar_preparation },
};

/// Compresses a directory and its contents into a ZIP file.
///
/// # Parameters
/// - `it: &mut dyn Iterator<Item = DirEntry>`: Iterator over the directory entries.
/// - `prefix: &str`: The base directory path to be compressed.
/// - `writer: T`: The writer to which the ZIP file data will be written.
///
/// # Returns
/// `Result<(), MdownError>`: Returns `Ok(())` if the operation is successful, or an `MdownError` if an error occurs.
///
/// # Panics
/// This function will panic if:
/// - The total number of items in the directory exceeds `usize::MAX`, causing an overflow in the `len` function.
/// - The path conversion to `&str` fails unexpectedly when using `strip_prefix`, which is unlikely unless there's a serious internal error in `Path` handling.
fn zip_dir<T>(
    it: &mut dyn Iterator<Item = DirEntry>,
    prefix: &str,
    writer: T
) -> Result<(), error::MdownError>
    where T: Write + Seek
{
    let method = zip::CompressionMethod::Stored;
    let walkdir = WalkDir::new(prefix);
    let dir_entries_vec: Vec<DirEntry> = walkdir
        .into_iter()
        .filter_map(|e| e.ok())
        .collect();
    let total_items = dir_entries_vec.len();

    // Determine the starting position for the progress bar.
    let start = if MAXPOINTS.max_x / 3 < ((total_items / 2) as u32) - 1 {
        1
    } else {
        MAXPOINTS.max_x / 3 - ((total_items / 2) as u32) - 1
    };
    progress_bar_preparation(start, total_items, 5);

    // Initialize the ZIP writer and file options.
    let mut zip = zip::ZipWriter::new(writer);
    let options = FileOptions::default().compression_method(method).unix_permissions(0o755);

    let mut buffer = Vec::new();
    for (times, entry) in it.enumerate() {
        let path = entry.path();
        let name = match path.strip_prefix(Path::new(prefix)) {
            Ok(name) => name,
            Err(err) => {
                return Err(error::MdownError::ConversionError(err.to_string()));
            }
        };

        // If the path is a file, compress it.
        if path.is_file() {
            string(5, start + (times as u32), "#");
            #[allow(deprecated)]
            match zip.start_file_from_path(name, options) {
                Ok(()) => (),
                Err(err) => {
                    return Err(error::MdownError::ZipError(err));
                }
            }
            let mut f = match File::open(path) {
                Ok(file) => file,
                Err(err) => {
                    return Err(error::MdownError::IoError(err, String::new()));
                }
            };

            // Read file content into the buffer and write it to the ZIP archive.
            match f.read_to_end(&mut buffer) {
                Ok(_size) => (),
                Err(err) => {
                    return Err(error::MdownError::IoError(err, String::new()));
                }
            }
            match zip.write_all(&buffer) {
                Ok(()) => (),
                Err(err) => {
                    return Err(error::MdownError::IoError(err, String::new()));
                }
            }
            buffer.clear();

            // If the path is a directory, add it to the ZIP archive.
        } else if !name.as_os_str().is_empty() {
            #[allow(deprecated)]
            match zip.add_directory_from_path(name, options) {
                Ok(()) => (),
                Err(err) => {
                    return Err(error::MdownError::ZipError(err));
                }
            };
        }
    }

    // Finalize the ZIP archive.
    match zip.finish() {
        Ok(_writer) => (),
        Err(err) => {
            return Err(error::MdownError::ZipError(err));
        }
    }
    Ok(())
}

/// Creates a ZIP file from a directory.
///
/// # Parameters
/// - `src_dir: &str`: The source directory to be compressed.
/// - `dst_file: &str`: The destination ZIP file path.
///
/// # Returns
/// `Result<(), MdownError>`: Returns `Ok(())` if the operation is successful, or an `MdownError` if an error occurs.
///
/// # Panics
/// This function will panic if:
/// - The directory path or file path cannot be represented as valid UTF-8 strings, though this is very unlikely.
fn doit(src_dir: &str, dst_file: &str) -> Result<(), error::MdownError> {
    // Check if the source directory exists.
    if !Path::new(src_dir).is_dir() {
        return Err(error::MdownError::ZipError(ZipError::FileNotFound));
    }
    let path = Path::new(dst_file);
    let file = match File::create(path) {
        Ok(file) => file,
        Err(err) => {
            return Err(error::MdownError::IoError(err, String::new()));
        }
    };

    // Walk through the directory and zip its contents.
    let walkdir = WalkDir::new(src_dir);
    let it = walkdir.into_iter();

    match zip_dir(&mut it.filter_map(|e| e.ok()), src_dir, file) {
        Ok(_) => (),
        Err(_err) => (),
    }

    Ok(())
}

/// Public interface for zipping a directory.
///
/// # Parameters
/// - `src_dir: &str`: The source directory to be compressed.
/// - `dst_file: &str`: The destination ZIP file path.
///
/// This function handles the zipping process and logs the operation based on certain conditions.
///
/// # Panics
/// This function will panic if:
/// - The directory path or file path cannot be represented as valid UTF-8 strings, though this is very unlikely.
pub(crate) fn to_zip(src_dir: &str, dst_file: &str) {
    if
        *args::ARGS_WEB ||
        *args::ARGS_GUI ||
        *args::ARGS_CHECK ||
        *args::ARGS_UPDATE ||
        *args::ARGS_LOG ||
        *args::ARGS_SERVER
    {
        log!(&format!("Zipping files to: {} ...", dst_file));
    }
    match doit(src_dir, dst_file) {
        Ok(_) => string(7, 0, format!("   done: {} written to {}", src_dir, dst_file).as_str()),
        Err(e) => eprintln!("  Error: {e:?}"),
    }
    if
        *args::ARGS_WEB ||
        *args::ARGS_GUI ||
        *args::ARGS_CHECK ||
        *args::ARGS_UPDATE ||
        *args::ARGS_LOG ||
        *args::ARGS_SERVER
    {
        log!(&format!("Zipping files to: {} Done", dst_file));
    }
}

/// Extracts a specific file from a ZIP archive.
///
/// # Parameters
/// - `zip_file_path: &str`: The path to the ZIP file.
/// - `metadata_file_name: &str`: The name of the file to extract from the ZIP archive.
///
/// # Returns
/// `Result<metadata::ChapterMetadataIn, MdownError>`: Returns the extracted file's metadata content if successful, or an `MdownError` if an error occurs.
///
/// # Panics
/// This function does not explicitly panic, but improper usage of the underlying filesystem or ZIP library could cause a panic in rare cases, such as invalid file paths or corrupted ZIP files.
pub(crate) fn extract_file_from_zip(
    zip_file_path: &str,
    metadata_file_name: &str
) -> Result<metadata::ChapterMetadataIn, error::MdownError> {
    let zip_file = match File::open(zip_file_path) {
        Ok(zip_file) => zip_file,
        Err(err) => {
            return Err(error::MdownError::IoError(err, zip_file_path.to_string()));
        }
    };
    let mut archive = match ZipArchive::new(zip_file) {
        Ok(archive) => archive,
        Err(err) => {
            return Err(error::MdownError::ZipError(err));
        }
    };

    let answer = match archive.by_name(metadata_file_name).map_err(error::MdownError::ZipError) {
        Ok(mut file) => {
            let mut metadata_content = String::new();
            match file.read_to_string(&mut metadata_content) {
                Ok(_) => (),
                Err(err) => {
                    return Err(error::MdownError::IoError(err, metadata_file_name.to_string()));
                }
            }
            let json_value = match utils::get_json(&metadata_content) {
                Ok(value) => value,
                Err(err) => {
                    return Err(err);
                }
            };
            match serde_json::from_value::<metadata::ChapterMetadataIn>(json_value) {
                Ok(obj) => {
                    return Ok(obj);
                }
                Err(err) => {
                    return Err(error::MdownError::JsonError(err.to_string()));
                }
            }
        }
        Err(_err) => {
            Err(
                error::MdownError::NotFoundError(
                    format!("File '{}' not found in the zip archive", metadata_file_name)
                )
            )
        }
    };
    answer
}

/// Extracts an image from a ZIP archive.
///
/// # Parameters
/// - `zip_file_path: &str`: The path to the ZIP file.
///
/// # Returns
/// `Result<Vec<u8>, MdownError>`: Returns the image content as a vector of bytes if successful, or an `MdownError` if an error occurs.
///
/// # Panics
/// This function does not explicitly panic, but improper usage of the underlying filesystem or ZIP library could cause a panic in rare cases, such as invalid file paths or corrupted ZIP files.
#[cfg(feature = "server")]
pub(crate) fn extract_image_from_zip(zip_file_path: &str) -> Result<Vec<u8>, error::MdownError> {
    let zip_file = match File::open(zip_file_path) {
        Ok(zip_file) => zip_file,
        Err(err) => {
            return Err(error::MdownError::IoError(err, zip_file_path.to_string()));
        }
    };
    let mut archive = match ZipArchive::new(zip_file) {
        Ok(archive) => archive,
        Err(err) => {
            return Err(error::MdownError::ZipError(err));
        }
    };

    for i in 0..archive.len() {
        let mut file = match archive.by_index(i) {
            Ok(file) => file,
            Err(err) => {
                return Err(error::MdownError::ZipError(err));
            }
        };
        if let Some(file_name) = file.name().to_lowercase().split('.').last() {
            match file_name {
                "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" => {
                    let mut content = Vec::new();
                    if let Err(err) = file.read_to_end(&mut content) {
                        return Err(error::MdownError::IoError(err, file.name().to_string()));
                    }
                    return Ok(content);
                }
                _ => {
                    continue;
                }
            }
        }
    }

    Err(error::MdownError::NotFoundError("File not found in the zip archive".to_owned()))
}

/// Extracts multiple images from a set of ZIP files, selecting up to 10 images randomly.
///
/// # Returns
/// `Result<Vec<Vec<u8>>, MdownError>`: Returns a vector of image contents (each as a vector of bytes) if successful, or an `MdownError` if an error occurs.
///
/// # Panics
/// This function does not explicitly panic, but improper usage of the underlying filesystem or ZIP library could cause a panic in rare cases, such as invalid file paths or corrupted ZIP files.
#[cfg(feature = "web")]
pub(crate) fn extract_images_from_zip() -> Result<Vec<Vec<u8>>, error::MdownError> {
    use crate::resolute;
    use rand::{ seq::SliceRandom, thread_rng };
    let mut images = Vec::new();
    let mut files = resolute::WEB_DOWNLOADED.lock().clone();
    files.truncate(10);

    for zip_file_path in files.iter() {
        if zip_file_path.ends_with(".cbz") {
            let file = match File::open(zip_file_path) {
                Ok(file) => file,
                Err(err) => {
                    return Err(error::MdownError::IoError(err, zip_file_path.to_string()));
                }
            };
            let mut archive = match ZipArchive::new(file) {
                Ok(archive) => archive,
                Err(err) => {
                    return Err(error::MdownError::ZipError(err));
                }
            };

            for i in 0..archive.len() {
                let mut file = match archive.by_index(i) {
                    Ok(file) => file,
                    Err(err) => {
                        return Err(error::MdownError::ZipError(err));
                    }
                };
                if let Some(file_name) = file.name().to_lowercase().split('.').last() {
                    match file_name {
                        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" => {
                            let mut content = Vec::new();
                            if let Err(err) = file.read_to_end(&mut content) {
                                return Err(
                                    error::MdownError::IoError(err, file.name().to_string())
                                );
                            }
                            images.push(content);
                        }
                        _ => {
                            continue;
                        }
                    }
                }
            }
        }
    }

    let mut rng = thread_rng();
    images.shuffle(&mut rng);
    images.truncate(10);
    Ok(images)
}
