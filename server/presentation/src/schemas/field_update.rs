use serde::{Deserialize, Deserializer};

/// 部分更新リクエストにおける 1 フィールドの指定。
///
/// キーを省略すると `Unchanged`、`null` を指定すると `Clear`、値を指定すると `Set` になる。
/// フィールド型を `Option<T>` にすると、serde が `null` を見た時点で `T::deserialize` を
/// 呼ばずに `None` を返すため、キー省略と `null` 明示を区別できない。
///
/// このフィールドには必ず `#[serde(default)]` を付ける。付け忘れると、キーがないときに
/// serde の `missing_field` デシリアライザが `visit_none` を返し、`null` 明示と同じ
/// `Clear` になるため、キーを省略した PUT が既存の値を消してしまう。
/// コンパイルエラーにならないので、テストでキー省略が `Unchanged` になることを確かめる。
///
/// また `Option<FieldUpdate<T>>` と書いてはならない。外側の `Option` が `null` を吸って
/// `Unchanged` を返してしまい、`FieldUpdate` を導入した理由そのもの（三値が二値に潰れる）
/// が再発する。
#[derive(Debug, Default)]
pub enum FieldUpdate<T> {
    #[default]
    Unchanged,
    Clear,
    Set(T),
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for FieldUpdate<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // キーが存在するときだけこの impl が呼ばれるため、
        // ここで観測できる `None` は `null` 明示だけを意味する。
        Option::<T>::deserialize(deserializer).map(|value| match value {
            Some(value) => Self::Set(value),
            None => Self::Clear,
        })
    }
}
