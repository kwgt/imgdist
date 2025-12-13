//
// Image file distributor
//
//  Copyright (C) 2025 Hiroshi KUWAGATA <kgt9221@gmail.com>
//

//!
//! キャッシュデータベースを扱うモジュール
//!

use std::fs::{File, Metadata};
use std::io::BufReader;

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use chrono::{DateTime, Local, TimeZone, Utc};
use exif::{Exif, Tag};
use fnv::FnvHasher;
use log::{debug, warn};
use redb::{Database, TableDefinition, TypeName, Value};
use serde::{Deserialize, Serialize};
use std::hash::Hasher;

use crate::cmd_args::CacheEvalMode;

/// キャッシュテーブルの定義
const TABLE: TableDefinition<String, CacheRecord> =
    TableDefinition::new("cache");

///
/// 処理済みファイル情報
///
#[derive(Clone, Debug, Serialize, Deserialize)]
struct CacheRecord {
    /// キャッシュに記録した日時（秒単位、ISO8601）
    timestamp: String,

    /// mtime（秒単位、ISO8601）
    mtime: String,

    /// ファイルサイズ
    file_size: u64,

    /// 抜粋したExif情報
    exif: ExifSummary,
}

impl CacheRecord {
    ///
    /// インスタンスを構築する
    ///
    /// # 引数
    /// * `mtime` - mtime（ISO8601、秒精度）
    /// * `file_size` - ファイルサイズ
    /// * `exif` - Exif情報のサマリ
    ///
    /// # 戻り値
    /// 構築された`CacheRecord`
    ///
    fn new(mtime: String, file_size: u64, exif: ExifSummary) -> Result<Self> {
        let timestamp = format_iso8601(truncate_system_time(SystemTime::now())?)?;

        Ok(Self {
            timestamp,
            mtime,
            file_size,
            exif
        })
    }
}

// Valueトレイトの実装
impl Value for CacheRecord {
    type SelfType<'a> = Self;
    type AsBytes<'a> = String;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a
    {
        serde_json::from_slice::<Self>(data).expect("JSON deserialize failed")
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b
    {
        serde_json::to_string(value).expect("JSON serialize failed")
    }

    fn type_name() -> TypeName {
        TypeName::new("CacheRecord")
    }
}

///
/// Exif情報の抜粋
///
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct ExifSummary {
    /// DateTimeOriginal
    pub(crate) datetime_original: Option<String>,

    /// Make/Model
    pub(crate) make_model: Option<String>,

    /// CameraSerialNumber/BodySerialNumber
    pub(crate) camera_serial: Option<String>,

    /// ExifImageUniqueID
    pub(crate) image_unique_id: Option<String>,

    /// ImageWidth/Height
    pub(crate) image_dimensions: Option<String>,
}

impl ExifSummary {
    ///
    /// 抜粋したExif情報からハッシュ値を計算する
    ///
    /// # 戻り値
    /// FNV1 64bitによるハッシュ値
    ///
    fn calc_hash(&self) -> u64 {
        let null = "".to_string();
        let s = format!(
            "{}:{}:{}:{}:{}",
            self.datetime_original.as_ref().unwrap_or(&null),
            self.make_model.as_ref().unwrap_or(&null),
            self.camera_serial.as_ref().unwrap_or(&null),
            self.image_unique_id.as_ref().unwrap_or(&null),
            self.image_dimensions.as_ref().unwrap_or(&null),
        );

        let mut hasher = FnvHasher::default();
        hasher.write(s.as_bytes());
        hasher.finish()
    }
}

// トレイトFrom<&Exif>の実装
impl From<&Exif> for ExifSummary {
    fn from(value: &Exif) -> Self {
        let datetime_original = value
            .get_field(Tag::DateTimeOriginal, exif::In::PRIMARY)
            .map(|field| field.display_value().to_string());

        let make = value
            .get_field(Tag::Make, exif::In::PRIMARY)
            .map(|field| field.display_value().to_string());

        let model = value
            .get_field(Tag::Model, exif::In::PRIMARY)
            .map(|field| field.display_value().to_string());

        let make_model = match (make, model) {
            (Some(make), Some(model)) => Some(format!("{}/{}", make, model)),
            (Some(make), None) => Some(make),
            (None, Some(model)) => Some(model),
            (None, None) => None,
        };

        let camera_serial = value
            .get_field(Tag::BodySerialNumber, exif::In::PRIMARY)
            .map(|field| field.display_value().to_string());

        let image_unique_id = value
            .get_field(Tag::ImageUniqueID, exif::In::PRIMARY)
            .map(|field| field.display_value().to_string());

        let width = value
            .get_field(Tag::PixelXDimension, exif::In::PRIMARY)
            .map(|field| field.display_value().to_string());

        let height = value
            .get_field(Tag::PixelYDimension, exif::In::PRIMARY)
            .map(|field| field.display_value().to_string());

        let image_dimensions = match (width, height) {
            (Some(w), Some(h)) => Some(format!("{}x{}", w, h)),
            _ => None,
        };

        Self {
            datetime_original,
            make_model,
            camera_serial,
            image_unique_id,
            image_dimensions,
        }
    }
}


///
/// キャッシュ判定の結果
///
pub(crate) enum CacheDecision {
    /// キャッシュヒット
    Hit,

    /// キャッシュミスまたは差分あり（コピー・コミットが必要）
    Miss { handle: TxnHandle, exif: Exif },
}

///
/// コミット用ハンドル
///
pub(crate) struct TxnHandle {
    rel_path: PathBuf,
    record: CacheRecord,
}

impl TxnHandle {
    ///
    /// ハンドルの相対パスの参照を返す
    ///
    /// # 戻り値
    /// キャッシュキーのバイト列
    ///
    fn rel_path<'a>(&'a self) -> &'a PathBuf {
        &self.rel_path
    }

    ///
    /// ハンドルのレコードの参照を返す
    ///
    /// # 戻り値
    /// キャッシュ値のバイト列
    ///
    fn record<'a>(&'a self) -> &'a CacheRecord {
        &self.record
    }
}

///
/// キャッシュデータベースを管理する構造体
///
#[derive(Debug)]
pub(crate) struct Cache {
    /// redbデータベース
    db: Database,

    /// キャッシュ評価モード
    eval_mode: CacheEvalMode,

    /// ボリュームID
    volume_id: String,

    /// ボリュームプレフィクス
    volume_prefix: PathBuf,
}

impl Cache {
    ///
    /// キャッシュデータベースを開く
    ///
    /// # 引数
    /// * `path` - データベースファイルのパス
    /// * `eval_mode` - キャッシュ評価モード
    ///
    /// # 戻り値
    /// 初期化済みの`Cache`構造体
    ///
    pub(crate) fn open<P>(db_path: P, eval_mode: CacheEvalMode, input_path: P)
        -> Result<Self>
    where 
        P: AsRef<Path>
    {
        /*
         * データベースのオープン
         */
        let db_path = db_path.as_ref();

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let db = match Database::builder().create(db_path) {
            Ok(db) => db,
            Err(err) => {
                warn!(
                    "cache open failed ({}), recreating: {}",
                    db_path.display(),
                    err
                );

                Database::builder().create(db_path)?
            },
        };

        // あろうがなかろうが、とりあえずテーブルを作る
        let write_txn = db.begin_write()?;
        {
            write_txn.open_table(TABLE)?;
            write_txn.commit()?;
        }

        /*
         * 入力パスのボリューム情報の取得
         */
        let volume_id = get_volume_id(&input_path)?;
        let volume_prefix = get_volume_prefix(&input_path)?;

        debug!("volume_id: {} , volume_prefix: {}", volume_id, volume_prefix.display());

        Ok(Self {db, eval_mode, volume_id, volume_prefix})
    }

    ///
    /// コミット用ハンドルを構築する
    ///
    /// # 引数
    /// * `volume_id` - ボリュームID
    /// * `rel_path` - 相対パス
    /// * `meta` - 現在のファイル情報
    ///
    /// # 戻り値
    /// コミット用ハンドル
    ///
    fn build_handle(&self, rel_path: PathBuf, record: CacheRecord,)
        -> Result<TxnHandle>
    {
        Ok(TxnHandle {rel_path, record})
    }

    ///
    /// キャッシュの更新をコミットする
    ///
    /// # 引数
    /// * `handle` - コミット用ハンドル
    ///
    /// # 戻り値
    /// コミット結果
    ///
    pub(crate) fn commit(&self, handle: TxnHandle) -> Result<()> {
        self.put_cache_record(handle.rel_path(), handle.record())
    }

    ///
    /// キャッシュレコードを読み出す
    ///
    /// # 引数
    /// * `rel_path` - 相対パス
    ///
    /// # 戻り値
    /// 見つかった場合はレコードを返し、見つからなければ`None`を返す
    ///
    fn get_cache_record(&self, rel_path: &Path) -> Result<Option<CacheRecord>> {
        let txn = self.db.begin_read()?;
        {
            let table = txn.open_table(TABLE)?;
            let key = build_key(&self.volume_id, &rel_path);

            Ok(table.get(&key)?.map(|data| data.value()))
        }
    }

    ///
    /// キャッシュレコードを書き込む
    ///
    /// # 引数
    /// * `rel_path` - 相対パス
    /// * `data` - 書き込むキャッシュレコード
    ///
    /// # 戻り値
    /// 書き込み結果
    ///
    fn put_cache_record(&self, rel_path: &Path, data: &CacheRecord)
        -> Result<()>
    {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(TABLE)?;
            let key = build_key(&self.volume_id, &rel_path);

            table.insert(&key, data)?;
        }

        txn.commit()?;
        Ok(())
    }

    ///
    /// キャッシュをモードに応じて評価し、必要ならハンドルを返す
    ///
    /// # 引数
    /// * `volume_id` - ボリュームID
    /// * `rel_path` - 相対パス
    /// * `file_size` - ファイルサイズ
    /// * `mtime` - mtime
    /// * `exif_loader` - Exif情報と撮影日時を遅延取得するクロージャ
    ///
    /// # 戻り値
    /// ヒットまたはコピー・コミットが必要な場合のハンドル
    ///
    pub(crate) fn evaluate<P>(&self, path: P, meta: Metadata)
        -> Result<CacheDecision>
    where
        P: AsRef<Path>,
    {
        let abs_path = path.as_ref().canonicalize()?;
        let rel_path = abs_path.strip_prefix(&self.volume_prefix)?;
        let mtime = format_iso8601(meta.modified()?)?;

        // Exif情報の取り置きを行う変数
        let mut reserve = None;

        /*
         * キャッシュ情報を読み出してファイルの更新状況を判断
         *   ヒット→変化無し
         *   ミス→変化有り
         */
        match self.get_cache_record(&rel_path)? {
            Some(data) => {
                // キャッシュデータがある場合はヒットかミスかを判断
                if data.file_size == meta.len() && data.mtime == mtime {
                    match self.eval_mode {
                        // Shallowの場合は、サイズとmtimeの一致のみでヒット
                        CacheEvalMode::Shallow => return Ok(CacheDecision::Hit),

                        // Strictの場合はサイズとmtimeの一致に加え、Exif情報の
                        // 一致で判断
                        CacheEvalMode::Strict => {
                            // Exifを読み出してハッシュ値をチェック
                            let (exif, summary) = read_exif(&path)?;
                            if summary.calc_hash() == data.exif.calc_hash() {
                                return Ok(CacheDecision::Hit);
                            }

                            // ここに到達した場合は、キャッシュミスなので新情報
                            // で更新する。せっかく読み出したExifなので取り置き
                            // しておき後で使う。
                            reserve = Some((exif, summary));
                        }
                    }
                }
            }

            None => {
                // キャッシュデータが存在しない場合はミスと判断
            }
        }

        /*
         * キャッシュミスの場合のフォールバック (キャッシュ情報を更新)
         */

        // 既に読み出していたexif情報がある場合はそれを利用、読み出していない
        // 場合は新規で読み出す。
        let (exif, summary) = match reserve {
            Some(reserve) => reserve,
            None => read_exif(path)?,
        };

        let handle = self.build_handle(
            rel_path.to_path_buf(), 
            CacheRecord::new(mtime, meta.len(), summary)?,
        )?;

        return Ok(CacheDecision::Miss {handle, exif});
    }
}

/// キーを構築する
fn build_key(volume_id: &str, rel_path: &Path) -> String {
    format!("{}:{}", volume_id, rel_path.display())
}

/// SystemTimeを秒単位に切り詰める
fn truncate_system_time(time: SystemTime) -> Result<SystemTime> {
    let dur = time.duration_since(UNIX_EPOCH)?;
    Ok(UNIX_EPOCH + Duration::from_secs(dur.as_secs()))
}

///
/// SystemTimeをISO8601文字列に変換する
///
/// # 引数
/// * `time` - 変換するシステム時刻
///
/// # 戻り値
/// ISO8601文字列を返す。ただし、秒未満の時刻情報は丸められて秒単位の情報に切り
/// 詰められる。
///
fn format_iso8601(time: SystemTime) -> Result<String> {
    let time = truncate_system_time(time)?;
    let datetime = DateTime::<Utc>::from(time).with_timezone(&Local);
    let seconds = datetime.timestamp();

    Ok(Local
        .timestamp_opt(seconds, 0)
        .single()
        .unwrap()
        .to_rfc3339()
    )
}

///
/// ボリュームIDを取得する
///
/// # 引数
/// * `path` - 対象となるパス
///
/// # 戻り値
/// ボリュームID
///
fn get_volume_id<P>(path: P) -> Result<String>
where 
    P: AsRef<Path>,
{
    /*
     * Linuxの場合はファイルシステムUUIDを使う (/dev/disk/by-uuid/のUUID)
     */
    #[cfg(target_os = "linux")]
    {
        use std::fs::read_dir;

        let (mount_point, source, dev_id) = linux_mount_info(path.as_ref())?;
        let mut candidates: Vec<PathBuf> = Vec::new();

        if let Some(dev_path) = source {
            if dev_path.starts_with("/dev") {
                candidates.push(dev_path);
            }
        }

        if let Some(dev_id) = dev_id {
            let sys_dev = PathBuf::from("/sys/dev/block").join(&dev_id);
            if sys_dev.exists() {
                if let Ok(link) = std::fs::read_link(&sys_dev) {
                    if let Some(name) = link.file_name() {
                        candidates.push(PathBuf::from("/dev").join(name));
                    }
                }
            }
        }

        for dev in candidates {
            let dev_real = match dev.canonicalize() {
                Ok(p) => p,
                Err(_) => continue,
            };

            let uuid_dir = PathBuf::from("/dev/disk/by-uuid");

            for entry in read_dir(&uuid_dir)? {
                let entry = entry?;

                let target = entry.path();

                if let Some(prefix) = target.parent() {
                    let target_real = std::fs::read_link(&target)
                        .map(|p| prefix.join(&p))?
                        .canonicalize()?;

                    if target_real == dev_real {
                        if let Some(name) = entry.file_name().to_str() {
                            return Ok(name.to_string());
                        }
                    }
                }
            }
        }

        Err(anyhow!("filesystem UUID not found for {}", mount_point.display()))
    }

    /*
     * macOSの場合はVolume UUID
     */
    #[cfg(target_os = "macos")]
    {
        use libc::{
            attrlist, getattrlist, ATTR_BIT_MAP_COUNT, ATTR_VOL_INFO,
            ATTR_VOL_UUID
        };
        use std::ffi::CString;
        use std::mem::{size_of, zeroed};

        #[repr(C)]
        #[derive(Debug, Copy, Clone)]
        struct VolUuidBuffer {
            length: u32,
            uuid: [u8; 16],
        }

        let c_path = CString::new(path.as_ref().as_os_str().as_bytes())?;

        let mut attrs: attrlist = unsafe { zeroed() };
        attrs.bitmapcount = ATTR_BIT_MAP_COUNT as u16;
        attrs.volattr = ATTR_VOL_INFO | ATTR_VOL_UUID;

        let mut buf: VolUuidBuffer = VolUuidBuffer {
            length: size_of::<VolUuidBuffer>() as u32,
            uuid: [0; 16],
        };

        let ret = unsafe {
            getattrlist(
                c_path.as_ptr(),
                &mut attrs,
                &mut buf as *mut _ as *mut _,
                buf.length as usize,
                0,
            )
        };

        if ret != 0 {
            return Err(anyhow!("volume uuid is not available"));
        }

        Ok(format_uuid(buf.uuid))
    }

    /*
     * その他のUNIX系
     */
    #[cfg(all(not(target_os = "linux"), not(target_os = "macos"),
        target_family = "unix"))]
    {
        use nix::sys::statfs::statfs;

        let info = statfs(path.as_ref())?;
        let fsid = info.f_fsid();
        Ok(format!("{:x}:{:x}", fsid.val[0], fsid.val[1]))
    }

    /*
     * Windowsの場合はボリュームシリアル番号を用いる
     */
    #[cfg(target_family = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::GetVolumeInformationW;

        let volume_root: Vec<u16> = get_volume_prefix(path)?
            .as_os_str()
            .encode_wide()
            .chain([0])
            .collect();

        let mut serial: u32 = 0;
        let mut dummy_max_comp_len: u32 = 0;
        let mut file_system_flags: u32 = 0;

        if unsafe {
            GetVolumeInformationW(
                PCWSTR(volume_root.as_ptr()),
                None,
                Some(&mut serial),
                Some(&mut dummy_max_comp_len),
                Some(&mut file_system_flags),
                None,
            )
        }.is_ok() {
            Ok(format!("{:08X}", serial))
        } else {
            Err(anyhow!("volume id is not available"))
        }
    }

    #[cfg(not(any(target_family = "unix", target_family = "windows")))]
    {
        Err(anyhow!("volume id is not available on this platform"))
    }
}

///
/// ボリュームプレフィクスを取得する
///
/// # 引数
/// * `path` - 対象となるパス
///
/// # 戻り値
/// マウントポイントのパス
///
fn get_volume_prefix<P>(path: P) -> Result<PathBuf>
where 
    P: AsRef<Path>,
{
    #[cfg(target_os = "linux")]
    {
        let (mount_point, _, _) = linux_mount_info(path.as_ref())?;
        Ok(mount_point)
    }

    #[cfg(all(not(target_os = "linux"), target_family= "unix"))]
    {
        use nix::sys::statfs::statfs;

        let info = statfs(path)?;
        let mntonname = info
            .f_mntonname
            .iter()
            .take_while(|c| **c != 0)
            .map(|c| *c as u8 as char)
            .collect::<String>();

        Ok(PathBuf::from(mntonname))
    }

    #[cfg(target_os = "windows")]
    {
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::GetVolumePathNameW;
        use std::os::windows::ffi::OsStrExt;

        let wide: Vec<u16> = path.as_ref().as_os_str().encode_wide().chain([0]).collect();
        let mut buffer = vec![0u16; 260];

        unsafe {
            if GetVolumePathNameW(
                PCWSTR(wide.as_ptr() as *mut _),
                &mut buffer,
            ).is_ok()
            {
                let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
                let s = String::from_utf16_lossy(&buffer[..end]);
                return Ok(PathBuf::from(s).canonicalize()?);
            }
        }

        Err(anyhow::anyhow!("GetVolumePathNameW failed"))
    }
}

///
/// mountinfoから対象パスのマウントポイントとデバイスパスを取得する
///
/// # 引数
/// * `path` - 対象パス
///
/// # 戻り値
/// (マウントポイント, デバイスパス) のタプル
///
#[cfg(target_os = "linux")]
fn linux_mount_info(path: &Path) -> Result<(PathBuf, Option<PathBuf>, Option<String>)> {
    let target = path.canonicalize()?;
    let file = File::open("/proc/self/mountinfo")?;
    let reader = BufReader::new(file);

    let mut best: Option<(PathBuf, Option<PathBuf>, Option<String>)> = None;

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() < 10 {
            continue;
        }

        let dash_pos = parts.iter().position(|v| *v == "-");
        if dash_pos.is_none() {
            continue;
        }

        let dash_pos = dash_pos.unwrap();
        if dash_pos + 2 >= parts.len() {
            continue;
        }

        let mount_point = decode_mount_path(parts[4]);
        let source = decode_mount_path(parts[dash_pos + 2]);
        let dev_id = parts[2].to_string();

        let mount_path = PathBuf::from(&mount_point);

        if target.starts_with(&mount_path) {
            let replace = match &best {
                Some((existing, _, _)) => {
                    mount_path.as_os_str().len() > existing.as_os_str().len()
                }
                None => true,
            };

            if replace {
                best = Some((mount_path, Some(PathBuf::from(source)), Some(dev_id)));
            }
        }
    }

    best.ok_or_else(|| anyhow!("mount point not found"))
}

#[cfg(target_os = "linux")]
///
/// mountinfo内のエスケープをデコードする
///
/// # 引数
/// * `s` - エスケープされたパス文字列
///
/// # 戻り値
/// デコード済みの文字列
///
fn decode_mount_path(s: &str) -> String {
    s.replace("\\040", " ")
}

///
/// macOSのUUIDをハイフン付き文字列表現に整形する
///
/// # 引数
/// * `uuid` - 16バイトのUUID
///
/// # 戻り値
/// ハイフン付き大文字16進表記の文字列
///
#[cfg(target_os = "macos")]
fn format_uuid(uuid: [u8; 16]) -> String {
    format!(
        concat!(
            "{:02X}{:02X}{:02X}{:02X}",
            "-",
            "{:02X}{:02X}",
            "-",
            "{:02X}{:02X}",
            "-",
            "{:02X}{:02X}",
            "-",
            "{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        )
        uuid[0], uuid[1], uuid[2], uuid[3],
        uuid[4], uuid[5],
        uuid[6], uuid[7],
        uuid[8], uuid[9],
        uuid[10], uuid[11], uuid[12], uuid[13], uuid[14], uuid[15],
    )
}

///
/// Exifを読み込む。またサマリ情報を作成し一緒に返す
///
/// # 引数
/// * `path` - 対象パス
///
/// # 戻り値
/// 読み込んだExif情報とサマリ情報をパックしたタプルを返す
///
fn read_exif<P>(path: P) -> Result<(Exif, ExifSummary)>
where 
    P: AsRef<Path>,
{
    let mut bufreader = BufReader::new(File::open(&path)?);

    match exif::Reader::new().read_from_container(&mut bufreader) {
        Ok(exif) => {
            let summary = ExifSummary::from(&exif);
            Ok((exif, summary))
        }

        Err(err) => Err(anyhow!(
            "read exif failed {}: {}",
            path.as_ref().display(),
            err
        )),
    }
}
