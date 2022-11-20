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
  "form_name": "formName",
  "form_questions": [
    {
      "question": "question1",
      "description": "description1",
      "question_type": "text"
    },
    {
      "question": "question2",
      "description": "description2",
      "question_type": "pulldown",
      "choices": [
        "value1",
        "value2"
      ]
    },
    {
      "question": "question3",
      "description": "description3",
      "question_type": "checkbox",
      "choices": [
        "value1",
        "value2"
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
