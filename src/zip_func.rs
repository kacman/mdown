use rand::{ seq::SliceRandom, thread_rng };
use std::{ fs::File, io::{ Read, Seek, Write }, path::Path };
use walkdir::{ DirEntry, WalkDir };
use zip::{ result::ZipError, write::FileOptions, ZipArchive };

use crate::{ ARGS, error, log, MAXPOINTS, resolute, string, utils::progress_bar_preparation };

fn zip_dir<T>(
    it: &mut dyn Iterator<Item = DirEntry>,
    prefix: &str,
    writer: T
) -> Result<(), error::MdownError>
    where T: Write + Seek
{
    let method = zip::CompressionMethod::Stored;
    let walkdir = WalkDir::new(prefix);
    let it_temp = &mut walkdir.into_iter().filter_map(|e| e.ok());
    let dir_entries_vec: Vec<DirEntry> = it_temp.collect();
    let total_items = dir_entries_vec.len();
    let start = MAXPOINTS.max_x / 3 - ((total_items / 2) as u32) - 1;
    progress_bar_preparation(start, total_items, 5);
    let mut zip = zip::ZipWriter::new(writer);
    let options = FileOptions::default().compression_method(method).unix_permissions(0o755);

    let mut buffer = Vec::new();
    let mut times = 0;
    for entry in it {
        let path = entry.path();
        let name = match path.strip_prefix(Path::new(prefix)) {
            Ok(name) => name,
            Err(err) => {
                return Err(error::MdownError::ConversionError(err.to_string()));
            }
        };
        if path.is_file() {
            string(5, start + times, "#");
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
                    return Err(error::MdownError::IoError(err, None));
                }
            };

            match f.read_to_end(&mut buffer) {
                Ok(_size) => (),
                Err(err) => {
                    return Err(error::MdownError::IoError(err, None));
                }
            }
            match zip.write_all(&buffer) {
                Ok(()) => (),
                Err(err) => {
                    return Err(error::MdownError::IoError(err, None));
                }
            }
            buffer.clear();
        } else if !name.as_os_str().is_empty() {
            #[allow(deprecated)]
            match zip.add_directory_from_path(name, options) {
                Ok(()) => (),
                Err(err) => {
                    return Err(error::MdownError::ZipError(err));
                }
            };
        }
        times += 1;
    }
    match zip.finish() {
        Ok(_writer) => (),
        Err(err) => {
            return Err(error::MdownError::ZipError(err));
        }
    }
    Ok(())
}

fn doit(src_dir: &str, dst_file: &str) -> Result<(), error::MdownError> {
    if !Path::new(src_dir).is_dir() {
        return Err(error::MdownError::ZipError(ZipError::FileNotFound));
    }
    let path = Path::new(dst_file);
    let file = match File::create(path) {
        Ok(file) => file,
        Err(err) => {
            return Err(error::MdownError::IoError(err, None));
        }
    };

    let walkdir = WalkDir::new(src_dir);
    let it = walkdir.into_iter();

    match zip_dir(&mut it.filter_map(|e| e.ok()), src_dir, file) {
        Ok(_) => (),
        Err(_err) => (),
    }

    Ok(())
}

pub(crate) fn to_zip(src_dir: &str, dst_file: &str) {
    if ARGS.web || ARGS.gui || ARGS.check || ARGS.update || ARGS.log || ARGS.server {
        log!(&format!("Zipping files to: {} ...", dst_file));
    }
    match doit(src_dir, dst_file) {
        Ok(_) => string(7, 0, format!("   done: {} written to {}", src_dir, dst_file).as_str()),
        Err(e) => eprintln!("  Error: {e:?}"),
    }
    if ARGS.web || ARGS.gui || ARGS.check || ARGS.update || ARGS.log || ARGS.server {
        log!(&format!("Zipping files to: {} Done", dst_file));
    }
}

pub(crate) fn extract_metadata_from_zip(
    zip_file_path: &str,
    metadata_file_name: &str
) -> Result<String, error::MdownError> {
    let file = match File::open(zip_file_path) {
        Ok(file) => file,
        Err(err) => {
            return Err(error::MdownError::IoError(err, Some(zip_file_path.to_string())));
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

        if file.name() == metadata_file_name {
            let mut content = String::new();
            match file.read_to_string(&mut content) {
                Ok(_) => (),
                Err(err) => {
                    return Err(
                        error::MdownError::IoError(err, Some(metadata_file_name.to_string()))
                    );
                }
            }
            return Ok(content);
        }
    }

    Err(
        error::MdownError::NotFoundError(
            format!("File '{}' not found in the zip archive", metadata_file_name)
        )
    )
}

pub(crate) fn extract_image_from_zip(zip_file_path: &str) -> Result<Vec<u8>, error::MdownError> {
    let file = match File::open(zip_file_path) {
        Ok(file) => file,
        Err(err) => {
            return Err(error::MdownError::IoError(err, Some(zip_file_path.to_string())));
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
                        return Err(error::MdownError::IoError(err, Some(file.name().to_string())));
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

pub(crate) fn extract_images_from_zip() -> Result<Vec<Vec<u8>>, error::MdownError> {
    let mut images = Vec::new();
    let mut files = match resolute::WEB_DOWNLOADED.lock() {
        Ok(file) => file.clone(),
        Err(err) => {
            return Err(error::MdownError::PoisonError(err.to_string()));
        }
    };
    files.truncate(10);

    for zip_file_path in files.iter() {
        if zip_file_path.ends_with(".cbz") {
            let file = match File::open(zip_file_path) {
                Ok(file) => file,
                Err(err) => {
                    return Err(error::MdownError::IoError(err, Some(zip_file_path.to_string())));
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
                                    error::MdownError::IoError(err, Some(file.name().to_string()))
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
