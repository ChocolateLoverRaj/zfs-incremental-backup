# Start all machines in parallel
uploader.start()
server.start()
downloader.start()

zpool_path = "/tmp/zpool"
zpool_name = "zpool"
dataset_name = "dataset"
snapshot_prefix = "backup"
bucket = "zfs-sends"
object_prefix = f'{dataset_name}/'
save_data_path = "/tmp/auto_data.ron"
temp_dir = "/tmp/backup_temp"
chunk_size = 30_000
file_0_name = "file_0.txt"
file_1_name = "file_1.txt"

# Initialize S3 server
server.wait_for_unit("minio")
server.wait_for_open_port(9000)
server.succeed("mc alias set minio http://localhost:9000 minioadmin minioadmin")
server.succeed(f"mc mb minio/{bucket}")

uploader.wait_for_unit("default.target")
uploader.succeed(f'truncate -s 64M {zpool_path}')
uploader.succeed(f'zpool create {zpool_name} {zpool_path}')
uploader.succeed(f'zfs create {zpool_name}/{dataset_name}')
uploader.succeed(f'zfs-incremental-backup init --zpool {zpool_name} --dataset {dataset_name} --snapshot-prefix {snapshot_prefix} --bucket {bucket} --object-prefix "{object_prefix}" --save-data-path {save_data_path}')
uploader.succeed(f'mkdir {temp_dir}')
# Create a snapshot with a file
uploader.succeed(f'touch /{zpool_name}/{dataset_name}/{file_0_name}')
uploader.succeed(f'zfs-incremental-backup run --save-data-path {save_data_path} --temp-dir {temp_dir} --storage-class STANDARD --chunk-size {chunk_size} --dev --dev-endpoint http://server:9000')
# Create a snapshot with another file
uploader.succeed(f'touch /{zpool_name}/{dataset_name}/{file_1_name}')
uploader.succeed(f'zfs-incremental-backup run --save-data-path {save_data_path} --temp-dir {temp_dir} --storage-class STANDARD --chunk-size {chunk_size} --dev --dev-endpoint http://server:9000')

# Verify that the backup can be restored
downloader.wait_for_unit("default.target")
downloader.succeed(f'truncate -s 64M {zpool_path}')
downloader.succeed(f'zpool create {zpool_name} {zpool_path}')
downloader.succeed("mc alias set minio http://server:9000 minioadmin minioadmin")
# Verify backup0
downloader.succeed(f"mc cat minio/{bucket}/{object_prefix}backup0/0 minio/{bucket}/{object_prefix}backup0/1 | zfs receive {zpool_name}/{dataset_name}")
downloader.succeed(f'ls /{zpool_name}/{dataset_name}/{file_0_name}')
downloader.fail(f'ls /{zpool_name}/{dataset_name}/{file_1_name}')
# Verify backup1
downloader.succeed(f"mc cat minio/{bucket}/{object_prefix}backup0_backup1/0 | zfs receive {zpool_name}/{dataset_name}")
downloader.succeed(f'ls /{zpool_name}/{dataset_name}/{file_0_name}')
downloader.succeed(f'ls /{zpool_name}/{dataset_name}/{file_1_name}')
