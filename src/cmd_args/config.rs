//
// Image file distributor
//
//  Copyright (C) 2025 Hiroshi KUWAGATA <kgt9221@gmail.com>
//

//!
//! コンフィギュレーションファイル関連の処理をまとめたモジュール
//!

use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use serde::Deserialize;

use super::LogLevel;

///
/// コンフィギュレーションデータを集約する構造体
///
#[derive(Debug, Deserialize)]
pub(super) struct Config {
    /// ログ関連の情報の格納先
    log_info: LogInfo,

    /// パス情報の格納先
    path_info: PathInfo,
}

impl Config {
    ///
    /// ログレベルへのアクセサ
    ///
    pub(super) fn log_level(&self) -> Option<LogLevel> {
        self.log_info.level.clone()
    }

    ///
    /// ログの出力先へのアクセサ
    ///
    pub(super) fn log_output(&self) -> Option<PathBuf> {
        self.log_info.output.clone()
    }

    ///
    /// RAWファイル格納先へのアクセサ
    ///
    pub(super) fn raw_output_path(&self) -> Option<PathBuf> {
        self.path_info.raw_output_path.clone()
    }

    ///
    /// ファイル出力先へのアクセサ
    ///
    pub(super) fn output_path(&self) -> Option<PathBuf> {
        self.path_info.output_path.clone()
    }
}

///
/// ログ設定を格納するサブ構造体
///
#[derive(Debug, Deserialize)]
struct LogInfo {
    /// ログレベル
    level: Option<LogLevel>,

    /// ログ出力先
    output: Option<PathBuf>,
}

///
/// パス設定を格納するサブ構造体
///
#[derive(Debug, Deserialize)]
struct PathInfo {
    /// RAWファイルの格納先
    raw_output_path: Option<PathBuf>,

    /// 出力先
    output_path: Option<PathBuf>,
}

///
/// コンフィギュレーションファイルの読み込み
///
pub(super) fn read<P>(path: P) -> Result<Config> 
where 
    P: AsRef<Path>
{
    Ok(toml::from_str(&std::fs::read_to_string(path)?)?)
}
