#!/bin/sh
set -eu

echo "Seeding MariaDB with debug users..."

until mariadb-admin ping -h db -P 3306 -u user -ppassword --silent; do
  sleep 1
done

mariadb -h db -P 3306 -u user -ppassword seichi_portal < /seed-debug-users.sql

echo "MariaDB debug users have been loaded."
