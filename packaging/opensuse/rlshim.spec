Name:           rlshim
Version:        1.5.1
Release:        1%{?dist}
Summary:        A lightweight, native Linux launcher for RuneLite.

License:        BSD-2-Clause
URL:            https://github.com/RdrSeraphim/rlshim
Source0:        %{name}-%{version}.tar.gz
Source1:        vendor.tar.gz
Source2:        libcurl-impersonate-v1.5.6.x86_64-linux-gnu.tar.gz
Source3:        libcurl-impersonate-v1.5.6.aarch64-linux-gnu.tar.gz

BuildRequires:  cargo-packaging
BuildRequires:  rust >= 1.85
BuildRequires:  gcc
BuildRequires:  gcc-c++
BuildRequires:  pkg-config
BuildRequires:  pkgconf
BuildRequires:  libsecret-devel

# Runtime dependencies (eframe/winit dlopens Wayland/X11/GL at runtime)
Requires:       libsecret-1-0
Requires:       glibc
Requires:       jre >= 11

%description
rlshim is a lightweight, native Linux launcher for RuneLite, designed as an alternative to the official AppImage and Flatpak.

%prep
%autosetup -a1

%build
mkdir -p %{_builddir}/curl-impersonate-extracted
%ifarch x86_64
tar -xzf %{SOURCE2} -C %{_builddir}/curl-impersonate-extracted
%endif
%ifarch aarch64
tar -xzf %{SOURCE3} -C %{_builddir}/curl-impersonate-extracted
%endif

export RLSHIM_CURL_IMP_DIR="%{_builddir}/curl-impersonate-extracted"
%cargo_build

%install
install -Dm755 target/release/rlshim %{buildroot}%{_bindir}/rlshim
install -Dm644 rlshim.desktop %{buildroot}%{_datadir}/applications/rlshim.desktop
install -Dm644 data/icon.png %{buildroot}%{_datadir}/pixmaps/rlshim.png
install -d %{buildroot}%{_datadir}/rlshim/data
cp -a data/* %{buildroot}%{_datadir}/rlshim/data/

%check
export RLSHIM_CURL_IMP_DIR="%{_builddir}/curl-impersonate-extracted"
%cargo_test

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
* Wed Nov 04 2026 Seraphim Pardee <me@srp.life> - 1.5.1-1
- build fixes for offline chroot with local deps
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
