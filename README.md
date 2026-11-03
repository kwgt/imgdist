# imgdist

Exifの撮影日時をもとに、画像ファイルを日付別のフォルダへコピーするCLIツールです。
メモリカードなどの入力ディレクトリを再帰的に走査し、処理済みファイルの情報を
redbに記録して、次回実行時の再処理を省きます。コピー元のファイルは残ります。

要求仕様は[doc/REQUIREMENTS.md](doc/REQUIREMENTS.md)を参照してください。
このREADMEのコマンド例と設定項目は、現在の実装に合わせています。

## ビルド方法

RustとCargoが利用できる環境で、プロジェクトのルートから実行します。

```sh
cargo build --release --locked
```

通常、実行ファイルは次の場所に生成されます。

- Windows: `target/release/imgdist.exe`
- Linux / macOS: `target/release/imgdist`

Cargoの設定や環境変数でビルド先を変更している場合は、その出力先を参照してください。
ビルド時にはGitのコミット情報も取得します。Gitを実行できない場合、バージョン表示の
コミット情報は`unknown`になります。

Cargo経由で起動する場合は、`--`以降にimgdistの引数を指定します。

```sh
cargo run --release --locked -- --help
```

以降の例は、実行ファイルのあるディレクトリを`PATH`に追加した場合の記述です。
必要に応じて`imgdist`を実行ファイルのパスに置き換えてください。

## 使用方法

```text
imgdist [OPTIONS] <INPUT_PATH>
```

`INPUT_PATH`には入力ディレクトリを指定します。出力先は`--output`または
設定ファイルの`path_info.output_path`で指定する必要があります。
入力ディレクトリ、出力先の基点ディレクトリ、指定する場合のRAW用の基点ディレクトリは、
実行前に作成してください。日付別のサブフォルダは自動で作成されます。

```sh
imgdist --output ./photos ./input
```

たとえば撮影日が2025年1月15日の`IMG_0001.JPG`は、次の場所へコピーされます。

```text
photos/
└── 2025/
    └── 20250115/
        └── IMG_0001.JPG
```

対象はJPEG（`jpg`、`jpeg`）とRAW（`dng`、`nef`、`cr2`、`arw`、`orf`、
`rw2`、`pef`、`srw`、`raf`、`3fr`、`fff`、`x3f`）です。
拡張子の大文字・小文字は区別しません。撮影日時にはExifの`DateTimeOriginal`を
使うため、その情報を読み取れない画像はコピーされません。

### 使用例

RAWを別の出力先へ保存する場合も、`年/YYYYMMDD/元のファイル名`の構成になります。
`--raw-output`を省略すると、RAWはJPEGと同じ出力先に保存されます。

```sh
imgdist --output ./photos --raw-output ./raw ./input
```

2025年1月に撮影された画像だけを処理する例です。開始日は含み、終了日は含みません。

```sh
imgdist --output ./photos --from-date 2025-01-01 --to-date 2025-02-01 ./input
```

Windowsで、メモリカードから取り込む例です。

```powershell
imgdist --output 'D:\Photos' --raw-output 'D:\RAW' 'E:\DCIM'
```

### CLIオプション

| オプション | 内容・省略時の動作 |
| --- | --- |
| `-l`, `--log-level <LEVEL>` | `off`、`error`、`warn`、`info`、`debug`、`trace`。既定値は`info`。大文字・小文字を区別しません。 |
| `-L`, `--log-output <PATH>` | ログの出力先。省略時は標準出力。ファイル指定で単一ファイル、既存ディレクトリ指定でローテーション付き出力。 |
| `-c`, `--config-file <FILE>` | 読み込むTOML設定ファイルのパス。省略時はOSごとの既定パスを使用。 |
| `-o`, `--output <DIR>` | 出力先の基点ディレクトリ。CLIまたは設定ファイルでの指定が必要。 |
| `-r`, `--raw-output <DIR>` | RAW用の基点ディレクトリ。省略時は通常の出力先を使用。 |
| `-f`, `--from-date <DATE>` | 撮影日の下限（`YYYY-MM-DD`、当日を含む）。省略時は下限なし。 |
| `-t`, `--to-date <DATE>` | 撮影日の上限（`YYYY-MM-DD`、当日を含まない）。省略時は上限なし。 |
| `--cache-db <FILE>` | キャッシュDBのパス。省略時はOSごとの既定パスを使用。 |
| `--cache-eval-mode <LEVEL>` | `shallow`または`strict`。既定値は`shallow`。大文字・小文字を区別しません。 |
| `-s`, `--show-options` | 適用後の設定を表示して終了。画像のコピーは行いません。 |
| `--save-config` | 適用後の設定を設定ファイルに保存して終了。既存ファイルへの上書きには確認があります。 |
| `-h`, `--help` | ヘルプを表示。 |
| `-V`, `--version` | バージョンを表示。 |

ログを既存ディレクトリへ出力する場合は、1本あたり2 MiBを基準にローテーションし、
ローテーション済みログを10本保持します。

`--show-options`と`--save-config`でも、入力・出力ディレクトリの検証と
キャッシュDBの初期化が行われます。有効な入力・出力先が必要です。

```sh
imgdist --output ./photos --show-options ./input
imgdist --output ./photos --save-config ./input
```

## 設定ファイル

設定ファイルはTOML形式です。設定の優先順位は、CLIで指定した値、設定ファイルの値、
組み込みの既定値の順です。

既定の保存場所は、`directories::BaseDirs`が返す設定用・キャッシュ用ディレクトリの
下にある`imgdist`ディレクトリです。

| ファイル | 既定パスの構成 |
| --- | --- |
| 設定ファイル | `<OSのローカル設定ディレクトリ>/imgdist/config.toml` |
| キャッシュDB | `<OSのキャッシュディレクトリ>/imgdist/cache.redb` |

たとえばLinuxでは、設定ファイルは`$XDG_CONFIG_HOME/imgdist/config.toml`
（未設定時は`~/.config/imgdist/config.toml`）、キャッシュDBは
`$XDG_CACHE_HOME/imgdist/cache.redb`（未設定時は`~/.cache/imgdist/cache.redb`）です。
Windowsでは、どちらも通常は`%LOCALAPPDATA%\imgdist\`以下に置かれます。
適用されたパスは`--show-options`で確認できます。

既定パスの設定ファイルが存在しない場合は、設定ファイルなしで動作します。
`--config-file`で明示したファイルは、存在しないとエラーになります。

### 設定例

以下のパスは利用環境に合わせて変更してください。`[log_info]`と`[path_info]`は
必要なセクションです。各キーと`[cache_info]`は省略できます。

```toml
[log_info]
level = "INFO"
# output = "./imgdist.log"

[path_info]
output_path = "./photos"
# raw_output_path = "./raw"
# cache_db_path = "./cache.redb"

[cache_info]
cache_eval_mode = "shallow"
```

- `log_info.level`: ログレベル。設定ファイルでは`OFF`、`ERROR`、`WARN`、
  `INFO`、`DEBUG`、`TRACE`の大文字で指定します。
- `log_info.output`: ログ出力先。省略時は標準出力です。
- `path_info.output_path`: 通常の出力先。
- `path_info.raw_output_path`: RAWを分離する場合の出力先。
- `path_info.cache_db_path`: キャッシュDBのパス。
- `cache_info.cache_eval_mode`: `shallow`または`strict`の小文字で指定します。

相対パスは実行時のカレントディレクトリを基準に扱われます。
Windowsのパスは、TOMLのリテラル文字列（例: `'D:\Photos'`）で記述すると
バックスラッシュのエスケープを避けられます。

```sh
imgdist --config-file ./config.toml ./input
```

`--save-config`では、ログ・出力先・キャッシュの設定が保存されます。
入力パスと日付範囲は保存されません。新規作成時は`--config-file`を省略すると
既定パスへ保存できます。任意のパスを指定する場合は、先に有効な設定ファイルを
用意してください。

## キャッシュと再実行

`shallow`は更新日時とファイルサイズを比較し、`strict`はそれらに加えて
抜粋したExif情報のハッシュも比較します。キャッシュに一致したファイルはスキップされます。

現在の実装では、出力先にファイルが残っているかどうかはキャッシュ判定に含まれません。
また、日付範囲外としてコピーしなかった画像もキャッシュに記録されます。
出力先や日付範囲を変更して再処理したい場合は、`--cache-db`で別のDBファイルを
指定してください。

同じ日付フォルダに同名ファイルがある場合、コピーが実行されると既存ファイルを
上書きします。

## 要求仕様との表記の相違

[要求仕様](doc/REQUIREMENTS.md)には、現在の実装と異なる記載があります。
利用時はこのREADMEと`--help`に記載した指定方法を使用してください。

| 項目 | 要求仕様の記載 | 現在の実装 |
| --- | --- | --- |
| 設定ファイルの指定 | `-C`、`--config-file`（本文では`--config`） | `-c`、`--config-file` |
| 出力先の指定 | `--output-path` | `--output` |
| 設定の保存 | 表では`--show-config`、本文では`--save-config` | `--save-config` |
| TOMLのログ設定キー | `log_info.log_level`、`log_info.log_output` | `log_info.level`、`log_info.output` |
| 既定のファイル配置 | XDGディレクトリ直下 | OSごとのディレクトリ配下の`imgdist/`内 |

## ライセンス

[MIT License](LICENSE)
