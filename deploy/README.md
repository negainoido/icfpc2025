デプロイ用の作業用スクリプトです

## 準備

GCP上のインスタンスにログインできる必要があります。下のコマンドが通ることを確認してください

```
gcloud compute ssh --zone "us-west1-b" "instance-20250824-043241" --project "negainoido"
```

その後、`gcloud compute config-ssh`でsshのコンフィグを生成後、対象のインスタンスのHostを`negainoido`に変更してください（別の名前をセットしたい場合は、デプロイ時に環境変数`SERVER`にその名前をセット）


## デプロイ方法

```
./deploy.sh
```
