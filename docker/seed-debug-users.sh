#!/bin/sh
set -eu

echo "Seeding MariaDB with debug data..."

until mariadb-admin ping -h db -P 3306 -u user -ppassword --silent; do
  sleep 1
done

mariadb -h db -P 3306 -u user -ppassword seichi_portal < /minecraft-bans-schema.sql
mariadb -h db -P 3306 -u user -ppassword seichi_portal < /seed-debug-users.sql
mariadb -h db -P 3306 -u user -ppassword seichi_portal < /seed-debug-minecraft-bans.sql

echo "MariaDB debug data has been loaded."
