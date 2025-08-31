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

## github action

`.github/workflows/deploy.yml` により、mainブランチへのpushまたは手動実行でGCP VMへの自動デプロイが可能です。

### 必要なGitHub Secrets設定

リポジトリの Settings > Secrets and variables > Actions で以下のシークレットを設定してください：

#### `DEPLOY_SSH_PRIVATE_KEY`
GCP VMにSSH接続するための秘密鍵（ed25519形式）

```bash
# 秘密鍵の生成例（ローカル）
ssh-keygen -t ed25519 -f ~/.ssh/gcp_deploy_key
# 公開鍵をGCP VMの~/.ssh/authorized_keysに追加
# 秘密鍵の内容をGitHub Secretsに設定
cat ~/.ssh/gcp_deploy_key
```

#### `DEPLOY_HOST` 
デプロイ先のホスト名またはIPアドレス


#### `DEPLOY_USER`
GCP VM上のデプロイ用ユーザー名。sudo可能でかつhome directoryが存在するものを指定してください

