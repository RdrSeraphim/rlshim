Name:           rlshim
Version:        1.2.0
Release:        1%{?dist}
Summary:        A lightweight, native Linux launcher for RuneLite.

License:        BSD-2-Clause
URL:            https://github.com/RdrSeraphim/rlshim
Source0:        https://github.com/RdrSeraphim/rlshim/archive/refs/tags/v%{version}.tar.gz

BuildRequires:  cmake
BuildRequires:  clang
BuildRequires:  make
BuildRequires:  pkgconf-pkg-config
BuildRequires:  libsecret-devel
BuildRequires:  openssl-devel
BuildRequires:  glfw-devel
BuildRequires:  mesa-libGL-devel
BuildRequires:  libcurl-devel
BuildRequires:  libX11-devel
BuildRequires:  libXcursor-devel
BuildRequires:  libXi-devel
BuildRequires:  libXinerama-devel
BuildRequires:  libXrandr-devel
BuildRequires:  git

Requires:       libsecret
Requires:       openssl
Requires:       glfw
Requires:       glibc

%description
rlshim is a lightweight, native Linux launcher for RuneLite, designed as an alternative to the official AppImage and Flatpak.

%prep
%autosetup -n rlshim-%{version}

%build
export CC=clang
export CXX=clang++
%cmake -DCMAKE_BUILD_TYPE=Release
%cmake_build

%install
%cmake_install

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
