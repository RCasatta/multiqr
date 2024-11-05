use clap::Parser;
use std::io::Read;

// TODO how to show multiple lines in clap help?

/// The `multiqr` utility accept an ascii string (trimming newlines) from std input and convert it to one or more QR codes.
///
/// It's more efficient to use the following characters for QR code efficiency:
/// 0–9, A–Z (upper-case only), space, $, %, *, +, -, ., /, :
///
/// To achieve good efficiency starting with binary data, one option is to use the `base32` utility. Even if the padding use `=` which is not in the QR code alphanumeric mode, the QR code library split the data and use the binary representation only for the final padding.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[clap(verbatim_doc_comment)]
pub struct Params {
    /// Max QR code version to use.
    #[arg(long, default_value_t = 16)]
    qr_version: u8,

    /// Modules at the border of the QR code
    #[arg(long, default_value_t = 4)]
    border: u8,

    /// Number of empty lines between one QR and the following
    #[arg(long, default_value_t = 6)]
    empty_lines: u8,

    #[arg(long)]
    label: Option<String>,
}

fn main() {
    match inner_main() {
        Err(Error::Other(s)) => println!("{s}"),
        Err(e) => println!("{e:?}"),
        Ok(_) => (),
    }
}

fn inner_main() -> Result<(), Error> {
    let params = Params::parse();
    let stdin = read_stdin().map_err(|e| Error::Other(e))?;
    let qr = qr(
        &stdin,
        params.qr_version,
        params.border,
        params.empty_lines,
        params.label,
    )?;
    println!("{qr}");
    Ok(())
}

pub fn read_stdin() -> Result<Vec<u8>, &'static str> {
    let mut stdin = std::io::stdin().lock();
    let mut buffer = vec![];
    stdin
        .read_to_end(&mut buffer)
        .map_err(|_| "error reading stdin")?;
    let mut result = vec![];

    for el in buffer.into_iter().filter(|e| *e != b'\n') {
        let c = char::from(el);
        if !c.is_ascii() {
            return Err("Standard input contains non ascii chars");
        }
        if c.is_ascii_control() {
            return Err("Standard input contains ascii control chars");
        }
        result.push(el);
    }
    Ok(result)
}

use qr_code::{types::QrError, QrCode, Version};

#[derive(Debug)]
pub enum Error {
    Qr(QrError),
    Other(&'static str),
}

fn qr(
    content: &[u8],
    version: u8,
    border: u8,
    empty_lines: u8,
    label: Option<String>,
) -> Result<String, Error> {
    let chunk_size = estimate_chunk(content, version).map_err(|e| Error::Other(e))?;

    let mut result = String::new();
    let empty_lines = "\n".repeat(empty_lines as usize);
    let label = label.as_deref().unwrap_or("");

    let splitted_data = content.chunks(chunk_size).collect::<Vec<_>>();
    let len = splitted_data.len();
    for (i, data) in splitted_data.iter().enumerate() {
        let qr = QrCode::new(data).map_err(|e| Error::Qr(e))?;
        print_qr(i, &qr, border, &mut result, len, label);
        if i < len - 1 {
            result.push_str(&empty_lines);
        }
    }

    Ok(result)
}

/// Find the lenght of the chunk of data given the desired version of the QR
///
/// Consider the data omogenous, ie if first part is more efficiently represented in the QR code not every QR code generated from chunks may be equal
fn estimate_chunk(content: &[u8], desired_version: u8) -> Result<usize, &'static str> {
    if desired_version == 0 || desired_version > 40 {
        return Err("Invalid version");
    }
    if content.len() == 0 {
        return Err("Invalid empty content");
    }

    let desired_version = desired_version as i16;
    let mut total = content.len();
    let chunk_size = loop {
        match QrCode::new(&content[..total]) {
            Ok(qr) => {
                let width = match qr.version() {
                    Version::Normal(w) => w,
                    Version::Micro(_) => panic!("micro"),
                };
                // println!("version:{} desired:{}", width, desired_version);

                if width < desired_version && total >= content.len() {
                    // the QR version of the full content is smaller than the desired version
                    return Ok(content.len());
                }

                if width == desired_version {
                    break total;
                }
                total = if width > desired_version {
                    total / 2
                } else {
                    (total * 3) / 2
                };

                if total >= content.len() {
                    return Ok(content.len());
                }
            }
            Err(QrError::DataTooLong) => {
                total /= 2;
            }
            Err(_) => {
                panic!("should not happen");
            }
        }
    };

    // Make chunks more similar instead of having the last one shorter
    let pieces = (content.len() / chunk_size) + 1;
    let new_chunk_size = (content.len() / pieces) + 1;

    Ok(new_chunk_size)
}

fn print_qr(i: usize, qr: &QrCode, border: u8, result: &mut String, len: usize, label: &str) {
    let version = match qr.version() {
        qr_code::Version::Normal(x) => x,
        qr_code::Version::Micro(x) => -x,
    };
    let number = format!("{} ({}/{len}) v{:?}\n", label, i + 1, version);
    let qr_width_with_border = qr.width() + border as usize * 2;
    let spaces = " ".repeat((qr_width_with_border.saturating_sub(number.len())) / 2);

    result.push_str(&spaces);
    result.push_str(&number);

    result.push_str(&qr.to_string(true, border));
}

#[cfg(test)]
mod test {
    use super::estimate_chunk;
    use rand::prelude::*;

    #[test]
    fn test_estimate_chunk() {
        let mut rng = rand::thread_rng();
        let data = ['x' as u8; u16::MAX as usize];

        for _ in 1..100 {
            let size = rng.gen::<u16>() as usize;
            let data = &data[..size];
            let version: u8 = rng.gen::<u8>() % 40 + 1;
            let chunk = estimate_chunk(data.as_ref(), version).unwrap();
            println!("size:{size} chunk:{chunk} version:{version}");

            assert!(chunk <= size);
            assert!(chunk > 0);
        }
    }
}
