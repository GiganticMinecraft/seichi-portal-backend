# seichi-form-backend
 ## フォームログイン可能アカウント
 - Microsoftアカウント

 ※ [wiki](https://wiki.vg/Microsoft_Authentication_Scheme)を見る限りMicrosoftアカウントと連携すればマインクラフトアカウントを取得できそう

 を用いてアカウントログインが可能になるようにする
 
 また、以下のアカウント(連絡用)と連携ができる
 - Twitterアカウント
 - Discordアカウント

 ## レスポンス用APIについて
 RestAPIを利用する

 また、受け取る方式としてはJSONを利用する

## フォームの作成
url: POST /api/form/create

フォーム作成サンプル
```json
{
  "form_titles": [
    "test1",
    "test2"
  ],
  "form_id": 1
}
```

### 想定される返り値
- Success (成功)
- AlreadyExists (すでに存在する)

## フォームの削除
url: POST /api/form/delete

フォーム削除サンプル
```json
{
  "form_id": 1
}
```

### 想定される返り値
- Success (成功)
- NotExists (指定されたフォームが存在しなかった)