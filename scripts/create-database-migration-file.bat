@echo off
CALL cd /d %~dp0

ECHO Please enter a migration file name.
SET FILE_NAME =
SET /P FILE_NAME=

CALL sea-orm-cli migrate generate %FILE_NAME% -d ../server/migration/
