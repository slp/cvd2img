use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
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
    cvd_dir: &str,
    envs: &HashMap<String, String>,
    image: &str,
) -> Result<(), std::io::Error> {
    let src = format!("{cvd_dir}/{image}");
    let tmp = format!("{cvd_dir}/{image}.tmp");
    let args = vec![src.clone(), tmp.clone()];

    match Command::new(format!("{cvd_dir}/bin/simg2img"))
        .args(&args)
        .envs(envs)
        .stderr(std::process::Stdio::inherit())
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                println!("Can't find simg2img in {cvd_dir}");
            } else {
                println!("Error executing simg2img: {}", err);
            }
            std::process::exit(-1);
        }
    };

    std::fs::rename(tmp, src)
}

fn is_sparse(cvd_dir: &str, image: &str) -> Result<bool, std::io::Error> {
    let mut f = File::open(format!("{cvd_dir}/{image}"))?;
    let mut buf: [u8; 4] = [0; 4];
    f.read_exact(&mut buf)?;

    if buf == SPARSE_MAGIC {
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn transform_sparse_images(
    cvd_dir: &str,
    envs: &HashMap<String, String>,
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
    cvd_dir: &str,
    tmp_dir: &str,
    envs: &HashMap<String, String>,
) -> Result<(), std::io::Error> {
    let uboot_env_path = format!("{tmp_dir}/uboot_env.img");
    let uboot_env_input_data = b"uenvcmd=setenv bootargs \"$cbootargs console=hvc0 earlycon=pl011,mmio32,0x9000000 \" && run bootcmd_android";
    let uboot_env_input_path = format!("{tmp_dir}/uboot_env_input");

    let args = vec![
        "-output_path".to_string(),
        uboot_env_path.clone(),
        "-input_path".to_string(),
        uboot_env_input_path.clone(),
    ];

    let mut f = File::create(&uboot_env_input_path)?;
    f.write_all(uboot_env_input_data)?;
    drop(f);

    match Command::new(format!("{cvd_dir}/bin/mkenvimage_slim"))
        .args(&args)
        .envs(envs)
        .stderr(std::process::Stdio::inherit())
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                println!("Can't find mkenvimage_slim in {cvd_dir}");
            } else {
                println!("Error executing mkenvimage_slim: {}", err);
            }
            std::process::exit(-1);
        }
    };

    let args = vec![
        "add_hash_footer".to_string(),
        "--image".to_string(),
        uboot_env_path,
        "--partition_size".to_string(),
        "73728".to_string(),
        "--partition_name".to_string(),
        "uboot_env".to_string(),
        "--key".to_string(),
        format!("{cvd_dir}/etc/cvd_avb_testkey.pem"),
        "--algorithm".to_string(),
        "SHA256_RSA4096".to_string(),
    ];

    match Command::new(format!("{cvd_dir}/bin/avbtool"))
        .args(&args)
        .envs(envs)
        .stderr(std::process::Stdio::inherit())
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                println!("Can't find avbtool in {cvd_dir}");
            } else {
                println!("Error executing avbtool: {}", err);
            }
            std::process::exit(-1);
        }
    };

    Ok(())
}

pub fn create_vbmeta(
    cvd_dir: &str,
    tmp_dir: &str,
    envs: &HashMap<String, String>,
) -> Result<(), std::io::Error> {
    let vbmeta_path = format!("{tmp_dir}/vbmeta.img");

    let args = vec![
        "make_vbmeta_image".to_string(),
        "--output".to_string(),
        vbmeta_path.clone(),
        "--chain_partition".to_string(),
        format!("uboot_env:1:{cvd_dir}/etc/cvd.avbpubkey"),
        "--chain_partition".to_string(),
        format!("bootconfig:2:{cvd_dir}/etc/cvd.avbpubkey"),
        "--key".to_string(),
        format!("{cvd_dir}/etc/cvd_avb_testkey.pem"),
        "--algorithm".to_string(),
        "SHA256_RSA4096".to_string(),
    ];

    match Command::new(format!("{cvd_dir}/bin/avbtool"))
        .args(&args)
        .envs(envs)
        .stderr(std::process::Stdio::inherit())
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                println!("Can't find avbtool in {cvd_dir}");
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
    cvd_dir: &str,
    tmp_dir: &str,
    envs: &HashMap<String, String>,
    arch: &Arch,
) -> Result<(), std::io::Error> {
    let props_x86_64_sw = b"androidboot.hypervisor.protected_vm.supported=0
androidboot.modem_simulator_ports=9600
androidboot.lcd_density=320
androidboot.vendor.audiocontrol.server.port=9410
androidboot.vendor.audiocontrol.server.cid=3
androidboot.cuttlefish_config_server_port=6800
androidboot.vendor.vehiclehal.server.port=9300
androidboot.vsock_touch_port=7100
androidboot.fstab_suffix=cf.f2fs.hctr2
androidboot.vsock_keyboard_port=7000
androidboot.enable_confirmationui=0
androidboot.hypervisor.vm.supported=1
androidboot.serialno=CUTTLEFISHCVD011
androidboot.setupwizard_mode=DISABLED
androidboot.cpuvulkan.version=4202496
androidboot.ddr_size=4915MB
androidboot.boot_devices=pci0000:00/0000:00:0d.0,pci0000:00/0000:00:0e.0
androidboot.hardware.angle_feature_overrides_enabled=preferLinearFilterForYUV:mapUnspecifiedColorSpaceToPassThrough
androidboot.hardware.egl=angle
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

    let props_aarch64_sw = b"androidboot.hypervisor.protected_vm.supported=0
androidboot.modem_simulator_ports=9600
androidboot.lcd_density=320
androidboot.vendor.audiocontrol.server.port=9410
androidboot.vendor.audiocontrol.server.cid=3
androidboot.cuttlefish_config_server_port=6800
androidboot.vendor.vehiclehal.server.port=9300
androidboot.vsock_touch_port=7100
androidboot.fstab_suffix=cf.f2fs.hctr2
androidboot.vsock_keyboard_port=7000
androidboot.enable_confirmationui=0
androidboot.hypervisor.vm.supported=0
androidboot.serialno=CUTTLEFISHCVD011
androidboot.setupwizard_mode=DISABLED
androidboot.cpuvulkan.version=4202496
androidboot.ddr_size=4915MB
androidboot.boot_devices=4010000000.pcie
androidboot.hardware.angle_feature_overrides_enabled=preferLinearFilterForYUV:mapUnspecifiedColorSpaceToPassThrough
androidboot.hardware.egl=angle
androidboot.enable_bootanimation=1
androidboot.hardware.gralloc=minigbm
androidboot.vendor.vehiclehal.server.cid=2
androidboot.hardware.vulkan=pastel
androidboot.opengles.version=196609
androidboot.wifi_mac_prefix=5554
androidboot.vsock_tombstone_port=6600
androidboot.hardware.hwcomposer=ranchu
androidboot.serialconsole=0
";

    let bootconfig_path = format!("{tmp_dir}/bootconfig");
    let mut f = File::create(bootconfig_path.clone())?;
    match arch {
        Arch::Aarch64 => f.write_all(props_aarch64_sw)?,
        Arch::X86_64 => f.write_all(props_x86_64_sw)?,
    };
    drop(f);

    let args = vec![
        "add_hash_footer".to_string(),
        "--image".to_string(),
        bootconfig_path,
        "--partition_size".to_string(),
        "73728".to_string(),
        "--partition_name".to_string(),
        "bootconfig".to_string(),
        "--key".to_string(),
        format!("{cvd_dir}/etc/cvd_avb_testkey.pem"),
        "--algorithm".to_string(),
        "SHA256_RSA4096".to_string(),
    ];

    match Command::new(format!("{cvd_dir}/bin/avbtool"))
        .args(&args)
        .envs(envs)
        .stderr(std::process::Stdio::inherit())
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                println!("Can't find avbtool in {cvd_dir}");
            } else {
                println!("Error executing avbtool: {}", err);
            }
            std::process::exit(-1);
        }
    };

    Ok(())
}
