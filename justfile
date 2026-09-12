name := 'popeinput'
export APPID := 'io.github.wanleung.popeinput'

rootdir := ''
prefix := '/usr'
base-dir := absolute_path(clean(rootdir / prefix))

cargo-target-dir := env('CARGO_TARGET_DIR', 'target')
bin-src := cargo-target-dir / 'release' / name
bin-dst := base-dir / 'bin' / name
settings-src := cargo-target-dir / 'release' / name + '-settings'
settings-dst := base-dir / 'bin' / name + '-settings'

applications-dst := base-dir / 'share' / 'applications'
autostart-dst := absolute_path(clean(rootdir / 'etc' / 'xdg' / 'autostart'))

default: build-release

clean:
    cargo clean

clean-vendor:
    rm -rf .cargo vendor vendor.tar

clean-dist: clean clean-vendor

build-debug *args:
    cargo build --workspace {{args}}

build-release *args: (build-debug '--release' args)

# Release build from vendor.tar, for package builds without network access
build-vendored *args: vendor-extract (build-release '--frozen --offline' args)

test:
    cargo test --workspace

check:
    cargo clippy --workspace --all-targets

run *args:
    env RUST_LOG=debug cargo run --release -p popeinput {{args}}

install:
    install -Dm0755 {{bin-src}} {{bin-dst}}
    install -Dm0755 {{settings-src}} {{settings-dst}}
    install -Dm0644 data/{{APPID}}.desktop {{applications-dst}}/{{APPID}}.desktop
    install -Dm0644 data/{{APPID}}.Settings.desktop {{applications-dst}}/{{APPID}}.Settings.desktop
    install -Dm0644 data/{{APPID}}.desktop {{autostart-dst}}/{{APPID}}.desktop

uninstall:
    rm -f {{bin-dst}} {{settings-dst}}
    rm -f {{applications-dst}}/{{APPID}}.desktop {{applications-dst}}/{{APPID}}.Settings.desktop
    rm -f {{autostart-dst}}/{{APPID}}.desktop

# Per-user install under ~/.local, no root needed
install-user: build-release
    just rootdir=~/.local prefix='' install
    install -Dm0644 data/{{APPID}}.desktop ~/.config/autostart/{{APPID}}.desktop
    -update-desktop-database ~/.local/share/applications

uninstall-user:
    just rootdir=~/.local prefix='' uninstall
    rm -f ~/.config/autostart/{{APPID}}.desktop
    -update-desktop-database ~/.local/share/applications

# Vendor dependencies into vendor.tar so the package builds offline
vendor:
    #!/usr/bin/env bash
    set -e
    mkdir -p .cargo
    cargo vendor --sync Cargo.toml | head -n -1 > .cargo/config.toml
    echo 'directory = "vendor"' >> .cargo/config.toml
    tar pcf vendor.tar .cargo vendor
    rm -rf .cargo vendor

vendor-extract:
    rm -rf vendor
    tar pxf vendor.tar

# Build the Debian package (needs debhelper, devscripts, just)
deb:
    dpkg-buildpackage -us -uc -b
