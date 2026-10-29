# wrac-examples

このリポジトリの保守と WRAC API の変更への追随は終了しました。
現在の実装例は、[WRAC Plugin Template](https://github.com/novonotes/wrac-plugin-template) の [WRAC Gain](https://github.com/novonotes/wrac-plugin-template/tree/main/plugins/wrac-gain) を参照してください。

> English version: [README.md](README.md)

## 構成

```text
examples/
  gain-basic/   基本的な audio effect の gain plugin。
  sine-synth/   MIDI note input でサイン波を出す instrument plugin。
  analyzer-3d/  リッチな3D表現のアナライザー
  echo-lama/    Delay 内蔵で vowel morphing する monophonic synth と専用 GUI。
```

## セットアップ

```sh
git submodule update --init --recursive
```

## ビルド

```sh
# すべての example を CLAP としてビルド
cargo xtask build --all --target=clap

# ひとつの example だけをビルド
cargo xtask build --plugin=sine-synth --target=clap

# release ビルド
cargo xtask build --plugin=sine-synth --target=clap --release

# VST3 / AU / Standalone は template submodule 内の wrapper builder を使う
cargo xtask build --plugin=gain-basic --target=vst3,au,standalone
```

`--plugin` に渡す値は、`examples/` 直下の example directory 名です。
たとえば `examples/sine-synth` は `--plugin=sine-synth` で指定します。

## プラグインのインストール

`install` は、ビルド済みのプラグインの artifact を DAW がスキャンするディレクトリにコピーします。
事前に `build` を実行しておく必要があります（`install` はビルドを行いません）。

```sh
# ひとつの example を user-local のプラグインフォルダにインストール
cargo xtask install --plugin=gain-basic

# すべての example をインストール
cargo xtask install --all

# release の artifact をインストール
cargo xtask install --plugin=gain-basic --release

# 特定のフォーマットだけをインストール
cargo xtask install --plugin=gain-basic --target=clap,vst3

# user-local ではなく system-wide にインストール
cargo xtask install --plugin=gain-basic --scope=system
```

デフォルトの target は OS ごとに決まります（macOS: clap, vst3, au / Windows・Linux: clap, vst3）。
`standalone` はプラグインフォーマットではないので、このコマンドではインストールできません。代わりに `launch` を使ってください。

scope ごとのインストール先:

| OS      | Scope  | CLAP                                 | VST3                                 | AU                                          |
| ------- | ------ | ------------------------------------ | ------------------------------------ | ------------------------------------------- |
| macOS   | user   | `~/Library/Audio/Plug-Ins/CLAP`      | `~/Library/Audio/Plug-Ins/VST3`      | `~/Library/Audio/Plug-Ins/Components`       |
| macOS   | system | `/Library/Audio/Plug-Ins/CLAP`       | `/Library/Audio/Plug-Ins/VST3`       | `/Library/Audio/Plug-Ins/Components`        |
| Windows | user   | `%LOCALAPPDATA%\Programs\Common\CLAP` | `%LOCALAPPDATA%\Programs\Common\VST3` | —                                           |
| Windows | system | `%CommonProgramFiles%\CLAP`           | `%CommonProgramFiles%\VST3`           | —                                           |
| Linux   | user   | `~/.clap`                            | `~/.vst3`                            | —                                           |
| Linux   | system | `/usr/lib/clap`                      | `/usr/lib/vst3`                      | —                                           |

インストール済みの artifact を削除するには `cargo xtask uninstall` を使います。`--plugin` / `--all` / `--target` は install と同じく指定でき、加えて `--scope=all|user|system`（デフォルトは `all`）と、削除せずに対象パスだけを表示する `--dry-run` があります。

```sh
cargo xtask uninstall --plugin=gain-basic
cargo xtask uninstall --plugin=gain-basic --dry-run
```

## Standalone の起動

```sh
cargo xtask build --plugin=gain-basic --target=standalone --release
cargo xtask launch --plugin=gain-basic --release
```

デバッグビルドを起動したい場合は、上記の両コマンドから `--release` を外してください。デバッグビルドは GUI を Vite dev server から読み込むため、起動前に下の [デバッグ版のGUIについて](#デバッグ版のguiについて) セクションに従って dev server を立ち上げる必要があります。

## デバッグ版のGUIについて

デバッグビルドでは GUI を埋め込みバンドルからではなく Vite dev server から読み込みます。
そのため、DAW や standalone でプラグインのウィンドウを開く前に dev server を起動しておく必要があります。起動していないと WebView は真っ白な画面のままになります。

リリースビルドでは `src-gui/dist` の内容がプラグインのバイナリに直接埋め込まれるため、dev server は不要です。

### dev server の起動

デバッグしたい example のディレクトリでターミナルを開き、Vite を起動します。デバッグ作業中はこのプロセスを起動したままにしておいてください。たとえば gain-basic plugin をデバッグするなら、以下のように実行します:

```sh
cd examples/gain-basic/src-gui
npm install   # 初回のみ
npm run dev
```

別のターミナルで debug モードのビルドを行い、install または launch します。

```sh
cargo xtask build --plugin=gain-basic
cargo xtask install --plugin=gain-basic # プラグインをインストール
# または
cargo xtask launch --plugin=gain-basic # standalone plugin を起動
```

`src-gui/` 配下のファイルを編集すると Vite の hot reload が走るため、フロントエンドの変更で Rust の再ビルドは必要ありません。

### ポート番号の変更

dev server の URL は Vite 側と Rust 側のどちらも `http://127.0.0.1:5173/` に固定されています。ポートを変更するには両方の編集が必要です。

1. `examples/<plugin>/src-gui/vite.config.ts` の `server.port` を変更。`strictPort: true` が設定されているため、指定したポートが使用中の場合でも Vite は別のポートにフォールバックしません。
2. `examples/<plugin>/src-plugin/src/gui/runtime.rs` の `#[cfg(debug_assertions)]` 内にある `url` のリテラルを同じポートに変更。
3. プラグインをリビルド（`cargo xtask build --plugin=<plugin>`）し、dev server を再起動。

ポート 5173 が他のプロセスに使われている場合、両側を書き換えるよりも、そのプロセスを停止するほうが簡単なことが多いです。

## メインリポジトリ

メインプロジェクトは [wrac-plugin-template](https://github.com/novonotes/wrac-plugin-template) です。
テンプレート、セットアップガイド、ビルドワークフロー、サポート窓口はそちらにあります。

このリポジトリには、そのテンプレートから作られた過去のサンプルが含まれています。
質問、フィードバック、互換性レポートは [wrac-plugin-template issues](https://github.com/novonotes/wrac-plugin-template/issues) または [discussions](https://github.com/novonotes/wrac-plugin-template/discussions) を使ってください。
