# ZFS Incremental Backup
Backup ZFS datasets to AWS S3 Glacier Deep to save forever

## Requirements
- Cost-effective. Uses AWS in the cheapest way possible (well, there is room for improvement).
- Incremental, so you only upload the new data that previous uploads don't already have.

## What this is made for
- When you take photos and videos and you want to save them **forever**
- You want to have that 99.999999999% (idk about you but I round this up to 100%) data durability, so that even if your other backups are unaccessible you can still recover the photos later
- You don't urgently need the photos. You can wait 2 days, or even months, to re-download all the photos. It's not about being able to see the photos at a given time. It's about *being able* to see the photos, even if you never look at them again. Cuz you spent so long taking them and organizing them, that it would be sad if you lost them (even if you never look at them again).
- You will not be deleting or modifying the photos. This backup tool does not do data deduplication so modifying/deleting could result in extra backup space used.
- You are okay with the files not being compressed. This is fine for photos and videos cuz they are already optimally compressed anyways. Compression could be added to this tool but for now to keep it simple it will not compress/decompress.
- You don't need extended attributes on the data. This tool does not handle extended attributes.
- When restoring, you want to restore the whole dataset and not specific files or folders

## Why did I make this and not use one of the bajillion existing tools?
- I don't want to store metadata in Standard S3 storage cuz it's expensive
- I want a tool that takes advantages of file systems with snapshots, so the backup solution doesn't need to add a snapshot "layer" on top of the FS
- I want a tool that works well specifically with AWS S3 Glacier Deep
- I want a tool that will spread out recovering data between multiple months to not go over the AWS 100GB/mo free download limit
- I want a tool that will efficiently retry when uploads / downloads fail
- I did not find an existing tool that restores Glacier Deep objects into Standard objects, waits until they are restored (could take 48 hours), and then downloads them

## Features
- Backs up ZFS snapshots (incrementally) to AWS S3 Glacier Deep
- Encrypting the data stored in AWS so Amazon cannot read the **contents** of files
- Uses [multi-part uploads](https://docs.aws.amazon.com/AmazonS3/latest/userguide/mpuoverview.html) to efficiently handle large interrupted uploads
- Only uploads the difference between two snapshots
- If, after 6 months, data stored on AWS could be reduced by re-uploading an entire snapshot and deleting the incremental changes, it will do that to save money
- Rust library as well as CLI, for flexibility in how you use this tool. You can make your custom program, scripts, and automation, or manually use the CLI.
- Handles restoring from AWS S3 Glacier Deep
- Spreads out downloads between multiple months to avoid AWS download costs after the 100GB of free downloads is used

## How it works
I did not write the code yet. Here is the plan (not fully planned).
### Create a ZFS dataset
You could use BTRFS or some other fs which can do snapshots, but for now it is made for ZFS.

## Getting started
### Create an AWS acccount

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

## Future Improvements
Here are some improvements I can think of:
- Being able to change the encryption password
- Supporting other file systems like BTRFS
- Supporting other backup destinations (especially if there is a lower-priced one)

### Why would these improvements be made?
- My personal setup changes (maybe I switch to BTRFS or something else other than ZFS)
- Someone asks for the feature and I feel like helping them (if it's not to hard then sure)
- Someone makes a PR and I merge the PR (sure I will merge if it's good)

## Story
Inspired by [Immich's documentation](https://immich.app/docs/overview/introduction#welcome) including a personal story of why it was created, I will also write my story here.

I was born after my parents (obviously). By the time I took my first picture, my family already had thousands of pictures that they wanted to keep somewhere. The pictures were just sitting on a SanDisk Extreme Pro 1TB Portable SSD ![Picture of SanDisk Extreme Pro 1TB Portable SSD](https://github.com/user-attachments/assets/26d9806e-be0e-47ba-9d94-572dae1bc534). Some of the photos were also located in other places, some weren't. There wasn't really a backup. We all agreed that we needed to backup the photos. As we all take pictures, we will need to keep storing more and more data.

So I came up with this plan: save the photos on multiple physical disks, and also save it on a cloud service for 100% (yes I'm rounding up) data durability. I found out about ZFS from a [Late Night Linux podcast](https://latenightlinux.com/). I realized that ZFS would be very nice for handling multiple disks and redundancy and self-healing all automatically at the file system level. I researched cloud storage option and found that [AWS S3 Glacier Deep Archive](https://docs.aws.amazon.com/AmazonS3/latest/userguide/glacier-storage-classes.html#GDA) is the cheapest option at $1/TB/mo.

But I needed a way of actually backing up the files (and being able to restore them, maybe even 40 years later, without writing code 40 years later) to AWS. I searched for existing tools, but none of them fit my scenario perfectly (although I don't think my scenario is that uncommon). That is why I made this tool.

## Why Rust
Do I need to explain? Isn't it obvious?

Personally I just like Rust, it's my favorite language, ever since I read the Rust book in December 2023.
