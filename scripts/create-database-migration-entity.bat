@echo off
CALL cd /d %~dp0

REM see https://www.sea-ql.org/SeaORM/docs/next/generate-entity/sea-orm-cli/

CALL sea-orm-cli generate entity -o ./server/database/src/entities/