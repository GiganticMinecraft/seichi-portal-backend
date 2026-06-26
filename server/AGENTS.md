# server/ AI Agent 向けコーディング規約

## `use` 宣言 — 原則と例外

型・トレイト・関数は原則 `use` でインポートし、コード本文では短名で参照する。
新しく型を追加・参照するときは、まずそのファイルの既存 `use` ブロックと実装の書き方に合わせる。

NG 例（AI がやりがちなミス）:

```rust
async fn handle(
    state: axum::extract::State<crate::server::AppState>,
) -> impl axum::response::IntoResponse {
    let id = domain::form::models::FormId::new(uuid::Uuid::new_v4());
}
```

OK 例:

```rust
use axum::{extract::State, response::IntoResponse};
use uuid::Uuid;

use crate::server::AppState;
use domain::form::models::FormId;

async fn handle(state: State<AppState>) -> impl IntoResponse {
    let id = FormId::new(Uuid::new_v4());
}
```

### 例外

`infra` 層の DTO ↔ ドメイン型変換では、DTO 型とドメイン型が同じ短名を持つことが多い。
このときは `use` でどちらかを隠すより、フルパスで両者を区別する。

```rust
impl TryFrom<CommentDto> for domain::form::comment::models::Comment {
    type Error = InfraError;
    fn try_from(dto: CommentDto) -> Result<Self, Self::Error> {
        Ok(domain::form::comment::models::Comment::from_raw_parts(...))
    }
}
```

---

## 宣言的・関数型スタイルの優先

単純な値の変換・絞り込み・収集では、`let mut` / 命令的 `for` ループよりイテレータチェーンを優先する。

特に、`Vec` を組み立てるだけの `let mut` + `for` + `push` は避け、`map` / `filter_map` / `flat_map` / `collect` を使う。

```rust
let result = items
    .into_iter()
    .flat_map(|item| item.try_into_read(&actor))
    .collect::<Vec<_>>();
```

構造体のフィールドを一部だけ変えた新しい値を返すときは、可能なら struct update syntax を使う。

```rust
pub fn change_title(self, title: Title) -> Self {
    Self { title, ..self }
}
```

---

## `sqlx` クエリの静的検証

静的に書ける SQL は、原則として `sqlx::query!`、`sqlx::query_as!`、`sqlx::query_scalar!` などの typed query マクロを使う。

`sqlx::query` や `sqlx::query_as` を使ってよいのは、SQL 文字列を実行時に組み立てる必要がある場合だけ。動的クエリにする場合は、なぜ typed query にできないかを近くのコメントか PR 説明に書く。

typed query を追加・変更した場合は、ルート `.env` の `DATABASE_URL` を使って `cargo sqlx prepare --workspace` を実行し、`.sqlx/` の更新をコミットに含める。

---

## Repository 境界での認可

Repository で権限が必要な操作は、呼び出し元の事前チェックだけに依存しない。

読み取り結果として認可対象を返す場合は `AuthorizationGuard<T, Read>` を返し、書き込みや削除など認可済みの値が必要な操作では `Allowed<T, Create>`、`Allowed<T, Update>`、`Allowed<T, Delete>` などを引数に要求する。

Handler や Usecase の `if` 文だけで認可を済ませず、Repository の型シグネチャで認可済みの経路を強制する。
