# Running SQLx migrations

## マイグレーションの運用ポリシー

- **適用済みのマイグレーションファイルは編集しない。** sqlx は適用時に checksum を
  `_sqlx_migrations` に記録し、次回起動時に検証する。適用済みファイルを編集すると
  checksum 不一致でアプリケーションが起動できなくなる。
- **スキーマ変更は新しいマイグレーションファイルとして追加する**
  (`cargo sqlx migrate add MIGRATION_NAME`)。初期マイグレーション
  (`20220101000001_create_table`) への追記で済ませない。既存データベースには
  `CREATE TABLE IF NOT EXISTS` が no-op となり、変更が適用されないまま
  スキーマドリフトを起こす (2026-08-01 の本番障害の原因)。
- 上記 2 点は CI の `migration-freeze-check` / `migration-convergence-check`
  ジョブで機械的に検証される。

## fixtures/

`fixtures/legacy_schema.sql` は SeaORM 時代の本番スキーマを再現するフィクスチャで、
CI の `migration-convergence-check` が使用する。レガシースキーマへ全マイグレーションを
適用した結果が、空のデータベースへ適用した結果と同一スキーマへ収束することを検証する。


- Generate a new migration file
    ```sh
    cargo sqlx migrate add MIGRATION_NAME
    ```
- Apply all pending migrations
    ```sh
    cargo sqlx migrate run --source ./migration/migrations
    ```
- Rollback the last applied migration
    ```sh
    cargo sqlx migrate revert --source ./migration/migrations
    ```
- Check the status of all migrations
    ```sh
    cargo sqlx migrate info --source ./migration/migrations
    ```

`server/migration` crate also exposes the embedded migrator used by the application at startup.
