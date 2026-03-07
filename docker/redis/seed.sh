#!/bin/sh
set -eu

echo "Seeding Redis with debug session data..."

# ADMINISTRATOR ロールのデバッグ用セッション
redis-cli -h redis -p 6379 JSON.SET "debug_session" "$" '{"name":"test_user","id":"478911be-3356-46c1-936e-fb14b71bf282","role":"ADMINISTRATOR"}'

# STANDARD_USER ロールのデバッグ用セッション
redis-cli -h redis -p 6379 JSON.SET "debug_session_standard" "$" '{"name":"test_standard_user","id":"5cb955fb-5a05-4729-93ea-edException001","role":"STANDARD_USER"}'

echo "Redis seed data has been loaded."
