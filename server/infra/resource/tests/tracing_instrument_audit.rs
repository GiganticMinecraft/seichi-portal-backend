//! `#[tracing::instrument]` の引数が暗黙にスパン属性へ記録されないことを守るテスト。
//!
//! 引数を暗黙記録すると、認証情報・回答内容などの PII や `ConnectionPool` の
//! Debug 出力（DB 接続情報・Meilisearch API キーを含む）がスパン属性に漏れる。
//! そのため、すべての `#[tracing::instrument]` に `skip_all` を必須とし、
//! 記録したい値は `fields(...)` で明示的に指定する。

use std::{
    fs,
    path::{Path, PathBuf},
};

fn server_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("resource crate must be under server/infra/resource")
        .to_path_buf()
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("server directory must be readable") {
        let path = entry.expect("directory entry must be readable").path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
}

/// `#[tracing::instrument` から始まる attribute 全体を、角括弧の対応を数えて取り出す。
fn instrument_attrs(source: &str) -> Vec<(usize, String)> {
    // この関数自身のリテラルにマッチしないよう、検索パターンは実行時に組み立てる
    let needle = format!("#[{}", "tracing::instrument");

    source
        .match_indices(&needle)
        .map(|(start, _)| {
            let mut depth = 0usize;
            let end = source[start..]
                .char_indices()
                .find_map(|(offset, character)| match character {
                    '[' => {
                        depth += 1;
                        None
                    }
                    ']' => {
                        depth -= 1;
                        (depth == 0).then_some(start + offset + 1)
                    }
                    _ => None,
                })
                .expect("attribute brackets must be balanced");

            let line = source[..start].lines().count();
            (line, source[start..end].to_string())
        })
        .collect()
}

#[test]
fn all_tracing_instruments_use_skip_all() {
    let mut files = Vec::new();
    collect_rust_files(&server_dir(), &mut files);

    let this_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/tracing_instrument_audit.rs");

    let violations = files
        .iter()
        .filter(|path| **path != this_file)
        .flat_map(|path| {
            let source = fs::read_to_string(path).expect("source file must be readable");
            instrument_attrs(&source)
                .into_iter()
                .filter(|(_, attr)| !attr.contains("skip_all"))
                .map(|(line, attr)| format!("{}:{line}: {attr}", path.display()))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "skip_all のない #[tracing::instrument] が見つかりました。\
         引数の暗黙記録は PII や接続情報がスパン属性に漏れる原因になるため、\
         skip_all を付け、記録したい値は fields(...) で明示してください:\n{}",
        violations.join("\n")
    );
}
