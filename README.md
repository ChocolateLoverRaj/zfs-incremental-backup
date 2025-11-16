# ZFS Incremental Backup
A solution to incrementally backup ZFS datasets to S3 cold storage.

## How it works
- Use any ZFS dataset
- This program takes a snapshot when you run it. You can customize the snapshot naming pattern.
- This program uses `zfs send` and then uploads its output to S3. For the lowest cost, use the "coldest" storage available, such as `DEEP_ARCHIVE` on AWS. Files are uploaded in chunks as multiple S3 objects to avoid multi-part uploads, which are super expensive. 
- That's it! All of the hard work is done by ZFS itself.

## Restoring data
I didn't write a program to restore, mostly because automatically restoring and downloading cold objects is tricky. But the structure is very simple, so you should be able to manually restore or write a script / program to do it. To download a snapshot / diff, download all of the chunks (labeled `0`, `1`, `2`, etc), and concatenate them. Then input that file into `zfs receive`. Start with the first snapshot (`backup0`), and then apply the incremental diffs to restore the next snapshot (`backup0_backup1`, `backup1_backup2`, etc).

## What this is made for
This program is optimized for when you have ZFS datasets where you only add files, not delete them. This way, it frequently can back up your dataset using `zfs snapshot` and `zfs send -i`. Only the S3 PUT operation needs to be used, making this work nicely with cold storage.
ects into Standard objects, waits until they are restored (could take 48 hours), and then downloads them

## Features
- Backs up ZFS snapshots (incrementally) to AWS S3 Glacier Deep
- Uses the `-w` flag when doing `zfs send`, so encrypted datasets stay encrypted as they are on S3
- Will gracefully continue its operation where it left off in the event of a program crash
- Purposely does not use multi-part uploads to save money
- Specifically designed for use with ZFS to let ZFS do the heavy lifting  

## Installation
This program is written in [Rust](https://rust-lang.org/) and packaged with [Nix](https://nixos.org/). This program contains a `flake.nix`, so installing through Nix is as simple as importing this flake.

## Development / Building Manually
First, install Rust. Then, run `cargo build` to build.

## Testing
`flake.nix` contains an integration test involving three NixOS virtual machines. See https://nixos.org/manual/nixos/stable/#sec-call-nixos-test-outside-nixos for information about how it works.

## Usage
### Initialize
This program stores its state in a file. Figure out where you will store the file. I recommend storing the file in a different dataset within the same.
```bash
zfs-incremental-backup init --help
```
for a full list of inputs.

#### `--snapshot-fix`
I recommend making this `"backup"`

#### `--object-prefix`
If your S3 bucket is entirely dedicated to backing up a single ZFS dataset with this program, leave it as `""`. If you want to dedicate a specific "folder" in the S3 bucket for this tool, make this `"folder/"` (remember the trailing `/`).

### Run a backup
The `run` subcommand will either resume a previous interrupted backup operation, or it will create and back up a new snapshot. This program also needs a directory to store temporary files (which include the entire output of `zfs send`). You will probably not have enough RAM for the temporary files to be stored in RAM. So keep it in a place with enough disk space.

#### `--storage-class`
Do your research to figure out which one you want to use. I use `DEEP_ARCHIVE` for the lowest cost.

#### `--chunk-size`
If using AWS, set this to `5000000000` (5GB), which is the largest allowed object size for a single part upload, and is the most cost efficient chunk size.

### Running backups automatically
Feel free to use systemd services or other scheduling tools to call `zfs-incremental-backup init`. Just don't run multiple instances `zfs-incremental-backup init` on the *same save data file* at the same time. You can run multiple `zfs-incremental-backup init` to back up **different** datasets at the same time, but consider that you will probably be limited by upload speed anyways, so it may not save time running them in parallel.

## Set up an AWS bucket 

### Create an AWS account

### Create an S3 bucket

### Create an AWS IAM user
- Click "Create user"
- In step 2 select "Attach policies directly"
- Click the "Create policy" button
- Click on "JSON"
- Paste the following JSON:
```json
{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Sid": "VisualEditor0",
            "Effect": "Allow",
            "Action": "s3:*",
            "Resource": "*"
        }
    ]
}
```
- Continue creating the user (no other things to set)

### Create an access key
- Click on the user
- Click "Create access key"
- Select the "Local code" use case
- Continue creating it

## Save the access key
- Get the AWS ClI
- Run `aws configure` and enter the credentials and region. Leave "Default output format" empty

## Story
Inspired by [Immich's documentation](https://immich.app/docs/overview/introduction#welcome) including a personal story of why it was created, I will also write my story here.

I was born after my parents (obviously). By the time I took my first picture, my family already had thousands of pictures that they wanted to keep somewhere. The pictures were just sitting on a SanDisk Extreme Pro 1TB Portable SSD ![Picture of SanDisk Extreme Pro 1TB Portable SSD](https://github.com/user-attachments/assets/26d9806e-be0e-47ba-9d94-572dae1bc534). Some of the photos were also located in other places, some weren't. There wasn't really a backup. We all agreed that we needed to backup the photos. As we all take pictures, we will need to keep storing more and more data.

So I came up with this plan: save the photos on multiple physical disks, and also save it on a cloud service for 100% (yes I'm rounding up) data durability. I found out about ZFS from a [Late Night Linux podcast](https://latenightlinux.com/). I realized that ZFS would be very nice for handling multiple disks and redundancy and self-healing all automatically at the file system level. I researched cloud storage option and found that [AWS S3 Glacier Deep Archive](https://docs.aws.amazon.com/AmazonS3/latest/userguide/glacier-storage-classes.html#GDA) is the cheapest option at $1/TB/mo.

## Why did I make this and not use one of the bajillion existing tools?
- Other tools don't really take advantage of ZFS snapshots
- Some other tools work with cold storage, but they don't have a good method of restoring
