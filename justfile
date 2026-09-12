# Build and install popeinput. `just install-user` needs no root and puts
# everything under ~/.local; `just install` goes to /usr/local via sudo.

export PKG_CONFIG_PATH := env_var_or_default("PKG_CONFIG_PATH", "")

build:
    cargo build --release --workspace

test:
    cargo test --workspace

install-user: build
    install -Dm755 target/release/popeinput          ~/.local/bin/popeinput
    install -Dm755 target/release/popeinput-settings ~/.local/bin/popeinput-settings
    install -Dm644 data/io.github.wanleung.popeinput.desktop          ~/.local/share/applications/io.github.wanleung.popeinput.desktop
    install -Dm644 data/io.github.wanleung.popeinput.Settings.desktop ~/.local/share/applications/io.github.wanleung.popeinput.Settings.desktop
    install -Dm644 data/io.github.wanleung.popeinput.desktop          ~/.config/autostart/io.github.wanleung.popeinput.desktop
    -update-desktop-database ~/.local/share/applications

uninstall-user:
    rm -f ~/.local/bin/popeinput ~/.local/bin/popeinput-settings
    rm -f ~/.local/share/applications/io.github.wanleung.popeinput.desktop
    rm -f ~/.local/share/applications/io.github.wanleung.popeinput.Settings.desktop
    rm -f ~/.config/autostart/io.github.wanleung.popeinput.desktop
    -update-desktop-database ~/.local/share/applications

install: build
    sudo install -Dm755 target/release/popeinput          /usr/local/bin/popeinput
    sudo install -Dm755 target/release/popeinput-settings /usr/local/bin/popeinput-settings
    sudo install -Dm644 data/io.github.wanleung.popeinput.desktop          /usr/local/share/applications/io.github.wanleung.popeinput.desktop
    sudo install -Dm644 data/io.github.wanleung.popeinput.Settings.desktop /usr/local/share/applications/io.github.wanleung.popeinput.Settings.desktop
    sudo install -Dm644 data/io.github.wanleung.popeinput.desktop          /etc/xdg/autostart/io.github.wanleung.popeinput.desktop

uninstall:
    sudo rm -f /usr/local/bin/popeinput /usr/local/bin/popeinput-settings
    sudo rm -f /usr/local/share/applications/io.github.wanleung.popeinput.desktop
    sudo rm -f /usr/local/share/applications/io.github.wanleung.popeinput.Settings.desktop
    sudo rm -f /etc/xdg/autostart/io.github.wanleung.popeinput.desktop
