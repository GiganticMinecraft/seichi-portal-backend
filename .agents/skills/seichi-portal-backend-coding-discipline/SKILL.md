---
name: seichi-portal-backend-coding-discipline
description: Use when changing seichi-portal-backend Rust code that touches sqlx queries, database access, repositories, use cases, authorization boundaries, or domain type construction. Enforces typed sqlx query macros for static SQL, AuthorizationGuard/Allowed at Repository boundaries, and proper use of unsafe fn from_raw_parts for domain type reconstruction.
---

# seichi-portal-backend コーディング規約

## Overview

この skill は、seichi-portal-backend の実装で見落としやすい規約を守るために使う。

- 静的に書ける SQL は `sqlx` の typed query マクロで検証する。
- Repository の型シグネチャで、認可済みの値だけが危険な操作へ渡るようにする。
- `unsafe fn from_raw_parts` は永続化済みデータの復元専用であり、`unsafe` を削除しない。

## `sqlx` typed query

静的に書ける SQL では、原則として次のマクロを使う。

- `sqlx::query!`
- `sqlx::query_as!`
- `sqlx::query_scalar!`

`sqlx::query`、`sqlx::query_as`、`sqlx::query_scalar` を使ってよいのは、SQL 文字列を実行時に組み立てる必要がある場合だけ。

### 判断手順

1. SQL 文字列がリテラルとして書けるか確認する。
2. リテラルで書けるなら typed query マクロを使う。
3. 実行時に条件、並び順、IN 句の要素数などを組み立てる必要がある場合だけ、通常の `query` を使う。
4. 通常の `query` を使う場合は、なぜ typed query にできないかを近くのコメントか PR 説明に書く。
5. typed query を追加・変更したら、ルート `.env` の `DATABASE_URL` を使って `cargo sqlx prepare --workspace` を実行し、`.sqlx/` の更新を含める。

### 書き方

NG:

```rust
sqlx::query("DELETE FROM form_meta_data WHERE id = ?")
    .bind(id.to_string())
    .execute(executor)
    .await?;
```

OK:

```rust
sqlx::query!("DELETE FROM form_meta_data WHERE id = ?", id.to_string())
    .execute(executor)
    .await?;
```

動的クエリの例外:

```rust
// 回答項目の数に応じて `IN (...)` の placeholder 数が変わるため、
// typed query ではなく AssertSqlSafe で組み立てた SQL を実行する。
let sql = format!(
    "SELECT id, question_id FROM form_choices WHERE question_id IN ({})",
    std::iter::repeat_n("?", question_ids.len()).join(", ")
);

let rows = question_ids
    .iter()
    .fold(sqlx::query(AssertSqlSafe(&*sql)), |query, question_id| {
        query.bind(question_id.into_inner().to_string())
    })
    .fetch_all(&mut *txn)
    .await?;
```

## Repository 境界の認可

Repository で権限が必要な操作は、呼び出し元の事前チェックだけに依存しない。Repository trait と実装の型シグネチャで、認可済みの値だけが渡るようにする。

### 原則

- 読み取り可能性を判定してから返す値は `AuthorizationGuard<T, Read>` を返す。
- 作成、更新、削除などの操作は `Allowed<T, Create>`、`Allowed<T, Update>`、`Allowed<T, Delete>` を引数に要求する。
- 子要素の認可は、親の `Allowed` から派生させる既存の仕組みに合わせる。
- Handler や Usecase の `if` 文だけで認可を済ませない。
- Repository の trait、infra 実装、test repository のシグネチャをそろえる。

### 書き方

NG:

```rust
async fn delete_label_for_forms(&self, label: FormLabel) -> Result<(), Error>;
```

OK:

```rust
async fn delete_label_for_forms(
    &self,
    label: Allowed<FormLabel, Delete>,
) -> Result<(), Error>;
```

読み取りで認可前の値を返す必要がある場合:

```rust
async fn get_label_by_id(
    &self,
    id: FormLabelId,
) -> Result<Option<AuthorizationGuard<FormLabel, Read>>, Error>;
```

## `unsafe fn from_raw_parts` によるドメイン型の復元

このプロジェクトでは、ドメイン型の多くに `#[derive(UnsafeFromRawParts)]` を付けている。これにより `unsafe fn from_raw_parts` が生成され、データベースなどの永続化済みデータからドメイン型を復元するために使う。

### なぜ `unsafe` か

ドメイン型のコンストラクタ（`new` や `create` など）はビジネスルールに基づくバリデーションやデフォルト値の設定を行う。一方、データベースから読み出した値はすでにバリデーション済みであり、コンストラクタを通すと不要な検証が走ったり、ID の再生成など意図しない副作用が起きる。

`from_raw_parts` はこのバリデーションをスキップしてフィールドを直接セットする。Rust 標準ライブラリの `unsafe` とは異なりメモリ安全性の問題はないが、「信頼できるデータソースからの復元」という使用条件を呼び出し側に意識させるために `unsafe` を付けている。

### 使ってよい場所

- infra 層でデータベースの行をドメイン型に変換するとき（`TryFrom<Record>` の実装など）
- テストコードでドメイン型のテストデータを組み立てるとき

### 使ってはいけない場所

- handler や usecase で新しいドメインオブジェクトを作るとき → コンストラクタを使う
- ユーザー入力からドメイン型を組み立てるとき → バリデーション付きのコンストラクタを使う

### `unsafe` ブロックを削除しない

`from_raw_parts` の `unsafe` は意図的な設計判断であり、Rust 標準ライブラリの `unsafe` とは目的が異なる。「メモリ安全性に影響しないから `unsafe` は不要」という理由で削除しない。

## 実装後の確認

変更内容に応じて次を確認する。

- Rust コードを変更したら `cargo build` を実行する。
- 整形や lint の確認が必要な変更では `makers pretty` を実行する。
- typed query を追加・変更したら `cargo sqlx prepare --workspace` を実行し、`.sqlx/` の差分を確認する。
- 認可境界を変えたら、domain repository trait、infra repository 実装、test repository 実装の型が一致していることを確認する。
