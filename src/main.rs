//
// Image file distributor
//
//  Copyright (C) 2025 Kuwagata HIROSHI <kgt9221@gmail.com>
//

//!
//! プログラムのエントリポイント
//!

mod cmd_args;

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use chrono::TimeZone;
use chrono::{DateTime, Local, NaiveDateTime};
use exif::{Exif, Tag};
use walkdir::WalkDir;

use crate::cmd_args::Options;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

///
/// プログラムのエントリポイント
///
/// # 注記
/// main()はエラー情報の集約のみを行い、実際の処理は実行処理に記述している。
///
fn main() {
    /*
     * コマンドラインオプションのパース
     */
    let opts = match cmd_args::parse() {
        Ok(opts) => opts,
        Err(err) => {
            eprintln!("error: {}", err);
            std::process::exit(1);
        },
    };

    /*
     * 実行関数の呼び出し
     */
    if let Err(err) = run(opts) {
        eprintln!("error: {}", err);
        std::process::exit(1);
    }
}

///
/// プログラムの実行関数
///
/// # 引数
/// * `opts` - オプション情報をパックしたオブジェクト
///
/// # 戻り値
/// プログラムが正常狩猟した場合は、`Ok(())`を返す。失敗した場合はエラー情報を
/// `Err()`でラップして返す。
///
fn run(opts: Arc<Options>) -> Result<()> {

    for entry in WalkDir::new(opts.input_path())
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension() {
                let ext = ext.to_string_lossy();

                if ext.eq_ignore_ascii_case("jpg")
                    || ext.eq_ignore_ascii_case("jpeg")
                    || ext.eq_ignore_ascii_case("dng")
                {
                    if let Err(err) =
                        distribute(entry.path(), opts.output_path())
                    {
                        error!("{}", err);
                    }
                }
            }
        }
    }

    Ok(())
}

fn distribute(src: impl AsRef<Path>, prefix: PathBuf) -> Result<()> {
    let src = src.as_ref();
    let exif = read_exif(&src)?;

    if let Some(field) =
        exif.get_field(Tag::DateTimeOriginal, exif::In::PRIMARY)
    {
        let datetime = parse_datetime(&(field.display_value().to_string()))?;
        let path = prefix
            .join(datetime.format("%Y").to_string())
            .join(datetime.format("%Y%m%d").to_string());
        let dst = path.join(src.file_name().unwrap());

        if !path.exists() {
            if let Err(err) = std::fs::create_dir_all(&path) {
                return Err(anyhow!("create directory failed: {}", err));
            }

            if !path.is_dir() {
                return Err(anyhow!("{} is not directory", path.display()));
            }
        }

        if let Err(err) = std::fs::copy(&src, &dst) {
            return Err(anyhow!("copy to {} failed: {}", dst.display(), err));
        }

        info!("copied {} to {}", src.display(), path.display());

    } else {
        warn!("not contained datetime info in {}", src.display());
    }

    Ok(())
}

fn read_exif(path: impl AsRef<Path>) -> Result<Exif> {
    let mut bufreader = BufReader::new(File::open(path.as_ref())?);

    match exif::Reader::new().read_from_container(&mut bufreader) {
        Ok(exif_data) => Ok(exif_data),
        Err(err) => Err(anyhow!("read exif failed: {}", err)),
    }
}

fn parse_datetime(s: &str) -> Result<DateTime<Local>> {
    match NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        Ok(datetime) => Ok(Local.from_local_datetime(&datetime).unwrap()),
        Err(err) => return Err(anyhow!("datetime parse failed: {}", err)),
    }
}
