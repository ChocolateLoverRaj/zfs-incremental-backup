#!/usr/bin/env bash
# Destroy zpool
sudo zpool destroy zfs-incremental-backup-dev
truncate -s 0 ./dev/zpool

# Create zpool and dataset
truncate -s 64M ./dev/zpool
sudo zpool create zfs-incremental-backup-dev $PWD/dev/zpool
sudo zfs create zfs-incremental-backup-dev/test

# Stop minio container and delete volume
docker compose down
rm -r ./dev/minio_data
# Start mino container
docker compose up -d
# Wait for it to be online
sleep 10
# Create the bucket "test"
mc alias set minio http://localhost:9000 minioadmin minioadmin
mc mb minio/test
