# 画像振り分けアプリケーション仕様

## 機能概要
指定したフォルダに置かれている画像ファイルのExif情報をもとに、撮影日付ごとにフォルダの振り分けを行う。

## コマンドライン仕様
```sh
imgdist [OPTIONS] <INPUT_PATH>
```

### オプション
以下のものが指定できる。

| オプション | 意味 | デフォルト値
|:--|:--|:--
| `-l`, `--log-level <LEVEL>`  | ログレベルの指定 | "info"
| `-L`, `--log-output <PATH>`  | ログの出力先の指定 | 標準出力へ出力
| `-C`, `--config-file <FILE>` | コンフィギュレーションファイルへのパス | $XDG_CONFIG_HOME/config.toml
|       `--cache-db <FILE>`    | キャッシュ用データベースファイルへのパス  | $XDG_CACHE_HOME/cache.redb
|       `--cache-eval-mode <LEVEL>` | キャッシュ評価時の詳細度  | shallow
| `-o`, `--output-path <DIR>`  | 基点となる出力ディレクトリのパス |
| `-r`, `--raw-output <DIR>`   | RAWファイルを分離保存する場合の基点ディレクトリのパス |
| `-f`, `--from-date <DATE>`   | 処理対象の撮影日付の始点 (YYYY-MM-DD形式、この日付を含む) |
| `-t`, `--to-date <DATE>`     | 処理対象の撮影日付の終点 (YYYY-MM-DD形式、この日付を含まない) |
| `-s`, `--show-options`       | 設定情報の表示 |
|       `--show-config`        | 設定情報をconfig.tomlへ保存する |

### 概要
`<INPUT_PATH>`で指定されたディレクトリ中のファイルを走査し、`--output-path`で指定されたディレクトリに日付単位でサブフォルダを作成しながらファイルの振り分けを行う（振り分けはファイルの移動ではなくコピーで行う）。

画像ファイルと思われるファイルを対象とし、画像ファイルか否かの判断は拡張子のみで行う。また、撮影日付の取得はExif情報の読み取りで行う(Exif情報を含まないファイルは処理対象外とする)。

`--from-date`とオプションと`--to-date`オプションで処理対象の日付範囲を指定することができる（始点日付は範囲に含むが終点日付は範囲に含まない）。

処理対象がデジタルカメラのメモリカードであるため、処理済みファイルの再処理を避ける仕組みを組み込む(Exif情報の読み込みが遅いため)。処理済みファイルの情報はキャッシュ情報として記録しておき、再度処理しないようにする(処理済みファイルを検出した場合は、ログにinfoレベルでスキップした旨を記録しする)。

`--log-level`オプションの`<LEVEL>`には以下の値が設定可能。

  - off : ログを記録しない
  - error : エラーの場合のみを記録
  - warn : 警告以上の場合を記録
  - info : 一般情報レベルを記録
  - debug : デバッグ用メッセージも記録
  - trace : トレース情報も記録

`--cache-eval-mode`オプションの`<LEVEL>`には以下の値が設定可能。

  - shallow : mtimeとファイルサイズのみで評価
  - strict : mtimeとファイルサイズに加え、Exif情報の内容の一致で評価

`--save-config`オプションを指定した場合は、その時の設定情報をconfig.tomlへ保存する。このときの書き込み先のパスはオプション評価で最終的に決定されたパスになる。また、このオプションが指定された場合、config.tomlへの保存だけを行いその他の処理は行わずプロセスを終了する。

## ファイル要件
本ツールで使用するファイルのデフォルトパスはXDG標準に準拠させる。本ツールでは以下のファイルを使用する。

 - コンフィギュレーションファイル
 - データベースファイル

### コンフィギュレーションファイル

各種オプションのデフォルト値が定義できる設定ファイル(toml形式)が置かれる。デフォルトパスは`$XDG_CONFIG_HOME/config.toml`とする (`--config`オプションで変更可能)。オプション類のデフォルト値を記述する。

以下にコンフィギュレーションファイルのスキーマ定義をYAML形式のTaplo Schemaで記述する。

```YAML
$schema: https://taplo.tamasfe.dev/schema.json
properties:
  log_info:
    description: >-
      ログ関連の設定が格納される。
    type: "object"
    properties:
      log_level:
        description: >-
          ログレベルが格納される(--log-levelオプションに対応)。
        type: "string"
        enum:
          - "off"
          - "error"
          - "warn"
          - "info"
          - "debug"
          - "trace"

      log_output:
        description: >-
          ログの出力先が格納される(--log-outputオプションに対応)。ファイルのパス
          を指定した場合は単一ファイルへの出力となり、ディレクトリパスを指定した
          場合はログローテション付きで10本のファイルに自動切り替えを行いながら記
          録を行う(一本あたりのサイズ制限は2Mバイト)。
        type: "string"

  path_info:
    description: >-
      パス関連の設定が格納される。
    type: "object"
    proeprties:
      output_path:
        description: >-
          基点となる出力先のディレクトリのパスが格納される(--output-pathオプショ
          ンに対応)。
        type: "string"

      raw_output_path:
        description: >-
          RAW ファイルを分離保存する場合の基点となる出力先のディレクトリのパスが
          格納される(--raw-outputオプションに対応)。

      cache_db_path:
        description: >-
          処理済みファイルキャッシュデータベースファイルへのパスが格納される。
          (--cache-dbオプションに対応)。
        type: "string"
        enum:
          - "shallow"
          - "strict"

  cache_info:
    description: >-
      キャッシュ情報関連の設定が格納される。
    type: "object"
    proeprties:
      cache_eval_mode:
        description: >-
          処理済みファイル判定のためのキャッシュ情報の評価モードを指定する。
          (--cache-eval-modeオプションに対応)。
```

## キャッシュ仕様
処理済みファイルのキャッシュ情報の管理はKVSで行う。 キーと値の仕様を以下に示す。

### キャッシュデータのキー
ボリュームIDと相対パスを連結した文字列をキーとする。

ボリュームIDはプラットフォームごとに以下のものを使用する。

| プラットフォーム | 使用する値
|:---|:---
| Windows | GetVolumeInformationW()で取得できる Volume Serial Number  
| Linux系 | ファイルシステムUUID
| macOS | Volume UUID

相対パスは、マウントポイントを基点とした相対パスとする。キャッシュデータ自体がプラットフォームをクロスして使用されることはないのでパスセパレータの正規化は行わずそのまま記録する。

### キャッシュデータの値
以下の情報をシリアライズしたJSONとする。

 - タイムスタンプ(キャッシュデータを記録した日時)
 - mtime (秒単位に切り詰めるたISO8601形式のタイムゾーン付き文字列)
 - ファイルサイズ
 - 抜粋したExif情報
     - DateTimeOriginal
     - Make/Model
     - CameraSerialNumber/BodySerialNumber
     - ExifImageUniqueID
     - ImageWidth/Height

### キャッシュ情報の評価
`--cache-eval-mode`で"shallow"が指定されている場合と"strict"が指定されている場合で評価の方法を切り替える。

"shallow"が指定されている場合は、mtimeとファイルサイズのみを比較する。

"strict"が指定されている場合は、mtimeとファイルサイズに加えExif情報のハッシュ値の比較を行う(ハッシュ値はFNV1程度でOK)。使用するハッシュ値はキャッシュ情報としているExif情報を連結し他文字列に対して行うが、連結ルール時は以下の規則に従う。

  - 各Exifレコードは文字列化する
  - 記録されていないExifレコードは空文字列として扱う
  - レコードのセパレータは":"とする

## 境界仕様

### キャッシュ利用ポリシー
キャッシュ故障（DB開けない/書けない）時は警告ログを残した上で、データベースの新規作成を行う。

### ボリュームIDの取得方法
以下のものを使用する。

| プラットフォーム | 使用する値
|:---|:---
| Windows | GetVolumeInformationW()で取得できる Volume Serial Number  
| Linux系 | ファイルシステムUUID
| macOS | Volume UUID

### EXIFハッシュ対象フィールド
以下のフィールドを文字列化し、":"をセパレータとして順に連結しハッシュ値を取る。
存在しないフィールドは空文字列を当てる。

 - DateTimeOriginal
 - Make/Model
 - CameraSerialNumber/BodySerialNumber
 - ExifImageUniqueID
 - ImageWidth/Height

### 上書きポリシー
キャッシュ情報の上書きは常に上書きで構わない。

