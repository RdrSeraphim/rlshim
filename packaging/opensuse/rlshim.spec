Name:           rlshim
Version:        1.0.1
Release:        1%{?dist}
Summary:        A lightweight, native Linux launcher for RuneLite.

License:        BSD-2-Clause
URL:            https://github.com/RdrSeraphim/rlshim
Source0:        %{name}-%{version}.tar.gz
Source1:        libcurl-impersonate-v1.5.6.x86_64-linux-gnu.tar.gz
Source2:        json-3.12.0.tar.gz
Source3:        glfw-3.3.8.tar.gz
Source4:        imgui-1.92.8.tar.gz
Source5:        libcurl-impersonate-v1.5.6.aarch64-linux-gnu.tar.gz

BuildRequires:  cmake
BuildRequires:  clang
BuildRequires:  gcc-c++
BuildRequires:  make
BuildRequires:  pkg-config
BuildRequires:  pkgconf
BuildRequires:  libsecret-devel
BuildRequires:  openssl-devel
BuildRequires:  libglfw-devel
BuildRequires:  Mesa-libGL-devel
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
%setup -q -n rlshim-%{version} -a 2 -a 3 -a 4

%build
mkdir -p build/_deps/curl-impersonate/extracted
%ifarch x86_64
tar -xzf %{SOURCE1} -C build/_deps/curl-impersonate/extracted
%endif
%ifarch aarch64
tar -xzf %{SOURCE5} -C build/_deps/curl-impersonate/extracted
%endif

export LDFLAGS="%{?__global_ldflags}"
export CFLAGS="%{optflags}"
export CXXFLAGS="%{optflags}"
%cmake -DCMAKE_BUILD_TYPE=Release \
       -DFETCHCONTENT_SOURCE_DIR_JSON=%{_builddir}/rlshim-%{version}/json-3.12.0 \
       -DFETCHCONTENT_SOURCE_DIR_GLFW=%{_builddir}/rlshim-%{version}/glfw-3.3.8 \
       -DFETCHCONTENT_SOURCE_DIR_IMGUI=%{_builddir}/rlshim-%{version}/imgui-1.92.8
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
* Tue Jun 23 2026 Seraphim Pardee <me@srp.life> - 1.0.1-1
- Add placeholders for empty character names in character selection menu.
* Sun Jun 14 2026 Seraphim Pardee <me@srp.life> - 1.0.0-1
- Initial release
