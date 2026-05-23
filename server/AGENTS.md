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
