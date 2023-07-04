# archivebot

Work-in-progress rewrite of Ragtag Archive bot.

## Testing

Create a local s3 bucket with `localstack`:

```sh
python -m venv venv
source ./venv/bin/activate
pip install localstack
localstack start -d
localstack ssh
awslocal s3api create-bucket --bucket test
```

Source the `.envrc.example` file, or copy it to `.envrc` and use `direnv` to
automatically source the file. Empty the `ARCHIVE_BASE_URL` environment variable
to use a mock / simulated archive site.

```sh
RUST_LOG=archivebot=debug cargo run
```
