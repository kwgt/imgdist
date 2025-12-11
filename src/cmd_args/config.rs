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
use serde::Serialize;
use serde::Deserialize;

use super::LogLevel;

///
/// コンフィギュレーションデータを集約する構造体
///
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Config {
    /// ログ関連の情報の格納先
    log_info: LogInfo,

    /// パス情報の格納先
    path_info: PathInfo,

    /// キャッシュ情報の格納先
    cache_info: Option<CacheInfo>,
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

    ///
    /// キャッシュデータベースファイルのパスへのアクセサ
    ///
    /// # 戻り値
    /// キャッシュデータベースファイルのパス（未設定の場合はNone）
    ///
    pub(super) fn cache_db_path(&self) -> Option<PathBuf> {
        self.path_info.cache_db_path.clone()
    }

    ///
    /// キャッシュ評価モードへのアクセサ
    ///
    /// # 戻り値
    /// キャッシュ評価モード（未設定の場合はNone）
    ///
    pub(super) fn cache_eval_mode(&self) -> Option<super::CacheEvalMode> {
        self.cache_info
            .as_ref()
            .and_then(|info| info.cache_eval_mode)
    }
}

///
/// ログ設定を格納するサブ構造体
///
#[derive(Debug, Deserialize, Serialize)]
struct LogInfo {
    /// ログレベル
    level: Option<LogLevel>,

    /// ログ出力先
    output: Option<PathBuf>,
}

///
/// パス設定を格納するサブ構造体
///
#[derive(Debug, Default, Deserialize, Serialize)]
struct PathInfo {
    /// RAWファイルの格納先
    raw_output_path: Option<PathBuf>,

    /// 出力先
    output_path: Option<PathBuf>,

    /// キャッシュデータベースのパス
    cache_db_path: Option<PathBuf>,
}

///
/// キャッシュ情報を格納するサブ構造体
///
#[derive(Debug, Deserialize, Serialize)]
struct CacheInfo {
    /// キャッシュ評価モード
    cache_eval_mode: Option<super::CacheEvalMode>,
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

///
/// コンフィギュレーションファイルを書き出す
///
/// # 引数
/// * `path` - 出力先パス
/// * `config` - 出力する設定
///
/// # 戻り値
/// 書き込み結果
///
pub(crate) fn write<P>(path: P, config: &crate::cmd_args::Options) -> Result<()>
where
    P: AsRef<Path>,
{
    let mut path_info = PathInfo::default();
    path_info.output_path = Some(config.output_path());
    path_info.raw_output_path = config.raw_output_path();
    path_info.cache_db_path = Some(config.cache_db_path());

    let log_info = LogInfo {
        level: Some(config.log_level()),
        output: config.log_output(),
    };

    let cache_info = CacheInfo {
        cache_eval_mode: Some(config.cache_eval_mode()),
    };

    let cfg = Config {
        log_info,
        path_info,
        cache_info: Some(cache_info),
    };

    let toml = toml::to_string_pretty(&cfg)?;

    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(path, toml)?;
    Ok(())
}
