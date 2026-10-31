# Third-party dependencies

このソースは依存パッケージ本体を同梱しません。依存関係は npm / Cargo のロックファイルから取得します。各依存関係のライセンスは各権利者に帰属し、本体コードのApache-2.0へ変更されません。

以下は公開準備時にローカルのパッケージメタデータから確認した直接依存関係です。推移依存関係を含む完全なライセンス本文・著作権表示の代わりにはなりません。ビルド済みアプリを再配布する場合は、実際に組み込む依存関係のライセンス本文と表示義務を別途確認してください。

| Ecosystem | Package | Locked version | Declared license |
| --- | --- | --- | --- |
| npm | [@dagrejs/dagre](https://www.npmjs.com/package/@dagrejs/dagre) | 3.1.3 | MIT |
| npm | [@fontsource/pixelify-sans](https://www.npmjs.com/package/@fontsource/pixelify-sans) | 5.3.0 | OFL-1.1 |
| npm | [@tauri-apps/api](https://www.npmjs.com/package/@tauri-apps/api) | 2.12.4 | Apache-2.0 OR MIT |
| npm | [lucide-react](https://www.npmjs.com/package/lucide-react) | 1.61.0 | ISC |
| npm | [react](https://www.npmjs.com/package/react) | 19.3.1 | MIT |
| npm | [react-dom](https://www.npmjs.com/package/react-dom) | 19.3.1 | MIT |
| npm | [@tauri-apps/cli](https://www.npmjs.com/package/@tauri-apps/cli) | 2.12.4 | Apache-2.0 OR MIT |
| npm | [@types/dagre](https://www.npmjs.com/package/@types/dagre) | 0.7.54 | MIT |
| npm | [@types/react](https://www.npmjs.com/package/@types/react) | 19.3.0 | MIT |
| npm | [@types/react-dom](https://www.npmjs.com/package/@types/react-dom) | 19.3.0 | MIT |
| npm | [@vitejs/plugin-react](https://www.npmjs.com/package/@vitejs/plugin-react) | 6.1.2 | MIT |
| npm | [typescript](https://www.npmjs.com/package/typescript) | 7.0.2 | Apache-2.0 |
| npm | [vite](https://www.npmjs.com/package/vite) | 8.3.5 | MIT |
| npm | [vitest](https://www.npmjs.com/package/vitest) | 5.0.5 | MIT |
| Cargo | [base64](https://crates.io/crates/base64/0.21.7) | 0.21.7 | MIT OR Apache-2.0 |
| Cargo | [base64](https://crates.io/crates/base64/0.23.1) | 0.23.1 | MIT OR Apache-2.0 |
| Cargo | [chrono](https://crates.io/crates/chrono/0.4.45) | 0.4.45 | MIT OR Apache-2.0 |
| Cargo | [cssparser](https://crates.io/crates/cssparser/0.37.0) | 0.37.0 | MPL-2.0 |
| Cargo | [flate2](https://crates.io/crates/flate2/1.1.10) | 1.1.10 | MIT OR Apache-2.0 |
| Cargo | [html5ever](https://crates.io/crates/html5ever/0.39.0) | 0.39.0 | MIT OR Apache-2.0 |
| Cargo | [image](https://crates.io/crates/image/0.25.10) | 0.25.10 | MIT OR Apache-2.0 |
| Cargo | [regex](https://crates.io/crates/regex/1.13.1) | 1.13.1 | MIT OR Apache-2.0 |
| Cargo | [rusqlite](https://crates.io/crates/rusqlite/0.37.0) | 0.37.0 | MIT |
| Cargo | [serde](https://crates.io/crates/serde/1.0.229) | 1.0.229 | MIT OR Apache-2.0 |
| Cargo | [serde_json](https://crates.io/crates/serde_json/1.0.154) | 1.0.154 | MIT OR Apache-2.0 |
| Cargo | [sha2](https://crates.io/crates/sha2/0.10.9) | 0.10.9 | MIT OR Apache-2.0 |
| Cargo | [tauri](https://crates.io/crates/tauri/2.12.5) | 2.12.5 | Apache-2.0 OR MIT |
| Cargo | [uuid](https://crates.io/crates/uuid/1.29.0) | 1.29.0 | Apache-2.0 OR MIT |
| Cargo | [windows-sys](https://crates.io/crates/windows-sys/0.45.0) | 0.45.0 | MIT OR Apache-2.0 |
| Cargo | [windows-sys](https://crates.io/crates/windows-sys/0.61.2) | 0.61.2 | MIT OR Apache-2.0 |
| Cargo | [zip](https://crates.io/crates/zip/9.1.3) | 9.1.3 | MIT |
| Cargo | [tauri-build](https://crates.io/crates/tauri-build/2.7.4) | 2.7.4 | Apache-2.0 OR MIT |

キャラクター素材の条件は `CHARACTER_ASSETS_LICENSE.md` を参照してください。npmのフォントパッケージ等を改変・再配布する場合も、依存パッケージ自身の条件を確認してください。
