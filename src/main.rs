use std::collections::HashMap;
use std::error;
use std::fmt;
use std::path::PathBuf;
use std::process::exit;

use clap::Parser;
use tempdir::TempDir;

mod disk;
use disk::{create_disk_image, create_partitions};
mod components;
use components::{create_bootconfig, create_uboot, create_vbmeta, transform_sparse_images, Arch};

const SYSTEM_COMPONENTS: &[(&str, &str)] = &[
    ("blank:1048576", "misc"),
    ("boot.img", "boot_a"),
    ("boot.img", "boot_b"),
    ("init_boot.img", "init_boot_a"),
    ("init_boot.img", "init_boot_b"),
    ("vendor_boot.img", "vendor_boot_a"),
    ("vendor_boot.img", "vendor_boot_b"),
    ("vbmeta.img", "vbmeta_a"),
    ("vbmeta.img", "vbmeta_b"),
    ("vbmeta_system.img", "vbmeta_system_a"),
    ("vbmeta_system.img", "vbmeta_system_b"),
    ("vbmeta_vendor_dlkm.img", "vbmeta_vendor_dlkm_a"),
    ("vbmeta_vendor_dlkm.img", "vbmeta_vendor_dlkm_b"),
    ("vbmeta_system_dlkm.img", "vbmeta_system_dlkm_a"),
    ("vbmeta_system_dlkm.img", "vbmeta_system_dlkm_b"),
    ("super.img", "super"),
    ("userdata.img", "userdata"),
    ("blank:67108864", "metadata"),
];

const PROPERTIES_COMPONENTS: &[(&str, &str)] = &[
    ("uboot_env.img", "uboot_env"),
    ("vbmeta.img", "vbmeta"),
    ("blank:1048576", "frp"),
    ("bootconfig", "bootconfig"),
];

#[derive(Debug)]
enum Error {
    Bootconfig(std::io::Error),
    DiskImage(std::io::Error),
    Partitions(std::io::Error),
    TransformSparse(std::io::Error),
    Uboot(std::io::Error),
    Vbmeta(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl error::Error for Error {}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Arguments {
    /// Architecture of the source images
    #[arg(short, long, value_enum)]
    arch: Option<Arch>,

    /// Directory containing the Android Cuttlefish images
    cvd_dir: PathBuf,

    /// Output file for the system disk image
    #[arg(short, long, value_name = "FILE")]
    system: Option<PathBuf>,

    /// Output file for the properties disk image
    #[arg(short, long, value_name = "FILE")]
    props: Option<PathBuf>,

    /// Output file for the virgl variant of the properties disk image
    #[arg(short, long, value_name = "FILE")]
    virgl_props: Option<PathBuf>,
}

fn create_disk_images(args: Arguments) -> Result<(), Error> {
    let cvd_dir = args.cvd_dir.canonicalize().unwrap();
    let out_system = args.system.unwrap_or_else(|| PathBuf::from("system.img"));
    let out_props = args
        .props
        .unwrap_or_else(|| PathBuf::from("properties.img"));
    let out_virgl_props = args
        .virgl_props
        .unwrap_or_else(|| PathBuf::from("properties_virgl.img"));
    let arch = args.arch.unwrap_or({
        if cfg!(target_arch = "aarch64") {
            Arch::Aarch64
        } else {
            Arch::X86_64
        }
    });

    let envs = HashMap::from([
        ("HOME", &cvd_dir),
        ("ANDROID_TZDATA_ROOT", &cvd_dir),
        ("ANDROID_ROOT", &cvd_dir),
    ]);

    println!("Transforming sparse images if needed");
    transform_sparse_images(&cvd_dir, &envs).map_err(Error::TransformSparse)?;

    println!("Creating {} disk image", out_system.display());
    let parts =
        create_disk_image(&cvd_dir, SYSTEM_COMPONENTS, &out_system).map_err(Error::DiskImage)?;
    create_partitions(parts, &out_system).map_err(Error::Partitions)?;

    let tmp_dir = TempDir::new("cvd2img").unwrap().into_path();

    println!("Creating persistent components");
    create_uboot(&cvd_dir, &tmp_dir, &envs).map_err(Error::Uboot)?;
    create_vbmeta(&cvd_dir, &tmp_dir, &envs).map_err(Error::Vbmeta)?;
    create_bootconfig(&cvd_dir, &tmp_dir, &envs, &arch, false).map_err(Error::Bootconfig)?;

    println!("Creating {} disk image", out_props.display());
    let parts =
        create_disk_image(&tmp_dir, PROPERTIES_COMPONENTS, &out_props).map_err(Error::DiskImage)?;
    create_partitions(parts, &out_props).map_err(Error::Partitions)?;

    create_bootconfig(&cvd_dir, &tmp_dir, &envs, &arch, true).map_err(Error::Bootconfig)?;
    println!("Creating {} disk image", out_virgl_props.display());
    let parts = create_disk_image(&tmp_dir, PROPERTIES_COMPONENTS, &out_virgl_props)
        .map_err(Error::DiskImage)?;
    create_partitions(parts, &out_virgl_props).map_err(Error::Partitions)?;

    Ok(())
}

fn main() {
    let args = Arguments::parse();

    if let Err(e) = create_disk_images(args) {
        println!("Image creation failed: {e}");
        exit(-1);
    }
}
