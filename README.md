# cvd2img

A tool to transform Android Cuttlefish images into raw disk images that can be
used with QEMU (and other VMMs).

## Building

### Dependencies

- **parted** headers and libraries (libparted-dev in Debian, parted-devel in Fedora)
- **clang** headers and libraries (libclang-dev in Debian, clang-devel in Fedora)
- The Rust Toolchain

### Generating the executable

With all dependencies in place, it should be as simple as running `cargo build`.
The resulting binary will be in `./target/debug/cvd2img`.

## Using

### Preparing the Cuttlefish Directory

1. Download a cuttlefish image from
[http://ci.android.com/](http://ci.android.com/). If you don't exactly what to
download, read [this guide](https://source.android.com/docs/setup/create/cuttlefish-use).

2. Create a directory and unpack for the cuttlefish images and the CVD tools.

``` sh
mkdir cuttlefish
cd cuttlefish
unzip ~/Downloads/aosp_cf_x86_64_phone-img*.zip
tar xf ~/Downloads/cvd-host_package.tar.gz
```

### Generating the raw disk images

Go back to the cvd2img build directory and execute it pointing to the directory
containing the cuttlefish images expanded above:

``` sh
./target/debug/cvd2img ~/cuttlefish
```

### More options

``` sh
./target/debug/cvd2img --help
Usage: cvd2img [OPTIONS] <CVD_DIR>

Arguments:
  <CVD_DIR>  Directory containing the Android Cuttlefish images

Options:
  -a, --arch <ARCH>    Architecture of the source images [possible values: x86-64, aarch64]
  -s, --system <FILE>  Output file for the system disk image
  -p, --props <FILE>   Output file for the properties disk image
  -h, --help           Print help
  -V, --version        Print version
```

