# Running SQLx migrations

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
