use std::io::{Read, Write};
use std::{fs::File, path::Path};

use libparted::{Device, Disk, DiskType, FileSystemType, Partition, PartitionType};

fn best_block_size(size: u64) -> usize {
    let mut bs = 1048576;
    loop {
        if size > bs && (size % bs) == 0 {
            return bs.try_into().unwrap();
        }
        bs /= 2;
    }
}

pub fn create_disk_image<'a>(
    cvd_dir: &Path,
    components: &[(&'a str, &'a str)],
    out_file: &Path,
) -> std::io::Result<Vec<(&'a str, &'a str, u64)>> {
    let mut parts = Vec::new();
    let mut out = File::create(out_file)?;

    let zeroes = vec![0; 20480];

    // Space reserved for GPT header
    out.write_all(&zeroes)?;

    for (image, name) in components {
        let size = if image.contains("blank") {
            let elems: Vec<&str> = image.split(':').collect();
            let size: u64 = (elems[1]).parse::<u64>().unwrap();
            let bs = best_block_size(size);
            let buf = vec![0u8; best_block_size(size)];
            let mut written = 0;
            loop {
                out.write_all(&buf)?;
                written += bs;
                if written >= size.try_into().unwrap() {
                    break;
                }
            }
            size
        } else {
            let mut src = File::open(cvd_dir.join(image))?;
            let metadata = src.metadata()?;
            let size = metadata.len();
            println!("image: {image} len={size}");
            let mut buf = vec![0u8; best_block_size(size)];
            let mut written = 0;
            loop {
                let n = src.read(&mut buf)?;
                out.write_all(&buf)?;
                written += n;
                if written >= size.try_into().unwrap() {
                    break;
                }
            }
            size
        };
        parts.push((*image, *name, size));
    }

    // Space reserved for GPT footer
    out.write_all(&zeroes)?;

    Ok(parts)
}

pub fn create_partitions(
    parts: Vec<(&str, &str, u64)>,
    out_file: &Path,
) -> Result<(), std::io::Error> {
    let mut dev = Device::new(out_file)?;
    let mut disk = Disk::new_fresh(&mut dev, DiskType::get("gpt").unwrap())?;

    let constraint = disk.constraint_any().unwrap();

    let mut start_sector = 40;

    for p in parts {
        let len: i64 = (((p.2 - 1) / 512) + 1).try_into().unwrap();
        Partition::new(
            &disk,
            PartitionType::PED_PARTITION_NORMAL,
            Some(&FileSystemType::get("ext2").unwrap()),
            start_sector,
            start_sector + len - 1,
        )
        .and_then(|mut part| {
            part.set_name(p.1).unwrap();
            disk.add_partition(&mut part, &constraint)
        })?;

        start_sector += len;
    }

    disk.commit()
}
