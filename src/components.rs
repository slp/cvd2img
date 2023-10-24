use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
};

use clap::ValueEnum;

const SPARSE_MAGIC: [u8; 4] = [0x3A, 0xFF, 0x26, 0xED];

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Arch {
    X86_64,
    Aarch64,
}

fn call_simg2img(
    cvd_dir: &Path,
    envs: &HashMap<&str, &PathBuf>,
    image: &str,
) -> Result<(), std::io::Error> {
    let src = cvd_dir.join(image);
    let tmp = src.with_extension("tmp");

    match Command::new(cvd_dir.join("bin/simg2img"))
        .arg(&src)
        .arg(&tmp)
        .envs(envs)
        .stderr(std::process::Stdio::inherit())
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                println!("Can't find simg2img in {}", cvd_dir.display());
            } else {
                println!("Error executing simg2img: {err}");
            }
            std::process::exit(-1);
        }
    };

    std::fs::rename(tmp, src)
}

fn is_sparse(cvd_dir: &Path, image: &str) -> Result<bool, std::io::Error> {
    let mut f = File::open(cvd_dir.join(image))?;
    let mut buf: [u8; 4] = [0; 4];
    f.read_exact(&mut buf)?;

    if buf == SPARSE_MAGIC {
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn transform_sparse_images(
    cvd_dir: &Path,
    envs: &HashMap<&str, &PathBuf>,
) -> Result<(), std::io::Error> {
    let images = vec!["super.img", "userdata.img"];
    for img in images {
        if is_sparse(cvd_dir, img)? {
            call_simg2img(cvd_dir, envs, img)?;
        }
    }
    Ok(())
}

pub fn create_uboot(
    cvd_dir: &Path,
    tmp_dir: &Path,
    envs: &HashMap<&str, &PathBuf>,
) -> Result<(), std::io::Error> {
    let uboot_env_path = tmp_dir.join("uboot_env.img");
    let uboot_env_input_data = b"uenvcmd=setenv bootargs \"$cbootargs console=hvc0 earlycon=pl011,mmio32,0x9000000 \" && run bootcmd_android";
    let uboot_env_input_path = tmp_dir.join("uboot_env_input");

    let mut f = File::create(&uboot_env_input_path)?;
    f.write_all(uboot_env_input_data)?;
    drop(f);

    match Command::new(cvd_dir.join("bin/mkenvimage_slim"))
        .arg("-output_path")
        .arg(&uboot_env_path)
        .arg("-input_path")
        .arg(uboot_env_input_path)
        .envs(envs)
        .stderr(std::process::Stdio::inherit())
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                println!("Can't find mkenvimage_slim in {}", cvd_dir.display());
            } else {
                println!("Error executing mkenvimage_slim: {err}");
            }
            std::process::exit(-1);
        }
    };

    match Command::new(cvd_dir.join("bin/avbtool"))
        .arg("add_hash_footer")
        .arg("--image")
        .arg(uboot_env_path)
        .arg("--partition_size")
        .arg("73728")
        .arg("--partition_name")
        .arg("uboot_env")
        .arg("--key")
        .arg(cvd_dir.join("etc/cvd_avb_testkey.pem"))
        .arg("--algorithm")
        .arg("SHA256_RSA4096")
        .envs(envs)
        .stderr(std::process::Stdio::inherit())
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                println!("Can't find avbtool in {}", cvd_dir.display());
            } else {
                println!("Error executing avbtool: {}", err);
            }
            std::process::exit(-1);
        }
    };

    Ok(())
}

pub fn create_vbmeta(
    cvd_dir: &Path,
    tmp_dir: &Path,
    envs: &HashMap<&str, &PathBuf>,
) -> Result<(), std::io::Error> {
    let vbmeta_path = tmp_dir.join("vbmeta.img");
    let cvd_key = cvd_dir
        .join("etc/cvd.avbpubkey")
        .into_os_string()
        .into_string()
        .unwrap();

    match Command::new(cvd_dir.join("bin/avbtool"))
        .arg("make_vbmeta_image")
        .arg("--output")
        .arg(&vbmeta_path)
        .arg("--chain_partition")
        .arg(format!("uboot_env:1:{cvd_key}"))
        .arg("--chain_partition")
        .arg(format!("bootconfig:2:{cvd_key}"))
        .arg("--key")
        .arg(cvd_dir.join("etc/cvd_avb_testkey.pem"))
        .arg("--algorithm")
        .arg("SHA256_RSA4096")
        .envs(envs)
        .stderr(std::process::Stdio::inherit())
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                println!("Can't find avbtool in {}", cvd_dir.display());
            } else {
                println!("Error executing avbtool: {}", err);
            }
            std::process::exit(-1);
        }
    };

    let mut f = OpenOptions::new().write(true).open(vbmeta_path)?;
    let metdata = f.metadata()?;
    f.seek(SeekFrom::End(0))?;
    let buf = vec![0; (65535 - metdata.len() + 1).try_into().unwrap()];
    f.write_all(&buf)?;

    Ok(())
}

pub fn create_bootconfig(
    cvd_dir: &Path,
    tmp_dir: &Path,
    envs: &HashMap<&str, &PathBuf>,
    arch: &Arch,
    virgl: bool,
) -> Result<(), std::io::Error> {
    let props_base = b"androidboot.hypervisor.protected_vm.supported=0
androidboot.modem_simulator_ports=9600
androidboot.lcd_density=320
androidboot.vendor.audiocontrol.server.port=9410
androidboot.vendor.audiocontrol.server.cid=3
androidboot.cuttlefish_config_server_port=6800
androidboot.vendor.vehiclehal.server.port=9300
androidboot.fstab_suffix=cf.f2fs.hctr2
androidboot.enable_confirmationui=0
androidboot.hypervisor.vm.supported=0
androidboot.serialno=CUTTLEFISHCVD011
androidboot.setupwizard_mode=DISABLED
androidboot.cpuvulkan.version=4202496
androidboot.ddr_size=4915MB
androidboot.hardware.angle_feature_overrides_enabled=preferLinearFilterForYUV:mapUnspecifiedColorSpaceToPassThrough
androidboot.enable_bootanimation=1
androidboot.hardware.gralloc=minigbm
androidboot.vendor.vehiclehal.server.cid=2
androidboot.hypervisor.version=cf-qemu_cli
androidboot.hardware.vulkan=pastel
androidboot.opengles.version=196609
androidboot.wifi_mac_prefix=5554
androidboot.vsock_tombstone_port=6600
androidboot.hardware.hwcomposer=ranchu
androidboot.serialconsole=0
";
    let props_boot_x86_64 =
        b"androidboot.boot_devices=pci0000:00/0000:00:0f.0,pci0000:00/0000:00:10.0
";
    let props_boot_aarch64 = b"androidboot.boot_devices=4010000000.pcie
";
    let props_render_sw = b"androidboot.hardware.egl=angle
";
    let props_render_virgl = b"androidboot.hardware.egl=mesa
androidboot.hardware.hwcomposer.display_finder_mode=drm
androidboot.hardware.hwcomposer.mode=client
";

    let bootconfig_path = tmp_dir.join("bootconfig");
    let mut f = File::create(&bootconfig_path)?;
    f.write_all(props_base)?;
    match arch {
        Arch::X86_64 => f.write_all(props_boot_x86_64)?,
        Arch::Aarch64 => f.write_all(props_boot_aarch64)?,
    };
    if virgl {
        f.write_all(props_render_virgl)?;
    } else {
        f.write_all(props_render_sw)?;
    }
    drop(f);

    match Command::new(cvd_dir.join("bin/avbtool"))
        .arg("add_hash_footer")
        .arg("--image")
        .arg(bootconfig_path)
        .arg("--partition_size")
        .arg("73728")
        .arg("--partition_name")
        .arg("bootconfig")
        .arg("--key")
        .arg(cvd_dir.join("etc/cvd_avb_testkey.pem"))
        .arg("--algorithm")
        .arg("SHA256_RSA4096")
        .envs(envs)
        .stderr(std::process::Stdio::inherit())
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                println!("Can't find avbtool in {}", cvd_dir.display());
            } else {
                println!("Error executing avbtool: {}", err);
            }
            std::process::exit(-1);
        }
    };

    Ok(())
}
