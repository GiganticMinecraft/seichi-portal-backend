# server/ AI Agent 向けコーディング規約

## `use` 宣言 — 原則と例外

**原則**: 型・トレイト・関数は `use` でインポートし、コード本文では短名で参照する。

NG 例（AI がやりがちなミス）:

```rust
// NG: use せずフルパスで書く
async fn handle(
    state: axum::extract::State<crate::server::AppState>,
) -> impl axum::response::IntoResponse {
    let id = domain::form::models::FormId::new(uuid::Uuid::new_v4());
}
```

OK 例:

```rust
use axum::{extract::State, response::IntoResponse};
use domain::form::models::FormId;
use uuid::Uuid;

async fn handle(state: State<AppState>) -> impl IntoResponse {
    let id = FormId::new(Uuid::new_v4());
}
```

補足:

- 同じモジュールから複数の型をインポートするときは `{}` でグループ化する
- 外部クレートを先に並べ、1 行空けて内部クレート（`domain`, `crate::` など）を書く（既存ファイルの並び順に従う）

### 例外 — 名前衝突時はフルパスが正しい

`infra` 層の DTO ↔ ドメイン型変換では、DTO 型とドメイン型が同じ短名を持つことが多い。
このときは `use` でどちらかを隠すより、フルパスで両者を区別するのが正しいスタイル（`dto.rs`、`messaging/schema.rs`、`messaging/connection.rs` が該当）。

```rust
// OK: 同名の型が衝突するので impl ヘッダーとボディをフルパスで書く
impl TryFrom<CommentDto> for domain::form::comment::models::Comment {
    type Error = InfraError;
    fn try_from(dto: CommentDto) -> Result<Self, Self::Error> {
        Ok(domain::form::comment::models::Comment::from_raw_parts(...))
    }
}
```

**判断基準**: 新しく型を追加・参照するとき、まずそのファイルの既存 `use` ブロックと既存の実装を見て、フルパスが使われているパターンかどうか確認する。

---

## 宣言的・関数型スタイルの優先

**原則**: `let mut` / 命令的 `for` ループよりイテレータチェーンを使った宣言的スタイルを優先する。

NG 例:

```rust
// NG: mutable な Vec を for ループで組み立てる
let mut result = vec![];
for item in items {
    if let Ok(read) = item.try_into_read(&actor) {
        result.push(read);
    }
}
```

OK 例:

```rust
// OK: flat_map で None/Err を除外しながら収集
let result = items
    .into_iter()
    .flat_map(|item| item.try_into_read(&actor))
    .collect::<Vec<_>>();
```

### 追加パターン

構造体のフィールドを 1 つ変えた新しい値を返す → struct update syntax:

```rust
// NG
let mut updated = self.clone();
updated.title = new_title;
updated

// OK: self を consume して新しい値を返す
pub fn change_title(self, title: Title) -> Self {
    Self { title, ..self }
}
```

`Result` のコレクション:

```rust
// OK
items
    .into_iter()
    .map(|guard| guard.try_into_read(actor))
    .collect::<Result<Vec<_>, _>>()
    .map_err(Into::into)
```

**判断基準**: `let mut` を書こうとしたら、まずイテレータチェーンで同等に書けないか検討する。「変更量を最小にする」という理由で `push` を追加するより、変換全体を宣言的に書き直すほうがこのプロジェクトでは正しい。
