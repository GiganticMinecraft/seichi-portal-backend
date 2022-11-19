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

 ## リクエストで受け取る内容
 アカウント情報{account_info}を必ず含める。

 ## 通報
 url: POST /api/form/report/{contents}
 
 - 報告内容の要約{report_title}
 - 通報内容{report_content}
 - 証拠のURL{evidence_url}
 - 通報対象のMinecraftID{target_minecraft_id}
 - 個人チャットの内容を含むか{is_include_private_chats}

 ## 不具合報告
  url: POST /api/form/bugReport/{contents}

 - 不具合の種類{bug-type}
 - 不具合の内容{bug-content}

## ご意見ご感想リクエスト
 url: POST /api/form/request/{contents}

- 投稿内容{content}

## 処罰への異議申し立て
url: POST /api/form/ban-objection/{contents}

- 申立内容{content}

## Modの使用可否
url: POST /api/form/can-mod-use/{contents}

- Mod名{mod_name}
- Modの内容{mod_content}
- Modの説明があるサイトのリンク{mod_explanation}

## 経験値のオーバーフロー
url: POST /api/form/exp-overflow/{contents}

- 内容{content}

## blacklistと表示される
url: POST /api/form/blacklist/{contents}

- IPアドレス{ip_address}

### 備考
1MCIDにつき1個だけ

## その他のお問い合わせ
url: POST /api/form/else-inquiry/{contents}

- 内容{content}