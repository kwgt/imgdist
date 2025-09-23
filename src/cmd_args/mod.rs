//
// Image file distributor
//
//  Copyright (C) 2025 Hiroshi KUWAGATA <kgt9221@gmail.com>
//

//!
//! コマンドラインオプション関連の処理をまとめたモジュール
//!

mod logger;

use std::sync::Arc;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::{Parser, ValueEnum};

///
/// ログレベルを指し示す列挙子
///
#[derive(Debug, Clone, Copy, PartialEq, ValueEnum)]
#[clap(rename_all = "SCREAMING_SNAKE_CASE")]
enum LogLevel {
    /// ログを記録しない
    Off,

    /// エラー情報以上のレベルを記録
    Error,

    /// 警告情報以上のレベルを記録
    Warn,

    /// 一般情報以上のレベルを記録
    Info,

    /// デバッグ情報以上のレベルを記録
    Debug,

    /// トレース情報以上のレベルを記録
    Trace,
}

// Intoトレイトの実装
impl Into<log::LevelFilter> for LogLevel {
    fn into(self) -> log::LevelFilter {
        match self {
            Self::Off => log::LevelFilter::Off,
            Self::Error => log::LevelFilter::Error,
            Self::Warn => log::LevelFilter::Warn,
            Self::Info => log::LevelFilter::Info,
            Self::Debug => log::LevelFilter::Debug,
            Self::Trace => log::LevelFilter::Trace,
        }
    }
}

// AsRefトレイトの実装
impl AsRef<str> for LogLevel {
    fn as_ref(&self) -> &str {
        match self {
            Self::Off => "none",
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

///
/// コマンドラインオプションをまとめた構造体
///
#[derive(Parser, Debug, Clone)]
#[command(about = "Logger for environment sensor")]
#[command(version = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("GIT_COMMIT_HASH"),
    ")",
))]
#[command(long_about = None)]
pub(crate) struct Options {
    /// 記録するログレベルの指定
    #[arg(short = 'l', long = "log-level", value_name = "LEVEL",
        default_value = "INFO", ignore_case = true)]
    log_level: LogLevel,

    /// ログの出力先の指定
    #[arg(short = 'L', long = "log-output", value_name = "PATH")]
    log_output: Option<PathBuf>,

    /// 入力ディレクトリのパス
    #[arg()]
    input_path: PathBuf,

    /// 出力ディレクトリのパス
    #[arg()]
    output_path: PathBuf,
}

impl Options {
    ///
    /// ログレベルへのアクセサ
    ///
    /// # 戻り値
    /// 設定されたログレベルを返す
    fn log_level(&self) -> LogLevel {
        self.log_level
    }

    ///
    /// ログの出力先へのアクセサ
    ///
    /// # 戻り値
    /// ログの出力先として設定されたパス情報を返す(未設定の場合はNone)。
    ///
    fn log_output(&self) -> Option<PathBuf> {
        self.log_output.clone()
    }

    /// 
    /// 入力ディレクトリへのアクセサ
    ///
    /// # 戻り値
    /// 入力ディレクトリへのパスオブジェクト
    ///
    pub(crate) fn input_path(&self) -> PathBuf {
        self.input_path.clone()
    } 

    /// 
    /// 出力ディレクトリへのアクセサ
    ///
    /// # 戻り値
    /// 出力ディレクトリへのパスオブジェクト
    ///
    pub(crate) fn output_path(&self) -> PathBuf {
        self.output_path.clone()
    } 

    ///
    /// 設定情報のバリデーション
    ///
    /// # 戻り値
    /// 設定情報に問題が無い場合は`Ok(())`を返す。問題があった場合はエラー情報
    /// を`Err()`でラップして返す。
    fn validate(&self) -> Result<()> {
        // 入力ディレクトリの確認
        if !self.input_path.is_dir() {
            return Err(anyhow!(
                "{} is not directory",
                self.input_path.display()
            ));
        }

        // 出力ディレクトリの確認
        if !self.output_path.is_dir() {
            return Err(anyhow!(
                "{} is not directory",
                self.output_path.display()
            ));
        }

        Ok(())
    }
}

///
/// コマンドラインオプションのパース
///
/// # 戻り値
/// 処理に成功した場合はオプション設定をパックしたオブジェクトを`Ok()`でラップ
/// して返す。失敗した場合はエラー情報を`Err()`でラップして返す。
///
pub(super) fn parse() -> Result<Arc<Options>> {
    let opts = Options::parse();

    /*
     * 設定情報のバリデーション
     */
    opts.validate()?;

    /*
     * ログ機能の初期化
     */
    logger::init(&opts)?;

    /*
     * 設定情報の返却
     */
    Ok(Arc::new(opts))
}
