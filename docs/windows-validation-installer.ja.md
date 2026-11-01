# Windows署名なし検証用インストーラー

対象ソース: main `7d91bcf1f84e4508d157d51d09ebd7eedbb69458`（2026-10-31）。
本手順はローカルWindows/Codex向けの検証用です。完成版、販売許可、実サービス接続の保証ではありません。

## 最小差分

- `src-tauri/tauri.validation.conf.json` を明示指定したときだけbundleを有効化し、NSIS一種類を生成。
- 通常の `tauri.conf.json`、`scripts/desktop.ps1`、製品名、identifier、version、CSP、Core/スキン/認証/保存処理は変更しない。
- インストールはcurrentUser。降格インストールは拒否する設定。ただし同一バージョンの再インストールとDB互換性は別途検証が必要。
- WebView2は既存環境を前提に `skip`。検証パッケージがWebView2を自動取得・導入することはない。WebView2未導入PCは本段階の起動検証対象外。
- resourcesは6文書の明示リストだけ。インストール先の `licenses/` に配置。プロファイル、DB、認証情報、成果物、追加スキン、テストZIPをコピーしない。
- コードのLICENSE/NOTICEと素材条件、第三者通知、認証注意を含むREADMEをそれぞれ保持。単一のApacheライセンス画面としてまとめない。
- スキンは既存の表示専用ライブラリのまま。購入/課金/配布基盤は追加しない。
- CIは既存のWindowsネイティブテストの成功後に生成し、署名なし状態を検査してSHA256を付ける。専用GitHub-hosted Windows runnerではサイレント導入・6文書のハッシュ・署名なしアプリ・同版再導入・削除・合成プロファイル保持も試験する。実際のDBや購入スキン、GUI起動はこのスモーク試験の対象外。7日保持のActions artifactのみ。GitHub Release、署名、updater、販売設定は追加しない。公開リポジトリのartifactは秘密の配布経路ではない。

## Windows x64ビルド

新しい作業コピーを使用する。Node.js 24/npm、Rust stable MSVC x64、Visual Studio C++ビルドツール/Windows SDK、Python 3、WebView2が必要。
ビルドではnpm/Cargo/NSISツール取得のネットワーク接続が必要になる。Codex本体・ログイン情報はビルドにも同梱にも不要。

リポジトリルートのPowerShellで、各コマンドの終了コード0を確認し、失敗したら後続を実行しない。

```powershell
npm ci
npm run check
npm run build
npm run test:boundary
npm run test:native
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-validation.ps1
```

TauriのbeforeBuildCommandがフロントを再生成する。Cargoは `--locked`。
既存の `npm run desktop -- -Build` は従来どおり実行ファイルのみ。

出力:
`src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/*-setup.exe`
および隣の `.sha256`。
`CARGO_TARGET_DIR` を設定している場合は解除した専用シェルを使う（スクリプトは標準出力先を検査する）。
署名設定を持たないクリーンな環境で実行する。スクリプトは生成物がNotSignedでなければ失敗とする。

ActionsではPRのCI → Windows native checks → Artifactsから取得する。runのhead SHAが対象差分のSHAに一致することを確認する。PR実行のartifact名末尾はcheckoutした一時merge commitのgithub.shaであり、head SHAとは異なる。
artifact ZIPとNSISのsetup.exeは別物。ZIPを展開してsetup.exeとそのSHA256を照合する。

```powershell
Get-FileHash -LiteralPath 'C:\検証\AI GUILD_0.1.0_x64-setup.exe' -Algorithm SHA256
Get-AuthenticodeSignature -LiteralPath 'C:\検証\AI GUILD_0.1.0_x64-setup.exe'
```

ファイル名は実際の生成物に置き換える。SHA256は改変検出用で発行者認証ではない。
署名なしのため警告や組織の実行制限があり得る。OSの保護機能を無効化しない。

## 既存データを保護したインストール検証

最初はスナップショット付きWindows VMまたは専用の新規Windowsユーザーを使用する。
同じ製品名/identifierなので既存開発版と並行して安全に分離されるインストーラーではない。
日常利用アカウントへ試験導入しない。

1. VMにWebView2が存在すること、AI GUILDの既存インストール/プロセスがないことを確認し、OS/build/WebView2版を記録。
2. setup.exeを対話実行。インストール先・終了状態・ショートカットを記録。完了画面に起動チェックがあれば外す。
3. 起動前に専用QAフォルダとマーカーを用意し、実際のインストール先を指定して起動する。

```powershell
$qaRoot = Join-Path $env:TEMP ('aiwv-validation-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $qaRoot | Out-Null
New-Item -ItemType File -Path (Join-Path $qaRoot '.ai-guild-qa') | Out-Null
$env:AI_GUILD_QA_DATA_DIR = $qaRoot
try {
    $app = Start-Process -FilePath 'C:\実際のインストール先\ai-guild.exe' -PassThru
    $app.WaitForExit()
} finally {
    Remove-Item Env:AI_GUILD_QA_DATA_DIR -ErrorAction SilentlyContinue
}
```

既存のqa_path.rsの仕組みを使用する。相対パス/未作成フォルダ/マーカーなしは拒否される。
ショートカットからの起動はこの環境変数を継承しないので、この試験では上記の専用シェルから起動する。

4. 画面表示、日本語、上部5名の素材、無料Paper/Mint、再起動後の保存状態を確認。起動だけを推論・投稿成功の証拠にしない。接続確認・実行・投稿は押さない。
5. インストール先licenses内の6文書をソースとSHA256照合。追加有料スキン・tests/fixtures ZIP・個人データの混入がないことを確認。
6. アプリを終了し、QAフォルダ全体を別フォルダへバックアップしてファイル相対名/SHA256一覧を記録。既存プロファイルも専用テストユーザー内に限り、アプリ終了中にフォルダ全体で保全する。SQLite本体だけでなくWAL/SHM、skins、presentation、WebViewデータも対象。作業先/成果物の外部フォルダは別途保全。
7. 同じパッケージを再インストール。起動前のQA/既存プロファイルのファイル一覧・SHA256を比較（インストール単体で変化しないこと）。起動後はDBファイルの単純ハッシュ一致ではなく、履歴・設定・スキン選択の論理内容を比較する。
8. 追加スキン保持の検証は自作の無害な試験用パッケージを使う。リポジトリの不正ZIP群を手動インストールしない。
9. アプリ終了後にアンインストール。データ削除選択が表示される場合は選択しない。インストール先アプリ/ショートカットの除去と、QA/既存プロファイル/外部成果物の保持を別々に確認する。再インストール後の復元も確認。
10. WebView2なし、空白/日本語ユーザー名、標準ユーザー、Windows 10/11、旧版からの更新、より古い版への降格拒否、アプリ起動中の更新/削除は独立したVMスナップショットで確認する。

通常プロファイルの想定場所は `%LOCALAPPDATA%\local.ai-guild.desktop`（main.rsのapp_local_data_dir）。実機では実際の保存先を確認する。
インストーラーへデータ削除/移行hookを追加していないことは、アンインストーラーの保持動作を実証したことにはならない。

## 認証と製品範囲

Codex本体・アカウント・認証情報を同梱しない。Core無料/追加スキンの境界、実行権限、承認、再送防止は既存実装のまま。
README記載のローカル/OSS向け認証と商用・ホスト型サービスの区別を維持する。
このパッケージ作成は商用認証の適合確認ではない。実サービスの接続・推論・投稿はこの検証には含めない。

## 検証記録（2026-11-01）

| 項目 | 今回の状態 |
| --- | --- |
| mainのSHA固定・設定/保存先/Core境界/ライセンス文書のソース確認 | 確認済み |
| 通常bundle=false/--no-bundleの維持 | 差分で確認 |
| 追加JSONとTauri公式設定項目の照合 | 静的確認、JSON/YAML構文解析成功 |
| ローカルnpm/check/build/boundary/native | 未実施（Linux、完全なソース/依存環境なし） |
| Windows CIとNSIS生成 | 成功。署名なしNSIS 3.65 MiB、SHA256は下記 |
| Windows CIの導入/同版再導入/削除 | 成功。6文書一致、合成プロファイル/skins/presentation保持。GUI起動は未検証 |
| 新規PCへ導入/起動/再導入/削除/データ保持 | 未実施、上記Windows実機手順が必要 |
| Codex実接続/推論/外部投稿 | 未実施、今回の範囲外 |

Linux環境にRust/PowerShell/Windows実行環境はなく、git cloneはネットワーク制限で403。GitHub連携の読み取りで対象ソースを確認した。
CI成功とインストール成功、実接続成功を混同しない。

実測証跡:
- [CI run 41263353171](https://github.com/brigadiergerald/ai-work-visualizer/actions/runs/41263353171): 成功。
- 検証した実装head SHA: `21bf25065048255e1be988a8ecfdc36d63e1580f`。
- CI checkout merge SHA: `ed74ad21f4bf8d79cc556a160be330d3725a9081`。
- Frontend: 109成功、境界: 3成功、Windows native: 162成功。依存監査も成功。
- [生成物ZIP](https://github.com/brigadiergerald/ai-work-visualizer/actions/runs/41263353171/artifacts/13066004062): `unsigned-windows-x64-validation-ed74ad21f4bf8d79cc556a160be330d3725a9081`。
- ZIPには `AI GUILD_0.1.0_x64-setup.exe` と `.sha256` を収録。保存期限は2026-11-08 11:10 JST予定（GitHub側の保持規則で変わる場合あり）。
- setup.exe SHA256: `fd75d458e03f48ea69f988427d8f6d70e8ccb4b29e8200d5ed36c5cb2e49f996`。
- 使い捨てGitHub-hosted Windows runnerでサイレント導入、6文書ハッシュ、署名なしアプリ、同版再導入、削除、合成プロファイル保持が成功。
- インストール先と合成プロファイルの検証であり、実際の履歴DB・WebView2・購入スキン・Windows 10実機・画面起動・実接続は未検証。
- この結果追記は文書のみの変更で、上記で検証したインストーラー設定/スクリプトを変更しない。

参考: [Tauri設定](https://v2.tauri.app/reference/config/)、[Tauri CLI](https://v2.tauri.app/reference/cli/)、[Windows installer](https://v2.tauri.app/distribute/windows-installer/)。
