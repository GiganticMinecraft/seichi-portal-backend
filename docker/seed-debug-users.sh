#!/bin/sh
set -eu

echo "Seeding MariaDB with debug users..."

until mariadb-admin ping -h db -P 3306 -u root -proot --silent; do
  sleep 1
done

mariadb -h db -P 3306 -u root -proot seichi_portal < /seed-debug-users.sql

echo "MariaDB debug users have been loaded."
