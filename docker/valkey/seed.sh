#!/bin/sh
set -eu

echo "Seeding Valkey with debug session data..."

# ADMINISTRATOR ロールのデバッグ用セッション
valkey-cli -h valkey -p 6379 SET "debug_session" '{"name":"test_user","id":"478911be-3356-46c1-936e-fb14b71bf282","role":"ADMINISTRATOR"}'

# STANDARD_USER ロールのデバッグ用セッション
valkey-cli -h valkey -p 6379 SET "debug_session_standard" '{"name":"test_std_user","id":"5cb955fb-5a05-4729-93ea-edcec7001001","role":"STANDARD_USER"}'

# seichi-portal-frontend のデバッグモード (NEXT_PUBLIC_DEBUG_MODE=true) が送るセッション。
# debug_session と同じ ADMINISTRATOR ユーザーにする
valkey-cli -h valkey -p 6379 SET "debug_user" '{"name":"test_user","id":"478911be-3356-46c1-936e-fb14b71bf282","role":"ADMINISTRATOR"}'

echo "Valkey seed data has been loaded."
