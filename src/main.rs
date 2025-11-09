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
use exif::{Exif, Tag, Field};
use walkdir::{DirEntry, WalkDir};

use crate::cmd_args::Options;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

/// ファイルタイプと保存先パスを表す列挙型
#[derive(Debug, Clone, PartialEq)]
enum FileType {
    /// JPEGファイル（保存先パス）
    Jpeg(PathBuf),
    /// RAWファイル（保存先パス）
    Raw(PathBuf),
}

/// 拡張子からRAWファイルかどうかを判定する
///
/// # 引数
/// * `ext` - ファイルの拡張子
///
/// # 戻り値
/// RAWファイルの場合は`true`、そうでなければ`false`
fn is_raw_file(ext: &str) -> bool {
    matches!(ext.to_lowercase().as_str(), 
        "dng" |
        "nef" |
        "cr2" |
        "arw" |
        "orf" |
        "rw2" |
        "pef" |
        "srw" |
        "raf" |
        "3fr" |
        "fff" |
        "x3f"
    )
}

/// 拡張子からファイルタイプと保存先パスを構築する
///
/// # 引数
/// * `ext` - ファイルの拡張子
/// * `datetime` - 撮影日時
/// * `jpeg_output` - JPEGファイルの出力ディレクトリ
/// * `raw_output` - RAWファイルの出力ディレクトリ（オプション）
///
/// # 戻り値
/// 判定されたファイルタイプと保存先パス、または`None`（サポートされていない形式）
fn build_file_type(ext: &str, datetime: &DateTime<Local>, opts: &Options)
    -> Option<FileType>
{
    let ext_lower = ext.to_lowercase();
    let year = datetime.format("%Y").to_string();
    let date = datetime.format("%Y%m%d").to_string();
    let jpeg_output = opts.output_path();
    let raw_output = opts.raw_output_path();
    
    match ext_lower.as_str() {
        "jpg" | "jpeg" => {
            Some(FileType::Jpeg(jpeg_output.join(year).join(date)))
        },

        _ if is_raw_file(&ext_lower) => {
            let base_path = if let Some(raw_dir) = raw_output {
                raw_dir.join(year).join(date)
            } else {
                jpeg_output.join(year).join(date)
            };

            Some(FileType::Raw(base_path))
        },

        _ => None,
    }
}

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

    if opts.is_show_options() {
        opts.show_options();
        std::process::exit(0);
    }

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
        .filter_entry(|e| !is_shadow(e))
        .filter_map(Result::ok)
    {
        if entry.file_type().is_file() {
            if let Some(_) = entry.path().extension() {
                if let Err(err) = process_file(entry.path(), &opts) {
                    error!("{}", err);
                }
            }
        }
    }

    Ok(())
}

fn is_shadow(entry: &DirEntry) -> bool {
    if let Some(name) = entry.file_name().to_str() {
        if name.starts_with("._") {
            return true;
        }

        if name == ".DS_Store" {
            return true;
        }

        if name == ".AppleDouble" {
            return true;
        }

        if name == ".Trashes" {
            return true;
        }

        if name == ".Spotlight-V100" {
            return true;
        }

        if name == ".fseventsd" {
            return true;
        }

        if name == ".TemporaryItems" {
            return true;
        }
    }

    return false;
}

/// ファイルを処理する（ファイルタイプ判定とパス構築を含む）
///
/// # 引数
/// * `src` - 処理するファイルのパス
/// * `opts` - オプション設定の参照
///
/// # 戻り値
/// 処理が成功した場合は`Ok(())`、失敗した場合はエラー情報を `Err()`でラップして
/// 返す
fn process_file(src: impl AsRef<Path>, opts: &Options) -> Result<()> {
    let src = src.as_ref();
    
    // 拡張子を取得
    let ext = match src.extension() {
        Some(ext) => ext.to_string_lossy(),
        None => return Ok(()), // 拡張子がない場合はスキップ
    };
    
    // Exif情報を読み取り
    let exif = read_exif(src)?;
    
    // 撮影日時を取得
    let datetime = if let Some(field) = get_datetime_field(&exif) {
        parse_datetime(&(field.display_value().to_string()))?
    } else {
        warn!("not contained datetime info in {}", src.display());
        return Ok(());
    };
    
    // 日付範囲のチェック
    if !is_date_in_range(&datetime, &opts) {
        debug!(
            "skipping {} (date {} is out of range)",
            src.display(),
            datetime.date_naive()
        );

        return Ok(());
    }
    
    // ファイルタイプと保存先パスを構築
    if let Some(file_type) = build_file_type(&ext, &datetime, &opts) {
        distribute(src, file_type)?;
    }
    
    Ok(())
}

/// Exif情報から撮影日時フィールドを取得する
///
/// # 引数
/// * `exif` - Exif情報を格納したオブジェクトの参照
///
/// # 戻り値
/// 撮影日時フィールドが存在する場合は`Some(&Field)`を返す。存在しない場合は
/// `None`を返す。
fn get_datetime_field(exif: &Exif) -> Option<&Field> {
    exif.get_field(Tag::DateTimeOriginal, exif::In::PRIMARY)
}

/// 撮影日時が指定された日付範囲内かどうかを判定する
///
/// # 引数
/// * `datetime` - 撮影日時
/// * `opts` - オプション設定の参照
///
/// # 戻り値
/// 日付範囲内の場合は`true`、範囲外の場合は`false`
fn is_date_in_range(datetime: &DateTime<Local>, opts: &Options) -> bool {
    // 始点のチェック
    if let Some(from_date) = opts.from_date() {
        if datetime.date_naive() < from_date.date_naive() {
            return false;
        }
    }
    
    // 終点のチェック
    if let Some(to_date) = opts.to_date() {
        if datetime.date_naive() >= to_date.date_naive() {
            return false;
        }
    }
    
    true
}

/// ファイルを指定されたパスにコピーする
///
/// # 引数
/// * `src` - コピー元ファイルのパス
/// * `file_type` - ファイルタイプと保存先パス
///
/// # 戻り値
/// 処理が成功した場合は`Ok(())`、失敗した場合はエラー情報を `Err()`でラップして
/// 返す
fn distribute(src: impl AsRef<Path>, file_type: FileType) -> Result<()> {
    let src = src.as_ref();
    
    // 保存先パスを取得
    let target_path = match file_type {
        FileType::Jpeg(path) | FileType::Raw(path) => path,
    };
    
    let dst = target_path.join(src.file_name().unwrap());

    // ディレクトリが存在しない場合は作成
    if !target_path.exists() {
        if let Err(err) = std::fs::create_dir_all(&target_path) {
            return Err(anyhow!("create directory failed: {}", err));
        }

        if !target_path.is_dir() {
            return Err(anyhow!("{} is not directory", target_path.display()));
        }
    }

    // ファイルをコピー
    if let Err(err) = std::fs::copy(&src, &dst) {
        return Err(anyhow!("copy to {} failed: {}", dst.display(), err));
    }

    info!("copied {} to {}", src.display(), target_path.display());

    Ok(())
}

fn read_exif(path: impl AsRef<Path>) -> Result<Exif> {
    let mut bufreader = BufReader::new(File::open(path.as_ref())?);

    match exif::Reader::new().read_from_container(&mut bufreader) {
        Ok(exif_data) => Ok(exif_data),
        Err(err) => Err(anyhow!(
            "read exif failed {}: {}",
            path.as_ref().display(),
            err
        )),
    }
}

fn parse_datetime(s: &str) -> Result<DateTime<Local>> {
    match NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        Ok(datetime) => Ok(Local.from_local_datetime(&datetime).unwrap()),
        Err(err) => return Err(anyhow!("datetime parse failed: {}", err)),
    }
}
