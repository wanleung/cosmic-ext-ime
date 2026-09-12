name := 'cosmic-ext-ime'
export APPID := 'io.github.wanleung.CosmicExtIme'

rootdir := ''
prefix := '/usr'
base-dir := absolute_path(clean(rootdir / prefix))

cargo-target-dir := env('CARGO_TARGET_DIR', 'target')
bin-src := cargo-target-dir / 'release' / name
bin-dst := base-dir / 'bin' / name
settings-src := cargo-target-dir / 'release' / name + '-settings'
settings-dst := base-dir / 'bin' / name + '-settings'

applications-dst := base-dir / 'share' / 'applications'
man-dst := base-dir / 'share' / 'man' / 'man1'
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
    env RUST_LOG=debug cargo run --release -p cosmic-ext-ime {{args}}

install:
    install -Dm0755 {{bin-src}} {{bin-dst}}
    install -Dm0755 {{settings-src}} {{settings-dst}}
    install -Dm0644 data/{{APPID}}.desktop {{applications-dst}}/{{APPID}}.desktop
    install -Dm0644 data/{{APPID}}.Settings.desktop {{applications-dst}}/{{APPID}}.Settings.desktop
    install -Dm0644 data/{{APPID}}.desktop {{autostart-dst}}/{{APPID}}.desktop
    install -Dm0644 data/man/cosmic-ext-ime.1 {{man-dst}}/cosmic-ext-ime.1
    install -Dm0644 data/man/cosmic-ext-ime-settings.1 {{man-dst}}/cosmic-ext-ime-settings.1

uninstall:
    rm -f {{bin-dst}} {{settings-dst}}
    rm -f {{applications-dst}}/{{APPID}}.desktop {{applications-dst}}/{{APPID}}.Settings.desktop
    rm -f {{autostart-dst}}/{{APPID}}.desktop
    rm -f {{man-dst}}/cosmic-ext-ime.1 {{man-dst}}/cosmic-ext-ime-settings.1

home := env('HOME')
user-base := home / '.local'

# Per-user install under ~/.local, no root needed
install-user: build-release
    install -Dm0755 {{bin-src}} {{user-base}}/bin/{{name}}
    install -Dm0755 {{settings-src}} {{user-base}}/bin/{{name}}-settings
    install -Dm0644 data/{{APPID}}.desktop {{user-base}}/share/applications/{{APPID}}.desktop
    install -Dm0644 data/{{APPID}}.Settings.desktop {{user-base}}/share/applications/{{APPID}}.Settings.desktop
    install -Dm0644 data/{{APPID}}.desktop {{home}}/.config/autostart/{{APPID}}.desktop
    install -Dm0644 data/man/{{name}}.1 {{user-base}}/share/man/man1/{{name}}.1
    install -Dm0644 data/man/{{name}}-settings.1 {{user-base}}/share/man/man1/{{name}}-settings.1
    -update-desktop-database {{user-base}}/share/applications

uninstall-user:
    rm -f {{user-base}}/bin/{{name}} {{user-base}}/bin/{{name}}-settings
    rm -f {{user-base}}/share/applications/{{APPID}}.desktop {{user-base}}/share/applications/{{APPID}}.Settings.desktop
    rm -f {{home}}/.config/autostart/{{APPID}}.desktop
    rm -f {{user-base}}/share/man/man1/{{name}}.1 {{user-base}}/share/man/man1/{{name}}-settings.1
    -update-desktop-database {{user-base}}/share/applications

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
