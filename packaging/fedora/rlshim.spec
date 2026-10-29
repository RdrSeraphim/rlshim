Name:           rlshim
Version:        1.5.0
Release:        1%{?dist}
Summary:        A lightweight, native Linux launcher for RuneLite.

License:        BSD-2-Clause
URL:            https://github.com/RdrSeraphim/rlshim
Source0:        https://github.com/RdrSeraphim/rlshim/archive/refs/tags/v%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc-c++
BuildRequires:  make
BuildRequires:  pkgconf-pkg-config
BuildRequires:  libsecret-devel
BuildRequires:  curl

Requires:       libsecret
Requires:       glibc
# RuneLite needs a JRE (11+); rlshim can't launch it otherwise, so pull one in.
Requires:       java >= 1:11

%description
rlshim is a lightweight, native Linux launcher for RuneLite, designed as an alternative to the official AppImage and Flatpak.

%prep
%autosetup -n rlshim-%{version}

%build
cargo build --release --locked

%install
DESTDIR=%{buildroot} PREFIX=%{_prefix} make install

%files
%license LICENSE
%{_bindir}/rlshim
%{_datadir}/applications/rlshim.desktop
%{_datadir}/pixmaps/rlshim.png
%dir %{_datadir}/rlshim
%dir %{_datadir}/rlshim/data
%{_datadir}/rlshim/data/background.png
%{_datadir}/rlshim/data/icon.png
%{_datadir}/rlshim/data/*.ttf

%changelog
* Wed Oct 28 2026 Seraphim Pardee <me@srp.life> - 1.5.0-1
- Port to rust, various Steam Deck bug fixes.
* Mon Sep 21 2026 Seraphim Pardee <me@srp.life> - 1.4.0-1
- add encrypted credentials vault for fallback
* Thu Jul 16 2026 Seraphim Pardee <me@srp.life> - 1.3.0-1
- Save preferred character for multi-char accounts, proper cli parsing
* Wed Jul 15 2026 Seraphim Pardee <me@srp.life> - 1.2.1-1
- Improve runelite handling, add java dependency, unit tests
* Fri Jul 03 2026 Seraphim Pardee <me@srp.life> - 1.2.0-1
- Add prompts for keyring unlock and critical errors.
* Wed Jun 24 2026 Seraphim Pardee <me@srp.life> - 1.1.1-1
- Version bump only to mitigate GitHub tag nuisance.
* Wed Jun 24 2026 Seraphim Pardee <me@srp.life> - 1.1.0-1
- Add flatpak support.
* Tue Jun 23 2026 Seraphim Pardee <me@srp.life> - 1.0.1-1
- Add placeholders for empty character names in character selection menu.
* Sun Jun 14 2026 Seraphim Pardee <me@srp.life> - 1.0.0-1
- Initial release
