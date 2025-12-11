//
// Image file distributor
//
//  Copyright (C) 2025 Hiroshi KUWAGATA <kgt9221@gmail.com>
//

//!
//! コマンドラインオプション関連の処理をまとめたモジュール
//!

pub(crate) mod config;
mod logger;
use std::sync::Arc;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Local, NaiveDate, TimeZone};
use clap::{Parser, ValueEnum};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};

use crate::cache::Cache;

///
/// デフォルトのコンフィグレーションファイルのパス情報を生成
///
/// # 戻り値
/// コンフィギュレーションファイルのパス情報
///
fn default_config_path() -> PathBuf {
    BaseDirs::new()
        .unwrap()
        .config_local_dir()
        .join(env!("CARGO_PKG_NAME"))
        .join("config.toml")
}

///
/// ログレベルを指し示す列挙子
///
#[derive(Debug, Clone, Copy, PartialEq, ValueEnum, Deserialize, Serialize)]
#[clap(rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "UPPERCASE")]
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

///
/// キャッシュ評価モードを指し示す列挙子
///
#[derive(Debug, Clone, Copy, PartialEq, ValueEnum, Deserialize, Serialize)]
#[clap(rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "lowercase")]
pub(crate) enum CacheEvalMode {
    /// mtimeとサイズのみで評価
    Shallow,

    /// mtime・サイズ・Exifハッシュで評価
    Strict,
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
        ignore_case = true)]
    log_level: Option<LogLevel>,

    /// ログの出力先の指定
    #[arg(short = 'L', long = "log-output", value_name = "PATH")]
    log_output: Option<PathBuf>,

    /// コンフィギュレーションファイルのパス
    #[arg(short = 'c', long = "config-file", value_name = "FILE")]
    config_file: Option<PathBuf>,

    /// 出力ディレクトリのパス
    #[arg(short = 'o', long = "output", value_name = "DIR")]
    output_path: Option<PathBuf>,

    /// RAW画像保存ディレクトリのパス（指定された場合、RAW画像はこのディレクトリ
    /// に保存）
    #[arg(short = 'r', long = "raw-output", value_name = "DIR")]
    raw_output_path: Option<PathBuf>,

    /// 処理対象の撮影日付の始点（YYYY-MM-DD形式、この日付を含む）
    #[arg(short = 'f', long = "from-date", value_name = "DATE")]
    from_date: Option<String>,

    /// 処理対象の撮影日付の終点（YYYY-MM-DD形式、この日付は含まない）
    #[arg(short = 't', long = "to-date", value_name = "DATE")]
    to_date: Option<String>,

    /// 設定情報の表示
    #[arg(short = 's', long = "show-options", default_value = "false")]
    show_options: bool,

    /// 現在の設定をconfig.tomlに書き出す
    #[arg(long = "save-config", default_value = "false")]
    save_config: bool,

    /// キャッシュデータベースファイルのパス
    #[arg(long = "cache-db", value_name = "FILE")]
    cache_db_path: Option<PathBuf>,

    /// キャッシュ評価時の詳細度
    #[arg(long = "cache-eval-mode", value_name = "LEVEL",
        ignore_case = true)]
    cache_eval_mode: Option<CacheEvalMode>,

    /// 入力ディレクトリのパス
    #[arg()]
    input_path: PathBuf,

    /// パース済みの開始日付（バリデーション時に設定）
    #[arg(skip)]
    parsed_from_date: Option<DateTime<Local>>,

    /// パース済みの終了日付（バリデーション時に設定）
    #[arg(skip)]
    parsed_to_date: Option<DateTime<Local>>,

    /// キャッシュデータベースファイルのパス（バリデーション時に設定）
    #[arg(skip)]
    parsed_cache_db_path: Option<PathBuf>,

    /// キャッシュ評価モード（バリデーション時に設定）
    #[arg(skip = CacheEvalMode::Shallow)]
    parsed_cache_eval_mode: CacheEvalMode,

    /// キャッシュデータベースオブジェクト（バリデーション時に設定）
    #[arg(skip)]
    cache: Option<Arc<Cache>>,

    /// コンフィギュレーションファイルの最終決定パス（バリデーション時に設定）
    #[arg(skip)]
    parsed_config_path: PathBuf,
}

impl Options {
    ///
    /// ログレベルへのアクセサ
    ///
    /// # 戻り値
    /// 設定されたログレベルを返す
    fn log_level(&self) -> LogLevel {
        if let Some(level) = self.log_level {
            level
        } else {
            LogLevel::Info
        }
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
    /// # 注記
    /// バリデーション関数により、self.output_pathがNoneのままこの関数が呼ばれ
    /// ることが無いことが保証されている。
    ///
    pub(crate) fn output_path(&self) -> PathBuf {
        self.output_path.as_ref().unwrap().clone()
    }

    /// 
    /// RAW画像保存ディレクトリへのアクセサ
    ///
    /// # 戻り値
    /// RAW画像保存ディレクトリへのパスオブジェクト（未設定の場合はNone）
    ///
    pub(crate) fn raw_output_path(&self) -> Option<PathBuf> {
        self.raw_output_path.clone()
    }

    /// 
    /// 撮影日付の始点へのアクセサ
    ///
    /// # 戻り値
    /// 撮影日付の始点（未設定の場合はNone）
    ///
    pub(crate) fn from_date(&self) -> Option<DateTime<Local>> {
        self.parsed_from_date
    }

    /// 
    /// 撮影日付の終点へのアクセサ
    ///
    /// # 戻り値
    /// 撮影日付の終点（未設定の場合はNone）
    ///
    pub(crate) fn to_date(&self) -> Option<DateTime<Local>> {
        self.parsed_to_date
    } 

    ///
    /// オプション情報モードか否かのフラグへのアクセサ
    ///
    /// # 戻り値
    /// オプション情報表示モードが指定されている場合は`true`が、通常モードのが
    /// 指定されている場合は`false`が返される。
    ///
    fn is_show_options(&self) -> bool {
        self.show_options
    }

    ///
    /// キャッシュデータベースファイルのパスへのアクセサ
    ///
    /// # 戻り値
    /// キャッシュデータベースファイルのパス
    ///
    pub(crate) fn cache_db_path(&self) -> PathBuf {
        self.parsed_cache_db_path.as_ref().unwrap().clone()
    }

    ///
    /// キャッシュ評価モードへのアクセサ
    ///
    /// # 戻り値
    /// キャッシュ評価モード
    ///
    pub(crate) fn cache_eval_mode(&self) -> CacheEvalMode {
        self.parsed_cache_eval_mode
    }

    ///
    /// コンフィギュレーションファイルパスへのアクセサ
    ///
    /// # 戻り値
    /// 確定したコンフィギュレーションファイルのパス
    ///
    fn config_path(&self) -> PathBuf {
        self.parsed_config_path.clone()
    }

    ///
    /// 設定保存フラグへのアクセサ
    ///
    /// # 戻り値
    /// `--save-config` が指定されていれば`true`
    ///
    fn is_save_config(&self) -> bool {
        self.save_config
    }

    ///
    /// キャッシュデータベースオブジェクトへのアクセサ
    ///
    /// # 戻り値
    /// キャッシュデータベースオブジェクト
    ///
    pub(crate) fn cache(&self) -> Arc<Cache> {
        self.cache.as_ref().unwrap().clone()
    }

    ///
    /// オプション設定内容の表示
    ///
    pub(crate) fn show_options(&self) {
        let config_path = if let Some(path) = &self.config_file {
            Some(path.clone())
        } else {
            let path = default_config_path();

            if path.exists() {
                Some(path)
            } else {
                None
            }
        };

        println!("log level:       {}", self.log_level().as_ref());
        println!("log output:      {:?}", self.log_output());
        println!("config path:     {:?}", config_path);
        println!("output path:     {:?}", self.output_path());
        println!("raw output path: {:?}", self.raw_output_path());
        println!("from data:       {:?}", self.from_date());
        println!("to data:         {:?}", self.to_date());
        println!("input path:      {:?}", self.input_path());
        println!("cache db path:   {:?}", self.cache_db_path());
        println!("cache eval mode: {:?}", self.cache_eval_mode());
        println!("save config:     {:?}", self.is_save_config());
        println!("config path:     {:?}", self.config_path());
    }

    ///
    /// コンフィギュレーションの適用
    ///
    /// # 注記
    /// config.tomlを読み込みオプション情報に反映する。
    ///
    fn apply_config(&mut self) -> Result<()> {
        let path = if let Some(path) = &self.config_file {
            // オプションでコンフィギュレーションファイルのパスが指定されて
            // いる場合、そのパスに何もなければエラー
            if !path.exists() {
                return Err(anyhow!("{} is not exists", path.display()));
            }

            // 指定されたパスを返す
            path.clone()
        } else {
            // 指定されていない場合はデフォルトのパスを返す
            default_config_path()
        };

        // この時点でパスに何も無い場合はそのまま何もせず正常終了
        if !path.exists() {
            return Ok(());
        }

        // 指定されたパスにあるのがファイルでなければエラー
        if !path.is_file() {
            return Err(anyhow!("{} is not file", path.display()));
        }

        // そのパスからコンフィギュレーションを読み取る
        match config::read(&path) {
            // 読み取れた場合は内容を適用
            Ok(config) => {
                if self.log_level.is_none() {
                    if let Some(level) = config.log_level() {
                        self.log_level = Some(level);
                    }
                }

                if self.log_output.is_none() {
                    if let Some(path) = config.log_output() {
                        self.log_output = Some(path);
                    }
                }

                if self.raw_output_path.is_none() {
                    if let Some(path) = config.raw_output_path() {
                        self.raw_output_path = Some(path);
                    }
                }

                if self.output_path.is_none() {
                    if let Some(path) = config.output_path() {
                        self.output_path = Some(path);
                    }
                }

                if self.cache_db_path.is_none() {
                    if let Some(path) = config.cache_db_path() {
                        self.cache_db_path = Some(path);
                    }
                }

                if self.cache_eval_mode.is_none() {
                    if let Some(mode) = config.cache_eval_mode() {
                        self.cache_eval_mode = Some(mode);
                    }
                }

                Ok(())
            }

            // エラーが出たらエラー
            Err(err) => Err(anyhow!("{}", err))
        }
    }

    ///
    /// 設定情報のバリデーションとキャッシュの構築
    ///
    /// # 戻り値
    /// 設定情報に問題が無い場合は`Ok(())`を返す。問題があった場合はエラー情報
    /// を`Err()`でラップして返す。
    fn validate(&mut self) -> Result<()> {
        /*
         * 入力ディレクトリの確認
         */
        if !self.input_path.is_dir() {
            return Err(anyhow!(
                "{} is not directory",
                self.input_path.display()
            ));
        }

        /*
         * 出力ディレクトリの確認
         */
        if let Some(path) = &self.output_path {
            // ディレクトリでなければエラー
            if !path.is_dir() {
                return Err(anyhow!("{} is not directory", path.display()));
            }
        } else {
            // 出力ディレクトリが指定されていなければエラー
            return Err(anyhow!("output path is not specified"));
        }

        /*
         * RAWディレクトリの確認（指定された場合）
         */
        if let Some(path) = &self.raw_output_path {
            // ディレクトリでなければエラー
            if !path.is_dir() {
                return Err(anyhow!("{} is not directory", path.display()));
            }
        }

        /*
         * 日付形式の確認とキャッシュの構築
         */
        if let Some(ref from_date) = self.from_date {
            self.parsed_from_date = Some(parse_datetime(from_date)?);
        }

        if let Some(ref to_date) = self.to_date {
            self.parsed_to_date = Some(parse_datetime(to_date)?);
        }

        /*
         * キャッシュデータベースパスの設定
         */
        let default_cache_db = BaseDirs::new()
            .unwrap()
            .cache_dir()
            .join(env!("CARGO_PKG_NAME"))
            .join("cache.redb");

        self.parsed_cache_db_path = Some(if let Some(path) = &self.cache_db_path {
            path.clone()
        } else {
            default_cache_db
        });

        /*
         * キャッシュ評価モードの設定
         */
        self.parsed_cache_eval_mode = if let Some(mode) = self.cache_eval_mode {
            mode
        } else {
            CacheEvalMode::Shallow
        };

        /*
         * キャッシュの初期化
         */
        let cache = Cache::open(
            self.parsed_cache_db_path.as_ref().unwrap(),
            self.parsed_cache_eval_mode,
            &self.input_path,
        )?;
        self.cache = Some(Arc::new(cache));

        /*
         * コンフィギュレーションファイルのパス設定
         */
        self.parsed_config_path = if let Some(path) = &self.config_file {
            path.clone()
        } else {
            default_config_path()
        };

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
    let mut opts = Options::parse();

    /*
     * コンフィギュレーションファイルの適用
     */
    opts.apply_config()?;

    /*
     * 設定情報のバリデーションとキャッシュの構築
     */
    opts.validate()?;

    /*
     * ログ機能の初期化
     */
    logger::init(&opts)?;

    /*
     * 設定の保存のみを行うモード
     */
    if opts.is_save_config() {
        opts.show_options();
        println!("");

        let cfg_path = opts.config_path();

        if cfg_path.exists() {
            print!(
                "config file {} already exists. overwrite? [y/N]: ",
                cfg_path.display()
            );
            io::stdout().flush().ok();

            let mut answer = String::new();
            io::stdin().read_line(&mut answer).ok();
            let answer = answer.trim().to_lowercase();

            if answer != "y" && answer != "yes" {
                println!("save-config aborted");
                std::process::exit(0);
            }
        }

        if let Err(err) = config::write(&cfg_path, &opts) {
            return Err(anyhow!("{}", err));
        }

        println!("config saved to {}", cfg_path.display());
        std::process::exit(0);
    }

    /*
     * 設定情報表示モード
     */
    if opts.is_show_options() {
        opts.show_options();
        std::process::exit(0);
    }

    /*
     * 設定情報の返却
     */
    Ok(Arc::new(opts))
}

///
/// 日付文字列をパースしてDateTime<Local>に変換する
///
/// # 引数
/// * `date_string` - YYYY-MM-DD形式の日付文字列
///
/// # 戻り値
/// パースが成功した場合は`Ok(DateTime<Local>)`を返す。失敗した場合はエラー情報を
/// `Err()`でラップして返す。
fn parse_datetime(date_string: &str) -> Result<DateTime<Local>> {
    match NaiveDate::parse_from_str(date_string, "%Y-%m-%d") {
        Ok(date) => {
            if let Some(datetime) = date.and_hms_opt(0, 0, 0) {
                Ok(Local.from_local_datetime(&datetime).unwrap())
            } else {
                Err(anyhow!(
                    "invalid date: {} (invalid date)",
                    date_string
                ))
            }
        },
        Err(_) => {
            Err(anyhow!(
                "invalid date format: {} (expected YYYY-MM-DD)",
                date_string
            ))
        }
    }
}
