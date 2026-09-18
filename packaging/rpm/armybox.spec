Name:           armybox
Version:        0.4.0
Release:        1%{?dist}
Summary:        BusyBox/Toybox clone written in Rust

License:        MIT OR Apache-2.0
URL:            https://github.com/quinnjr/armybox
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust >= 1.70

%description
armybox is a modern implementation of common Unix utilities
in a single multi-call binary, similar to BusyBox and Toybox.

Features include:
- 303 applets (coreutils, compression, networking, shell, init)
- Memory-safe Rust implementation
- ash-compatible shell
- PID 1 init system
- Static linking support

%prep
%autosetup

%build
cargo build --release

%install
install -D -m 755 target/release/armybox %{buildroot}%{_bindir}/armybox
install -D -m 755 packaging/debian/armybox-install-symlinks %{buildroot}%{_bindir}/armybox-install-symlinks

# Create man page directory
mkdir -p %{buildroot}%{_mandir}/man1

%check
cargo test --release

%post
echo "Run 'armybox-install-symlinks' to create symlinks for all applets."

%files
%license LICENSE-MIT LICENSE-APACHE
%doc README.md BENCHMARK.md DOCKER.md
%{_bindir}/armybox
%{_bindir}/armybox-install-symlinks

%changelog
* Thu Jan 02 2026 Joseph R. Quinn <quinn.josephr@protonmail.com> - 0.1.0-1
- Initial package
- 303 applets including coreutils, compression, networking
- ash-compatible shell
- PID 1 init system
