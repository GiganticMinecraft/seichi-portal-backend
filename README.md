# seichi-form-backend
 ## フォームログイン可能アカウント
 - Microsoftアカウント

 ※ [wiki](https://wiki.vg/Microsoft_Authentication_Scheme)を見る限りMicrosoftアカウントと連携すればMinecraftアカウントを取得できそう

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
  "title": "お問い合わせフォーム",
  "questions": [
    {
      "title": "質問1",
      "description": "質問1の説明",
      "type": "text"
    },
    {
      "title": "質問2",
      "description": "質問2の説明",
      "type": "pulldown",
      "choices": [
        "選択肢1",
        "選択肢2"
      ]
    },
    {
      "title": "質問3",
      "description": "質問3の説明",
      "type": "checkbox",
      "choices": [
        "選択肢1",
        "選択肢2"
      ]
    }
  ]
}
```

### 想定される返り値
- Success (成功) StatusCode: 200

## フォームの削除
url: POST /api/form/delete

フォーム削除サンプル
```json
{
  "form_id": 1
}
```

### 想定される返り値
- Success (成功) StatusCode: 200
- NotExists (指定されたフォームが存在しなかった) StatusCode: 409
